/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The adversarial parenthesization matrix: a generated cross-product over
//! (parent kind × child kind × child position) in which **every child is
//! explicitly parenthesized in the source**, round-tripped in both
//! [`Pretty`] modes.
//!
//! # Why this file exists
//!
//! Task 15 measured the Tier 1 corpus gate (`tests/corpus.rs`) and found
//! what it does *not* reach. The 420 checked-in corpus files reach 262 of
//! the 271 node kinds, but they contain only **87 parenthesized nodes**,
//! spread over 23 kinds and 40 distinct (parent → child) edges.
//! `FunctionTypeAnnotation` and `ConditionalTypeAnnotation` never appear
//! parenthesized anywhere in them; `OptionalMemberExpression`,
//! `OptionalCallExpression`, `ConditionalExpression`, `YieldExpression`,
//! `AwaitExpression`, `UnaryExpression`, `NewExpression`,
//! `TaggedTemplateExpression`, `SpreadElement`, `PrivateName`,
//! `TemplateLiteral` and `BigIntLiteral` never appear parenthesized at all.
//!
//! The proof that this is a real hole, not a theoretical one: breaking the
//! optional-chain rule in `precedence.rs` fails this crate's hand-written
//! unit tests **while the corpus gate stays green**. Real-world source
//! almost never writes a redundant parenthesis, so adding more real files
//! adds node *kinds* and almost no new parenthesized *shapes*. All 27
//! defects found in this port live in the "must ADD parens" direction of
//! `need_parens`, which is exactly the direction real source cannot
//! exercise.
//!
//! So this file does not read files. It **generates** the shapes:
//!
//! 1. an [`Edge`] table, one entry per `print_child` call site in
//!    `src/` — the parent kind, the field, and the child position — written
//!    as a source template with a `%s` hole;
//! 2. a [`Payload`] table, one entry per node kind that can occupy such a
//!    hole, per dialect;
//! 3. a [`Frame`] wrapper that decides what is in scope around the
//!    template (`yield`/`await`/`super`/`new.target`, strict vs sloppy).
//!
//! Each (frame × edge × payload) triple is instantiated with the payload
//! **wrapped in parentheses**, so the parsed tree holds the raw parent →
//! child edge with no `Paren` node of any kind (this parser, like every
//! ESTree producer, records grouping parens nowhere). The generator must
//! then decide, unaided, whether to put them back. Anything it gets wrong
//! either fails to reparse or reparses to a different tree, and both are
//! caught here.
//!
//! # Skips are classified, never listed
//!
//! Most generated triples are not legal JavaScript — `(yield x) = 1` has no
//! meaning, a `SpreadElement` cannot be parenthesized at all, `await`
//! outside an async function is an identifier. A skip list of *strings*
//! would let a shape leave the matrix silently the moment anything about it
//! changed, which is precisely how defect 24's first fix passed review with
//! two whole node families missing.
//!
//! Instead every skip is **predicted from tags** by [`predict_skip`] before
//! the fact and classified into a [`Skip`] variant. A triple whose source
//! fails to parse and for which `predict_skip` has no rule is an
//! **unclassified skip and fails this test**. The per-reason counts are
//! pinned, so the matrix cannot quietly shrink either.
//!
//! # Parenthesizability is measured, not declared
//!
//! The premise "wrap the payload in parens and the raw edge appears in the
//! tree" is false for some payloads, for reasons that belong to the
//! *parser*: TypeScript's `(`-cover only reaches the full type grammar when
//! what follows the `(` is an identifier or another `(`, and four TS keyword
//! types are mapped by `parseTSPrimaryType` but not by
//! `reparseIdentifierAsTSTypeAnnotation`. Hand-writing which payloads those
//! are would be the string list this file exists to avoid, and would go
//! stale the day the parser changed. So [`measure_paren_behavior`] probes
//! each payload once in a canonical context and the matrix wraps only the
//! ones it measures as [`ParenBehavior::Transparent`]; the other two buckets
//! are reported and pinned as evidence about the parser (`MANIFEST.md`,
//! PD-1/PD-2).
//!
//! # What is pinned
//!
//! [`paren_matrix_every_edge_round_trips`] asserts, in this order: no
//! failures; no unclassified skips; that every edge and every payload took
//! part in at least one live probe; that the probe count is exactly the
//! cross-product it claims to be, and its value; the live/round-trip/
//! cover-grammar/dialect-conflict counts; the count of each skip reason,
//! and that they account for every non-live probe; the number of distinct
//! (parent kind → child kind) pairs the matrix put into a tree — the number
//! directly comparable to the corpus gate's 40; and the two measured
//! parser-defect buckets.
//!
//! # What it found
//!
//! The first run was **289 failures out of 13 012 round trips**, in 13
//! root-cause groups: 6 generator defects nothing else in this crate
//! caught (28-33, plus 34 and 35 which it found on the second pass), 2
//! parser defects, and one already-documented domain exclusion. Two
//! mutation experiments, recorded in `MANIFEST.md`, show the corpus gate,
//! `roundtrip.rs` and `sweep_regressions.rs` all staying **green** while
//! this file goes red.

// The two tables below are long runs of `v.push(...)`, one line per entry, so
// that a row can be added, removed or annotated without touching its
// neighbours. Clippy would rather see one `vec![...]` literal; that would make
// every diff of these tables span the whole literal.
#![allow(clippy::vec_init_then_push)]

use std::collections::{BTreeMap, HashSet};

use hermes_ast::dump::{ESTreeDumpMode, ESTreeRawProp, LocationDumpMode};
use hermes_ast::node::{Node, NodeKind};
use hermes_ast::visitor::Visitor;
use hermes_gen_js::{generate, Opt, Pretty};
use hermes_parser::{ParseFlags, ParsedJS};

// ===========================================================================
// Dialects
// ===========================================================================

/// The grammar features a template or payload needs turned on.
///
/// Kept as a set of independent flags rather than an enum because the
/// dialects genuinely compose: `-parse-jsx -parse-flow` is a real
/// configuration (`tests/corpus.rs`'s `corpus_parser_jsx_flow`), and Flow
/// `match` and component syntax are each an extra flag on top of Flow.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Dialect {
    /// `-parse-flow`.
    flow: bool,
    /// `-Xparse-flow-match` (implies [`Self::flow`]).
    flow_match: bool,
    /// `-Xparse-component-syntax` (implies [`Self::flow`]).
    flow_component: bool,
    /// `-parse-ts`.
    ts: bool,
    /// `-parse-jsx`.
    jsx: bool,
}

/// Plain ECMAScript: no dialect flag at all.
const ES: Dialect = Dialect {
    flow: false,
    flow_match: false,
    flow_component: false,
    ts: false,
    jsx: false,
};

/// `-parse-flow`.
const FLOW: Dialect = Dialect {
    flow: true,
    flow_match: false,
    flow_component: false,
    ts: false,
    jsx: false,
};

/// `-parse-flow -Xparse-flow-match`.
const FLOW_MATCH: Dialect = Dialect {
    flow: true,
    flow_match: true,
    flow_component: false,
    ts: false,
    jsx: false,
};

/// `-parse-flow -Xparse-component-syntax`.
const FLOW_COMPONENT: Dialect = Dialect {
    flow: true,
    flow_match: false,
    flow_component: true,
    ts: false,
    jsx: false,
};

/// `-parse-ts`.
const TS: Dialect = Dialect {
    flow: false,
    flow_match: false,
    flow_component: false,
    ts: true,
    jsx: false,
};

/// `-parse-jsx`.
const JSX: Dialect = Dialect {
    flow: false,
    flow_match: false,
    flow_component: false,
    ts: false,
    jsx: true,
};

impl Dialect {
    /// The union of two dialects, or `None` if they cannot be combined.
    ///
    /// Flow and TypeScript are the one genuine conflict: they are two
    /// different type grammars competing for the same `:`/`<`/`as` syntax,
    /// and the parser's own corpora never enable both
    /// (`tests/corpus.rs`'s per-directory flag sets). A pair that lands
    /// here is not *skipped* — it is never generated at all, and is
    /// reported separately as [`Matrix::dialect_conflicts`].
    fn merge(self, other: Dialect) -> Option<Dialect> {
        if (self.flow || self.flow_match || self.flow_component) && other.ts
            || self.ts && (other.flow || other.flow_match || other.flow_component)
        {
            return None;
        }
        Some(Dialect {
            flow: self.flow || other.flow || self.flow_match || other.flow_match,
            flow_match: self.flow_match || other.flow_match,
            flow_component: self.flow_component || other.flow_component,
            ts: self.ts || other.ts,
            jsx: self.jsx || other.jsx,
        })
    }

    /// The [`ParseFlags`] this dialect asks the parser for.
    fn flags(self) -> ParseFlags {
        ParseFlags {
            parse_flow: self.flow || self.flow_match || self.flow_component,
            parse_flow_component_syntax: self.flow_component,
            parse_flow_records: false,
            parse_flow_match: self.flow_match,
            parse_ts: self.ts,
            parse_jsx: self.jsx,
            strict_mode: false,
        }
    }
}

// ===========================================================================
// Frames
// ===========================================================================

/// What is in lexical scope around an edge template.
///
/// The same expression edge behaves differently at script top level and
/// inside an async generator method: `yield`/`await`/`super`/`new.target`
/// are available in one and not the other, and class bodies are strict
/// code. Running the expression edges in both is what makes the matrix a
/// cross-product over *context* as well as over parent and child.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Frame {
    /// The template as a top-level statement of a sloppy-mode script.
    Script,
    /// The template inside `class C extends B { async *m() { … } }`:
    /// strict code with `yield`, `await`, `super.x`, `new.target` and
    /// `arguments` all in scope.
    Method,
}

impl Frame {
    /// Wrap `stmt` in this frame.
    fn wrap(self, stmt: &str) -> String {
        match self {
            Frame::Script => stmt.to_string(),
            Frame::Method => format!("class C extends B {{ async *m() {{ {stmt} }} }}"),
        }
    }

    /// Whether `yield` is a yield operator here rather than an identifier.
    fn has_yield(self) -> bool {
        self == Frame::Method
    }

    /// Whether `await` is an await operator here.
    fn has_await(self) -> bool {
        self == Frame::Method
    }

    /// Whether `super.x` has a home object here.
    fn has_super(self) -> bool {
        self == Frame::Method
    }

    /// Whether `new.target`/`arguments` are in scope here.
    fn has_function(self) -> bool {
        self == Frame::Method
    }

}

// ===========================================================================
// Grammatical contexts
// ===========================================================================

/// The grammatical category a hole accepts and a payload produces.
///
/// Edges and payloads are only paired within one context: a Flow type
/// cannot occupy a TypeScript type hole, and neither can occupy an
/// expression hole. Cross-context pairs are not "skips", they are simply
/// outside the matrix, and are not counted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ctx {
    /// An expression (or, for two edges, a declaration in expression
    /// position).
    Expr,
    /// A Flow type annotation.
    FlowType,
    /// A TypeScript type annotation.
    TsType,
    /// A Flow `match` pattern.
    MatchPattern,
}

// ===========================================================================
// Payloads
// ===========================================================================

/// One child shape that can be dropped into a hole.
#[derive(Clone)]
struct Payload {
    /// Reported name; by convention the node kind, disambiguated where one
    /// kind has several interesting spellings.
    name: &'static str,
    /// The source text of the child, **without** parentheses.
    src: &'static str,
    /// Which holes this can go in.
    ctx: Ctx,
    /// The dialect this spelling needs.
    dialect: Dialect,
    /// Whether `( src )` is itself legal. False for the two shapes the
    /// grammar forbids parenthesizing at all: `...a` and a bare `#x`.
    parenthesizable: bool,
    /// Whether this is a legal assignment/update target.
    target: bool,
    /// This payload is a `SpreadElement`.
    spread: bool,
    /// This payload is a bare `PrivateName`.
    private_name: bool,
    /// Needs an enclosing generator.
    needs_yield: bool,
    /// Needs an enclosing async function.
    needs_await: bool,
    /// Needs an enclosing method with a home object.
    needs_super: bool,
    /// Needs an enclosing function (`new.target`, `arguments`).
    needs_function: bool,
}

/// Start a payload with every tag off.
fn p(name: &'static str, src: &'static str, ctx: Ctx, dialect: Dialect) -> Payload {
    Payload {
        name,
        src,
        ctx,
        dialect,
        parenthesizable: true,
        target: false,
        spread: false,
        private_name: false,
        needs_yield: false,
        needs_await: false,
        needs_super: false,
        needs_function: false,
    }
}

impl Payload {
    /// Mark as a legal assignment/update target.
    fn target(mut self) -> Self {
        self.target = true;
        self
    }
    /// Mark as a `SpreadElement`: legal only in an argument/element list,
    /// and never parenthesizable.
    fn spread(mut self) -> Self {
        self.spread = true;
        self.parenthesizable = false;
        self
    }
    /// Mark as a bare `PrivateName`: legal only as `in`'s left operand,
    /// and never parenthesizable.
    fn private_name(mut self) -> Self {
        self.private_name = true;
        self.parenthesizable = false;
        self
    }
    /// Mark as needing an enclosing generator.
    /// Mark as needing an enclosing generator.
    fn needs_yield(mut self) -> Self {
        self.needs_yield = true;
        self
    }
    /// Mark as needing an enclosing async function.
    fn needs_await(mut self) -> Self {
        self.needs_await = true;
        self
    }
    /// Mark as needing a home object.
    fn needs_super(mut self) -> Self {
        self.needs_super = true;
        self
    }
    /// Mark as needing an enclosing function.
    fn needs_function(mut self) -> Self {
        self.needs_function = true;
        self
    }
}

/// Every expression, type and pattern shape the matrix drops into a hole.
///
/// The expression half deliberately includes every kind the Task 15
/// measurement found the corpus never parenthesizes, and both operand
/// spellings of the operators whose associativity is a known hazard (`**`,
/// `in`, `??`).
fn payloads() -> Vec<Payload> {
    let mut v = Vec::new();

    // --- ES expressions ---------------------------------------------------
    v.push(p("Identifier", "x", Ctx::Expr, ES).target());
    v.push(p("ThisExpression", "this", Ctx::Expr, ES));
    v.push(p("NullLiteral", "null", Ctx::Expr, ES));
    v.push(p("BooleanLiteral", "true", Ctx::Expr, ES));
    v.push(p("StringLiteral", "\"s\"", Ctx::Expr, ES));
    v.push(p("NumericLiteral", "1", Ctx::Expr, ES));
    v.push(p("NumericLiteral.int", "50", Ctx::Expr, ES));
    v.push(p("BigIntLiteral", "1n", Ctx::Expr, ES));
    v.push(p("RegExpLiteral", "/re/g", Ctx::Expr, ES));
    v.push(p("TemplateLiteral", "`t${x}`", Ctx::Expr, ES));
    v.push(p("TaggedTemplateExpression", "f`t`", Ctx::Expr, ES));
    v.push(p("ArrayExpression", "[x]", Ctx::Expr, ES));
    v.push(p("ObjectExpression", "{a: 1}", Ctx::Expr, ES));
    v.push(p("FunctionExpression", "function () {}", Ctx::Expr, ES));
    v.push(p("ArrowFunctionExpression", "() => 1", Ctx::Expr, ES));
    v.push(p("ClassExpression", "class {}", Ctx::Expr, ES));
    v.push(p("SequenceExpression", "a, b", Ctx::Expr, ES));
    v.push(p("AssignmentExpression", "a = b", Ctx::Expr, ES));
    v.push(p("ConditionalExpression", "a ? b : c", Ctx::Expr, ES));
    v.push(p("LogicalExpression.or", "a || b", Ctx::Expr, ES));
    v.push(p("LogicalExpression.nullish", "a ?? b", Ctx::Expr, ES));
    v.push(p("BinaryExpression.add", "a + b", Ctx::Expr, ES));
    v.push(p("BinaryExpression.in", "a in b", Ctx::Expr, ES));
    v.push(p("BinaryExpression.exp", "a ** b", Ctx::Expr, ES));
    v.push(p("BinaryExpression.instanceof", "a instanceof b", Ctx::Expr, ES));
    v.push(p("UnaryExpression.minus", "-a", Ctx::Expr, ES));
    v.push(p("UnaryExpression.typeof", "typeof a", Ctx::Expr, ES));
    v.push(p("UnaryExpression.void", "void a", Ctx::Expr, ES));
    v.push(p("UnaryExpression.delete", "delete a.b", Ctx::Expr, ES));
    v.push(p("UnaryExpression.not", "!a", Ctx::Expr, ES));
    v.push(p("UpdateExpression.prefix", "++a", Ctx::Expr, ES));
    v.push(p("UpdateExpression.postfix", "a++", Ctx::Expr, ES));
    v.push(p("NewExpression.args", "new C(1)", Ctx::Expr, ES));
    v.push(p("NewExpression.noargs", "new C", Ctx::Expr, ES));
    v.push(p("CallExpression", "f(1)", Ctx::Expr, ES));
    v.push(p("MemberExpression.dot", "a.b", Ctx::Expr, ES).target());
    v.push(p("MemberExpression.computed", "a[b]", Ctx::Expr, ES).target());
    v.push(p("OptionalMemberExpression", "a?.b", Ctx::Expr, ES));
    v.push(p("OptionalCallExpression", "a?.(1)", Ctx::Expr, ES));
    v.push(p("YieldExpression", "yield a", Ctx::Expr, ES).needs_yield());
    v.push(p("YieldExpression.delegate", "yield* a", Ctx::Expr, ES).needs_yield());
    v.push(p("AwaitExpression", "await a", Ctx::Expr, ES).needs_await());
    v.push(p("SpreadElement", "...a", Ctx::Expr, ES).spread());
    v.push(p("MetaProperty.newtarget", "new.target", Ctx::Expr, ES).needs_function());
    v.push(p("MetaProperty.importmeta", "import.meta", Ctx::Expr, ES));
    v.push(p("Super.member", "super.b", Ctx::Expr, ES).needs_super().target());
    v.push(p("ImportExpression", "import(\"m\")", Ctx::Expr, ES));
    v.push(p("PrivateName", "#x", Ctx::Expr, ES).private_name());

    // --- JSX expressions --------------------------------------------------
    v.push(p("JSXElement", "<a/>", Ctx::Expr, JSX));
    v.push(p("JSXFragment", "<></>", Ctx::Expr, JSX));

    // --- Flow expressions -------------------------------------------------
    v.push(p("TypeCastExpression", "(x: number)", Ctx::Expr, FLOW));
    v.push(p("AsExpression", "x as T", Ctx::Expr, FLOW));
    v.push(p("AsConstExpression", "x as const", Ctx::Expr, FLOW));
    v.push(p(
        "MatchExpression",
        "match (x) { _ => 1 }",
        Ctx::Expr,
        FLOW_MATCH,
    ));

    // --- TypeScript expressions -------------------------------------------
    v.push(p("TSAsExpression", "x as T", Ctx::Expr, TS));
    v.push(p("TSTypeAssertion", "<T>x", Ctx::Expr, TS));
    // No `TSNonNullExpression` payload: `x!` is not in this AST at all.
    // Grepping the whole tree for the name finds no `ESTree.def` entry, no
    // parser site and no C++ site — it is a TypeScript construct this
    // front end does not model. An earlier draft of this table had one, and
    // the "every payload must take part in a live probe" assertion at the
    // bottom of this file is what caught it.

    // --- Flow types -------------------------------------------------------
    for (name, src) in [
        ("AnyTypeAnnotation", "any"),
        ("MixedTypeAnnotation", "mixed"),
        ("EmptyTypeAnnotation", "empty"),
        ("VoidTypeAnnotation", "void"),
        ("NullLiteralTypeAnnotation", "null"),
        ("NumberTypeAnnotation", "number"),
        ("StringTypeAnnotation", "string"),
        ("BooleanTypeAnnotation", "boolean"),
        ("SymbolTypeAnnotation", "symbol"),
        ("BigIntTypeAnnotation", "bigint"),
        ("ExistsTypeAnnotation", "*"),
        ("GenericTypeAnnotation", "A"),
        ("GenericTypeAnnotation.args", "A<B>"),
        ("QualifiedTypeIdentifier", "A.B"),
        ("NullableTypeAnnotation", "?A"),
        ("ArrayTypeAnnotation", "A[]"),
        ("TupleTypeAnnotation", "[A, B]"),
        ("UnionTypeAnnotation", "A | B"),
        ("IntersectionTypeAnnotation", "A & B"),
        ("FunctionTypeAnnotation.anon", "A => B"),
        ("FunctionTypeAnnotation.named", "(a: A) => B"),
        ("FunctionTypeAnnotation.empty", "() => B"),
        ("ObjectTypeAnnotation", "{ p: A }"),
        ("ObjectTypeAnnotation.exact", "{| p: A |}"),
        ("ObjectTypeAnnotation.indexer", "{ [K]: A }"),
        ("InterfaceTypeAnnotation", "interface { p: A }"),
        ("TypeofTypeAnnotation", "typeof x"),
        ("IndexedAccessType", "A[K]"),
        ("OptionalIndexedAccessType", "A?.[K]"),
        ("KeyofTypeAnnotation", "keyof A"),
        ("ConditionalTypeAnnotation", "A extends B ? C : D"),
        ("StringLiteralTypeAnnotation", "\"s\""),
        ("NumberLiteralTypeAnnotation", "1"),
        ("BooleanLiteralTypeAnnotation", "true"),
        ("BigIntLiteralTypeAnnotation", "1n"),
    ] {
        v.push(p(name, src, Ctx::FlowType, FLOW));
    }
    v.push(p("TypeOperator.renders", "renders A", Ctx::FlowType, FLOW_COMPONENT));

    // --- TypeScript types -------------------------------------------------
    for (name, src) in [
        ("TSAnyKeyword", "any"),
        ("TSUnknownKeyword", "unknown"),
        ("TSNeverKeyword", "never"),
        ("TSUndefinedKeyword", "undefined"),
        ("TSBigIntKeyword", "bigint"),
        ("TSVoidKeyword", "void"),
        ("TSNullKeyword", "null"),
        ("TSNumberKeyword", "number"),
        ("TSStringKeyword", "string"),
        ("TSBooleanKeyword", "boolean"),
        ("TSSymbolKeyword", "symbol"),
        ("TSObjectKeyword", "object"),
        ("TSTypeReference", "A"),
        ("TSTypeReference.args", "A<B>"),
        ("TSQualifiedName", "A.B"),
        ("TSArrayType", "A[]"),
        ("TSTupleType", "[A, B]"),
        ("TSUnionType", "A | B"),
        ("TSIntersectionType", "A & B"),
        ("TSFunctionType", "(a: A) => B"),
        ("TSConstructorType", "new (a: A) => B"),
        ("TSTypeLiteral", "{ p: A }"),
        ("TSTypeQuery", "typeof x"),
        ("TSIndexedAccessType", "A[K]"),
        ("TSConditionalType", "A extends B ? C : D"),
        ("TSLiteralType.string", "\"s\""),
        ("TSLiteralType.number", "1"),
        ("TSLiteralType.boolean", "true"),
        // No `TSTypeOperator` payload: `keyof T` / `readonly T[]` are not in
        // this AST. Grepping `ESTree.def`, `crates/ast/src/node.rs` and the
        // whole parser for the name finds nothing, and `type Q = keyof A;`
        // is a parse error under `-parse-ts`. (Flow's `keyof` *is*
        // modelled — `KeyofTypeAnnotation` — and is in the Flow payload
        // list above.) An earlier draft of this table had one; the
        // "every payload parses in its canonical context" assertion in
        // `measure_paren_behavior` is what caught it.
    ] {
        v.push(p(name, src, Ctx::TsType, TS));
    }

    // --- Flow `match` patterns -------------------------------------------
    for (name, src) in [
        ("MatchWildcardPattern", "_"),
        ("MatchLiteralPattern.number", "1"),
        ("MatchLiteralPattern.string", "\"s\""),
        ("MatchLiteralPattern.boolean", "true"),
        ("MatchUnaryPattern", "-1"),
        ("MatchIdentifierPattern", "y"),
        ("MatchMemberPattern", "a.b"),
        ("MatchBindingPattern", "const y"),
        ("MatchObjectPattern", "{a: 1}"),
        ("MatchArrayPattern", "[1]"),
        ("MatchOrPattern", "1 | 2"),
        ("MatchAsPattern", "1 as y"),
    ] {
        v.push(p(name, src, Ctx::MatchPattern, FLOW_MATCH));
    }

    v
}

// ===========================================================================
// Edges
// ===========================================================================

/// One `print_child` call site, as a source template with a `%s` hole.
#[derive(Clone)]
struct Edge {
    /// `ParentKind.field` (plus a disambiguator where one field has several
    /// interesting spellings), matching the `print_child` call site.
    name: &'static str,
    /// The statement, with `%s` where the (parenthesized) payload goes.
    template: &'static str,
    /// Which payloads this hole accepts.
    ctx: Ctx,
    /// The dialect the template itself needs.
    dialect: Dialect,
    /// Which frames the template can be placed in.
    frames: &'static [Frame],
    /// The hole is an assignment/update target position.
    target: bool,
    /// The hole accepts a `SpreadElement`.
    accepts_spread: bool,
    /// The hole accepts a bare `PrivateName` (only `in`'s left operand).
    accepts_private_name: bool,
    /// The template spells TypeScript's angle-bracket type assertion
    /// (`<T>expr`), which cannot coexist with JSX. See
    /// [`Skip::AngleBracketAssertionUnderJsx`].
    angle_bracket: bool,
    /// Reaching the hole crosses into a non-async, non-generator function
    /// body, so the frame's `yield`/`await` do not reach it.
    ///
    /// ECMA-262's `ConciseBody[In] : ExpressionBody[?In, ~Await]` — an
    /// arrow body is `[~Await]` and carries no `[Yield]` parameter at all,
    /// so `async function f() { const g = () => await 1; }` is a
    /// SyntaxError even though the arrow is lexically inside an async
    /// function.
    fn_boundary: bool,
}

/// Both frames — the default for an expression edge.
const BOTH_FRAMES: &[Frame] = &[Frame::Script, Frame::Method];
/// Script top level only.
const SCRIPT_ONLY: &[Frame] = &[Frame::Script];

/// Start an edge with every tag off.
fn e(
    name: &'static str,
    template: &'static str,
    ctx: Ctx,
    dialect: Dialect,
    frames: &'static [Frame],
) -> Edge {
    Edge {
        name,
        template,
        ctx,
        dialect,
        frames,
        target: false,
        accepts_spread: false,
        accepts_private_name: false,
        angle_bracket: false,
        fn_boundary: false,
    }
}

impl Edge {
    /// Mark the hole as an assignment/update target position.
    fn target(mut self) -> Self {
        self.target = true;
        self
    }
    /// Mark the hole as accepting a `SpreadElement`.
    fn accepts_spread(mut self) -> Self {
        self.accepts_spread = true;
        self
    }
    /// Mark the hole as accepting a bare `PrivateName`.
    fn accepts_private_name(mut self) -> Self {
        self.accepts_private_name = true;
        self
    }
    /// Mark the template as using angle-bracket type-assertion syntax.
    fn angle_bracket(mut self) -> Self {
        self.angle_bracket = true;
        self
    }
    /// Mark the hole as living behind a nested function boundary.
    fn fn_boundary(mut self) -> Self {
        self.fn_boundary = true;
        self
    }
}

/// Every parent → child edge at which `precedence.rs`'s `print_child` (or
/// `print_comma_expression`) is called, as a template.
///
/// The list is derived mechanically from the 78 `self.print_child(` /
/// `self.print_comma_expression(` call sites in `src/`, one entry per
/// (enclosing arm, `NodeField`, `ChildPos`) triple, plus a second entry
/// wherever a list field has a meaningfully different first and last
/// position (`SequenceExpression.expressions`, `UnionTypeAnnotation.types`,
/// …) or an operator whose associativity is itself the hazard (`**`, `in`).
fn edges() -> Vec<Edge> {
    let mut v = Vec::new();

    // --- ES expressions ---------------------------------------------------
    v.push(e("SequenceExpression.expressions[0]", "t = (%s, 0);", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("SequenceExpression.expressions[n]", "t = (0, %s);", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("ArrayExpression.elements", "t = [%s];", Ctx::Expr, ES, BOTH_FRAMES).accepts_spread());
    v.push(e("NewExpression.callee", "t = new %s();", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(
        e("NewExpression.arguments", "t = new C(%s);", Ctx::Expr, ES, BOTH_FRAMES).accepts_spread(),
    );
    v.push(e("YieldExpression.argument", "t = yield %s;", Ctx::Expr, ES, &[Frame::Method]));
    v.push(e("AwaitExpression.argument", "t = await %s;", Ctx::Expr, ES, &[Frame::Method]));
    v.push(e("CallExpression.callee", "t = %s();", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("CallExpression.arguments", "t = f(%s);", Ctx::Expr, ES, BOTH_FRAMES).accepts_spread());
    v.push(e("OptionalCallExpression.callee", "t = %s?.();", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(
        e("OptionalCallExpression.arguments", "t = f?.(%s);", Ctx::Expr, ES, BOTH_FRAMES)
            .accepts_spread(),
    );
    v.push(e("AssignmentExpression.left", "%s = y;", Ctx::Expr, ES, BOTH_FRAMES).target());
    v.push(e("AssignmentExpression.right", "t = %s;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("UnaryExpression.argument.minus", "t = -%s;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("UnaryExpression.argument.typeof", "t = typeof %s;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("UnaryExpression.argument.delete", "t = delete %s;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("UpdateExpression.argument.prefix", "++%s;", Ctx::Expr, ES, BOTH_FRAMES).target());
    v.push(e("UpdateExpression.argument.postfix", "%s++;", Ctx::Expr, ES, BOTH_FRAMES).target());
    v.push(e("MemberExpression.object", "t = %s.p;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("MemberExpression.property", "t = a[%s];", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("OptionalMemberExpression.object", "t = %s?.p;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("OptionalMemberExpression.property", "t = a?.[%s];", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("BinaryExpression.left.add", "t = %s + b;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("BinaryExpression.right.add", "t = a + %s;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("BinaryExpression.left.exp", "t = %s ** b;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("BinaryExpression.right.exp", "t = a ** %s;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(
        e("BinaryExpression.left.in", "t = %s in b;", Ctx::Expr, ES, BOTH_FRAMES)
            .accepts_private_name(),
    );
    v.push(e("BinaryExpression.right.in", "t = a in %s;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("ConditionalExpression.test", "t = %s ? b : c;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("ConditionalExpression.consequent", "t = a ? %s : c;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("ConditionalExpression.alternate", "t = a ? b : %s;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("LogicalExpression.left.or", "t = %s || b;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("LogicalExpression.right.or", "t = a || %s;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("LogicalExpression.right.nullish", "t = a ?? %s;", Ctx::Expr, ES, BOTH_FRAMES));
    // A left operand immediately followed by a `==` — the one edge that
    // exercises the token-adjacency hazard `gen.rs`'s
    // `separate_from_equals` guards (a self-closing JSX tag's `>` merging
    // with the following `=` in `Pretty::No`).
    v.push(e("BinaryExpression.left.equals", "t = %s == b;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(
        e("ArrowFunctionExpression.body", "t = () => %s;", Ctx::Expr, ES, BOTH_FRAMES).fn_boundary(),
    );
    v.push(e("TaggedTemplateExpression.tag", "t = %s`q`;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("ExpressionStatement.expression", "%s;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("VariableDeclarator.init", "var t = %s;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("ForStatement.init", "for (%s;;) break;", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("ClassDeclaration.superClass", "class K extends %s {}", Ctx::Expr, ES, BOTH_FRAMES));
    v.push(e("ExportDefaultDeclaration.declaration", "export default %s;", Ctx::Expr, ES, SCRIPT_ONLY));

    // --- Flow expressions -------------------------------------------------
    v.push(e("TypeCastExpression.expression", "t = (%s: number);", Ctx::Expr, FLOW, BOTH_FRAMES));
    v.push(e("AsExpression.expression", "t = %s as T;", Ctx::Expr, FLOW, BOTH_FRAMES));
    v.push(e("AsConstExpression.expression", "t = %s as const;", Ctx::Expr, FLOW, BOTH_FRAMES));
    v.push(e(
        "MatchExpressionCase.body",
        "t = match (x) { _ => %s };",
        Ctx::Expr,
        FLOW_MATCH,
        BOTH_FRAMES,
    ));

    // --- TypeScript expressions -------------------------------------------
    v.push(e("TSAsExpression.expression", "t = %s as T;", Ctx::Expr, TS, BOTH_FRAMES));
    v.push(
        e("TSTypeAssertion.expression", "t = <T>%s;", Ctx::Expr, TS, BOTH_FRAMES).angle_bracket(),
    );
    v.push(e("TSEnumMember.initializer", "enum E { A = %s }", Ctx::Expr, TS, SCRIPT_ONLY));

    // --- Flow types -------------------------------------------------------
    v.push(e("NullableTypeAnnotation.typeAnnotation", "type Q = ?%s;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("ArrayTypeAnnotation.elementType", "type Q = %s[];", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("UnionTypeAnnotation.types[0]", "type Q = %s | A;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("UnionTypeAnnotation.types[n]", "type Q = A | %s;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("IntersectionTypeAnnotation.types[0]", "type Q = %s & A;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("IntersectionTypeAnnotation.types[n]", "type Q = A & %s;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("IndexedAccessType.objectType", "type Q = %s[K];", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("OptionalIndexedAccessType.objectType", "type Q = %s?.[K];", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("ConditionalTypeAnnotation.checkType", "type Q = %s extends A ? B : C;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("ConditionalTypeAnnotation.extendsType", "type Q = A extends %s ? B : C;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("ConditionalTypeAnnotation.trueType", "type Q = A extends B ? %s : C;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("ConditionalTypeAnnotation.falseType", "type Q = A extends B ? C : %s;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("KeyofTypeAnnotation.argument", "type Q = keyof %s;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("TypeOperator.typeAnnotation", "type Q = renders %s;", Ctx::FlowType, FLOW_COMPONENT, SCRIPT_ONLY));
    v.push(e("TypePredicate.typeAnnotation", "function g(x: mixed): x is %s { return true; }", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("ArrowFunctionExpression.returnType", "var f = (x: mixed): %s => 1;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("TypeCastExpression.typeAnnotation", "t = (x: %s);", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("TypeAnnotation.typeAnnotation", "var v: %s;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    // The three type positions immediately followed by a `=`: the other
    // half of the `separate_from_equals` hazard, where Flow's
    // `ExistsTypeAnnotation` (`*`) would otherwise be run together with the
    // initializer's `=` in `Pretty::No`.
    v.push(e("TypeAnnotation.typeAnnotation.init", "var v: %s = 1;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("ClassProperty.typeAnnotation", "class K { p: %s = 1; }", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("AssignmentPattern.left.typeAnnotation", "function g(a: %s = 1) {}", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("FunctionTypeAnnotation.returnType", "type Q = () => %s;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("TupleTypeAnnotation.types", "type Q = [%s];", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("TypeParameter.bound", "type Q<T: %s> = T;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("TypeParameterInstantiation.params", "type Q = A<%s>;", Ctx::FlowType, FLOW, SCRIPT_ONLY));
    v.push(e("ObjectTypeProperty.value", "type Q = { p: %s };", Ctx::FlowType, FLOW, SCRIPT_ONLY));

    // --- TypeScript types -------------------------------------------------
    v.push(e("TSArrayType.elementType", "type Q = %s[];", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSIndexedAccessType.objectType", "type Q = %s[K];", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSIndexedAccessType.indexType", "type Q = A[%s];", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSFunctionType.returnType", "type Q = () => %s;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSTypePredicate.typeAnnotation", "function g(x: any): x is %s { return true; }", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSTupleType.elementTypes", "type Q = [%s];", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSUnionType.types[0]", "type Q = %s | A;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSUnionType.types[n]", "type Q = A | %s;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSIntersectionType.types[0]", "type Q = %s & A;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSIntersectionType.types[n]", "type Q = A & %s;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSConditionalType.checkType", "type Q = %s extends A ? B : C;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSConditionalType.extendsType", "type Q = A extends %s ? B : C;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSConditionalType.trueType", "type Q = A extends B ? %s : C;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSConditionalType.falseType", "type Q = A extends B ? C : %s;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSTypeParameterInstantiation.params", "type Q = A<%s>;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSTypeParameter.constraint", "type Q<T extends %s> = T;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSTypeParameter.default", "type Q<T = %s> = T;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSAsExpression.typeAnnotation", "t = x as %s;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSTypeAssertion.typeAnnotation", "t = <%s>x;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSTypeAliasDeclaration.typeAnnotation", "type Q = %s;", Ctx::TsType, TS, SCRIPT_ONLY));
    v.push(e("TSTypeAnnotation.typeAnnotation", "var v: %s;", Ctx::TsType, TS, SCRIPT_ONLY));

    // --- Flow `match` patterns -------------------------------------------
    v.push(e("MatchAsPattern.pattern", "t = match (x) { %s as y => 1 };", Ctx::MatchPattern, FLOW_MATCH, SCRIPT_ONLY));
    v.push(e("MatchOrPattern.patterns[0]", "t = match (x) { %s | 2 => 1 };", Ctx::MatchPattern, FLOW_MATCH, SCRIPT_ONLY));
    v.push(e("MatchOrPattern.patterns[n]", "t = match (x) { 2 | %s => 1 };", Ctx::MatchPattern, FLOW_MATCH, SCRIPT_ONLY));

    v
}

// ===========================================================================
// Measured parenthesizability
// ===========================================================================

/// What happens when a payload is wrapped in `( … )`.
///
/// **Measured, not declared.** The matrix's whole premise is that wrapping
/// the payload in parens puts the raw parent → child edge in the tree, and
/// that premise is false for some payloads — for reasons that are properties
/// of *the parser*, not of this test. Declaring which ones by hand would be
/// the string skip list this file exists to avoid, and would go stale
/// silently the day the parser changed. So [`measure_paren_behavior`] probes
/// each payload once, in a canonical context, and the matrix wraps only the
/// ones it measures as [`ParenBehavior::Transparent`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ParenBehavior {
    /// `( payload )` parses, and (in a type context) denotes the same tree
    /// as the bare spelling. The matrix wraps it.
    Transparent,
    /// `( payload )` does not parse in this context at all, so no source
    /// spells this child parenthesized. The matrix runs the payload bare.
    Rejected,
    /// `( payload )` parses but denotes a **different tree** than the bare
    /// spelling. Only distinguishable in a type context, where a `( Type )`
    /// group builds no node (Flow: `parseFunctionOrGroupTypeAnnotationFlow`
    /// returns the inner type unwrapped; TS: likewise), so any difference
    /// is the parser classifying the same tokens two ways. The matrix runs
    /// the payload bare.
    ChangesTree,
}

/// The canonical single-statement context for `text` in `ctx`, used only by
/// [`measure_paren_behavior`].
///
/// The expression context is the `Frame::Method` body so that `await`,
/// `yield`, `super` and `new.target` are all in scope; the two type
/// contexts are a bare type alias, which is a *full* type slot in both
/// dialects (`parse_type_annotation_flow` / `parse_type_annotation_ts`), so
/// nothing there can re-associate across the parens.
fn canonical(ctx: Ctx, text: &str) -> String {
    match ctx {
        Ctx::Expr => Frame::Method.wrap(&format!("t = {text};")),
        Ctx::FlowType | Ctx::TsType => format!("type Q = {text};"),
        Ctx::MatchPattern => format!("t = match (x) {{ {text} => 1 }};"),
    }
}

/// Measure whether `payload` can be parenthesized at all, and whether doing
/// so changes the tree.
///
/// Panics if the payload does not parse even *bare* in its canonical
/// context: that is a bug in the payload table, not a property of the
/// parser, and it must not be silently absorbed.
fn measure_paren_behavior(payload: &Payload) -> ParenBehavior {
    let flags = payload.dialect.flags();
    let bare = canonical(payload.ctx, payload.src);
    let wrapped = canonical(payload.ctx, &format!("({})", payload.src));
    let mut bare_parsed = hermes_parser::parse(&bare, flags).unwrap_or_else(|e| {
        panic!(
            "payload {:?} ({:?}) does not parse in its canonical context \
             {bare:?}: {e:?} — fix the payload table",
            payload.name, payload.src
        )
    });
    let Ok(mut wrapped_parsed) = hermes_parser::parse(&wrapped, flags) else {
        return ParenBehavior::Rejected;
    };
    // Only a type context can answer the "different tree" question: in an
    // expression context parens legitimately re-associate (`t = (a, b);` is
    // a different tree from `t = a, b;`), which is exactly what the matrix
    // is testing rather than a reason to exclude anything.
    if matches!(payload.ctx, Ctx::FlowType | Ctx::TsType)
        && ast_json(&mut bare_parsed) != ast_json(&mut wrapped_parsed)
    {
        return ParenBehavior::ChangesTree;
    }
    ParenBehavior::Transparent
}

// ===========================================================================
// Skip classification
// ===========================================================================

/// Why a generated triple's **source** does not parse.
///
/// Every variant is decided by [`predict_skip`] from the tags on the frame,
/// the edge and the payload — never from the text of the probe. A probe
/// that fails to parse and matches no rule is an unclassified skip and
/// fails the test.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Skip {
    /// The hole is an assignment/update target and the payload is not a
    /// valid one (`(a + b) = y`).
    NotAnAssignmentTarget,
    /// A `SpreadElement` outside an argument/element list.
    SpreadOutsideList,
    /// A bare `PrivateName` outside `in`'s left operand.
    PrivateNameOutsideIn,
    /// `yield` with no enclosing generator, or behind a nested function
    /// boundary.
    NoYieldInScope,
    /// `await` with no enclosing async function, or behind a nested
    /// function boundary.
    NoAwaitInScope,
    /// `super.x` with no home object.
    NoSuperInScope,
    /// `new.target`/`arguments` outside a function.
    NoFunctionInScope,
    /// A JSX payload under an edge that uses TypeScript's angle-bracket
    /// type-assertion syntax (`<T>expr`). The two grammars compete for `<`
    /// — this is the same reason `.tsx` files may not use `<T>expr` — so
    /// `t = <T>(<a/>);` is not a program in any dialect.
    AngleBracketAssertionUnderJsx,
    /// The payload's parenthesized spelling is not available (measured:
    /// [`ParenBehavior::Rejected`] or [`ParenBehavior::ChangesTree`]), so
    /// the probe had to use the bare spelling — and the bare spelling does
    /// not fit this particular hole.
    ///
    /// The only members today are TypeScript's `TSConstructorType` as the
    /// last member of a union or an intersection
    /// (`type Q = A | new (a: A) => B;`): the parenthesized spelling the
    /// grammar wants there is exactly what the parser's `(`-cover refuses.
    /// This is the *last* rule tried, so any more specific reason wins.
    UnparenthesizableInThisHole,
}

/// Predict why `(frame, edge, payload)` will not parse, or `None` if it is
/// expected to.
///
/// Rules are tried in the order written and the first match wins; the order
/// is only cosmetic (it decides which bucket a doubly-illegal triple lands
/// in), never a correctness question, because an unpredicted parse failure
/// fails the test regardless of which rule would have caught it.
fn predict_skip(
    frame: Frame,
    edge: &Edge,
    payload: &Payload,
    dialect: Dialect,
    behavior: ParenBehavior,
) -> Option<Skip> {
    if edge.angle_bracket && dialect.jsx {
        return Some(Skip::AngleBracketAssertionUnderJsx);
    }
    if payload.spread && !edge.accepts_spread {
        return Some(Skip::SpreadOutsideList);
    }
    if payload.private_name && !edge.accepts_private_name {
        return Some(Skip::PrivateNameOutsideIn);
    }
    if edge.target && !payload.target {
        return Some(Skip::NotAnAssignmentTarget);
    }
    if payload.needs_yield && (!frame.has_yield() || edge.fn_boundary) {
        return Some(Skip::NoYieldInScope);
    }
    if payload.needs_await && (!frame.has_await() || edge.fn_boundary) {
        return Some(Skip::NoAwaitInScope);
    }
    if payload.needs_super && !frame.has_super() {
        return Some(Skip::NoSuperInScope);
    }
    if payload.needs_function && !frame.has_function() {
        return Some(Skip::NoFunctionInScope);
    }
    if behavior != ParenBehavior::Transparent {
        return Some(Skip::UnparenthesizableInThisHole);
    }
    None
}

// ===========================================================================
// The oracle
// ===========================================================================

/// The round-trip oracle: the ESTree dump with `"raw"` omitted and no
/// locations — identical to `tests/corpus.rs`'s and to juno's ported cases'.
///
/// `"raw"` is the verbatim source spelling of a numeric literal, which no
/// generator can preserve when it reprints from the `f64` value. Everything
/// else — every kind, every field, the whole shape — is compared byte for
/// byte.
fn ast_json(parsed: &mut ParsedJS) -> String {
    parsed.to_estree_json_with(
        true,
        ESTreeDumpMode::HideEmpty,
        LocationDumpMode::None,
        ESTreeRawProp::Exclude,
    )
}

/// Generate `parsed` under `pretty`.
fn gen(parsed: &mut ParsedJS, pretty: Pretty) -> Result<String, String> {
    let mut out = Vec::new();
    let res = parsed.with_program(|gc, root| {
        generate(
            &mut out,
            gc,
            root,
            Opt {
                pretty,
                ..Opt::default()
            },
        )
    });
    match res {
        Ok(()) => String::from_utf8(out).map_err(|e| format!("non-UTF-8 output: {e}")),
        Err(e) => Err(format!("generation failed: {e:?}")),
    }
}

/// Collects the distinct (parent kind → child kind) pairs of a tree.
///
/// This is the measurement that makes the matrix's reach comparable to the
/// Tier 1 corpus's: the corpus contains 40 distinct parenthesized edges,
/// and every pair counted here came from a child that **was** parenthesized
/// in the probe's source.
struct EdgeWalk {
    /// The chain of ancestors of the node being visited.
    stack: Vec<NodeKind>,
    /// Every (parent, child) pair seen.
    pairs: HashSet<(NodeKind, NodeKind)>,
}

impl<'gc> Visitor<'gc> for EdgeWalk {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        if let Some(parent) = self.stack.last() {
            self.pairs.insert((*parent, node.kind()));
        }
        self.stack.push(node.kind());
        node.visit_children(self);
        self.stack.pop();
    }
}

// ===========================================================================
// The matrix
// ===========================================================================

/// Everything one run of the matrix measured.
#[derive(Default)]
struct Matrix {
    /// Triples instantiated and parsed.
    probed: usize,
    /// Round trips performed (two per parsing probe).
    round_trips: usize,
    /// Probes whose source parsed.
    live: usize,
    /// Probes whose source did not parse, by predicted reason.
    skips: BTreeMap<Skip, usize>,
    /// Probes whose source did not parse with **no** predicted reason.
    unclassified: Vec<String>,
    /// Probes whose source was predicted not to parse but did.
    ///
    /// Not an error: [`predict_skip`] is allowed to be conservative, and a
    /// triple that parses is simply round-tripped like any other. Counted
    /// so the report says how tight the prediction is.
    predicted_but_parsed: usize,
    /// Round-trip failures, formatted for the assertion message.
    failures: Vec<String>,
    /// Names of edges that took part in at least one live probe.
    live_edges: HashSet<&'static str>,
    /// Names of payloads that took part in at least one live probe.
    live_payloads: HashSet<&'static str>,
    /// Distinct (parent kind → child kind) pairs across every live probe.
    pairs: HashSet<(NodeKind, NodeKind)>,
    /// (edge, payload) pairs skipped because their dialects cannot combine.
    dialect_conflicts: usize,
    /// Payloads whose parenthesized spelling the parser rejects outright.
    /// Measured, not declared; see [`ParenBehavior::Rejected`].
    paren_rejected: Vec<&'static str>,
    /// Payloads whose parenthesized spelling denotes a different tree.
    /// Measured, not declared; see [`ParenBehavior::ChangesTree`].
    paren_changes_tree: Vec<&'static str>,
    /// Probes whose source parsed to a tree holding a cover-grammar node —
    /// not a program, so outside the generator's domain. See
    /// [`CoverFinder`].
    cover_grammar: usize,
}

/// Run one triple, updating `m`.
fn probe(
    m: &mut Matrix,
    frame: Frame,
    edge: &Edge,
    payload: &Payload,
    dialect: Dialect,
    behavior: ParenBehavior,
) {
    let child = if payload.parenthesizable && behavior == ParenBehavior::Transparent {
        format!("({})", payload.src)
    } else {
        payload.src.to_string()
    };
    let src = frame.wrap(&edge.template.replace("%s", &child));
    let flags = dialect.flags();
    m.probed += 1;

    // Our parser has at least one known panic on legal input
    // (`match(x){_=>1};` — `crates/parser/src/js/statements.rs:1196`), and
    // the whole point of this matrix is to reach shapes nothing else does.
    // A panic in one triple must be reported as that triple's failure, not
    // take the other several thousand down with it.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_probe(&src, flags)
    }));
    let outcome = match outcome {
        Ok(o) => o,
        Err(e) => {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "<non-string panic payload>".to_string());
            m.failures.push(format!(
                "{} / {} / {frame:?}: PANIC on {src:?}: {msg}",
                edge.name, payload.name
            ));
            return;
        }
    };

    match outcome {
        Outcome::DoesNotParse => match predict_skip(frame, edge, payload, dialect, behavior) {
            Some(reason) => *m.skips.entry(reason).or_insert(0) += 1,
            None => m.unclassified.push(format!(
                "{} / {} / {frame:?}: {src:?}",
                edge.name, payload.name
            )),
        },
        Outcome::CoverGrammar => m.cover_grammar += 1,
        Outcome::Live { pairs, failures } => {
            m.live += 1;
            m.round_trips += 2;
            if predict_skip(frame, edge, payload, dialect, behavior).is_some() {
                m.predicted_but_parsed += 1;
            }
            m.live_edges.insert(edge.name);
            m.live_payloads.insert(payload.name);
            m.pairs.extend(pairs);
            for f in failures {
                m.failures
                    .push(format!("{} / {} / {frame:?}: {f}", edge.name, payload.name));
            }
        }
    }
}

/// Finds cover-grammar nodes — the parser's placeholders for syntax legal
/// only inside arrow parameters or a destructuring target, left in the tree
/// for sema to reject.
///
/// Copied from `tests/corpus.rs`'s `CoverFinder`, and here for the same
/// reason: a tree holding one of these **is not a JavaScript program**, and
/// the generator's documented domain excludes the five kinds (5 of the 7
/// that report `GenJsError::UnsupportedKind` by design, `src/dispatch.rs`,
/// spec §4). The matrix reaches them because `t = (0, ...a);` and
/// `t = (...a: number);` parse — as a `CoverRestElement` — and "generation
/// refused" is the correct outcome there, not a failure.
///
/// Detected structurally, never by name, so a future payload with the same
/// shape is classified the same way.
#[derive(Default)]
struct CoverFinder {
    /// The first cover kind seen.
    found: Option<&'static str>,
}

impl<'gc> Visitor<'gc> for CoverFinder {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        let name = match node {
            Node::CoverEmptyArgs(_) => Some("CoverEmptyArgs"),
            Node::CoverInitializer(_) => Some("CoverInitializer"),
            Node::CoverRestElement(_) => Some("CoverRestElement"),
            Node::CoverTrailingComma(_) => Some("CoverTrailingComma"),
            Node::CoverTypedIdentifier(_) => Some("CoverTypedIdentifier"),
            _ => None,
        };
        if name.is_some() && self.found.is_none() {
            self.found = name;
        }
        node.visit_children(self);
    }
}

/// What one probe's source turned out to be.
enum Outcome {
    /// The source is not a legal program under its dialect.
    DoesNotParse,
    /// The source parsed, but its tree holds a cover-grammar node, so it is
    /// not a program. See [`CoverFinder`].
    CoverGrammar,
    /// The source parsed; here is what its tree contained and what the two
    /// round trips found.
    Live {
        /// The tree's (parent kind → child kind) pairs.
        pairs: HashSet<(NodeKind, NodeKind)>,
        /// One string per round-trip failure.
        failures: Vec<String>,
    },
}

/// Parse `src`, then round-trip it in both [`Pretty`] modes.
fn run_probe(src: &str, flags: ParseFlags) -> Outcome {
    let mut parsed = match hermes_parser::parse(src, flags) {
        Ok(p) => p,
        Err(_) => return Outcome::DoesNotParse,
    };
    if parsed.with_program(|_gc, root| {
        let mut finder = CoverFinder::default();
        finder.visit_node(root);
        finder.found.is_some()
    }) {
        return Outcome::CoverGrammar;
    }
    let pairs = parsed.with_program(|_gc, root| {
        let mut w = EdgeWalk {
            stack: Vec::new(),
            pairs: HashSet::new(),
        };
        w.visit_node(root);
        w.pairs
    });
    let before = ast_json(&mut parsed);
    let mut failures = Vec::new();
    for pretty in [Pretty::Yes, Pretty::No] {
        let js = match gen(&mut parsed, pretty) {
            Ok(js) => js,
            Err(err) => {
                failures.push(format!("{src:?} [{pretty:?}]: {err}"));
                continue;
            }
        };
        match hermes_parser::parse(&js, flags) {
            Err(err) => failures.push(format!(
                "{src:?} [{pretty:?}] -> {:?} DOES NOT PARSE: {err:?}",
                js.trim()
            )),
            Ok(mut reparsed) => {
                if ast_json(&mut reparsed) != before {
                    failures.push(format!(
                        "{src:?} [{pretty:?}] -> {:?} DIFFERENT AST",
                        js.trim()
                    ));
                }
            }
        }
    }
    Outcome::Live { pairs, failures }
}

/// Run the whole matrix.
fn run_matrix() -> Matrix {
    let edges = edges();
    let payloads = payloads();
    let mut m = Matrix::default();
    // Measure each payload's parenthesizability once, up front; see
    // [`ParenBehavior`].
    let behaviors: Vec<ParenBehavior> = payloads
        .iter()
        .map(|p| {
            if p.parenthesizable {
                measure_paren_behavior(p)
            } else {
                ParenBehavior::Rejected
            }
        })
        .collect();
    for (payload, behavior) in payloads.iter().zip(&behaviors) {
        match behavior {
            ParenBehavior::Transparent => {}
            ParenBehavior::Rejected if !payload.parenthesizable => {}
            ParenBehavior::Rejected => m.paren_rejected.push(payload.name),
            ParenBehavior::ChangesTree => m.paren_changes_tree.push(payload.name),
        }
    }
    for edge in &edges {
        for (payload, &behavior) in payloads.iter().zip(&behaviors) {
            if payload.ctx != edge.ctx {
                continue;
            }
            let Some(dialect) = edge.dialect.merge(payload.dialect) else {
                m.dialect_conflicts += 1;
                continue;
            };
            for &frame in edge.frames {
                probe(&mut m, frame, edge, payload, dialect, behavior);
            }
        }
    }
    m
}

/// Run `f` on a thread with a 64 MiB stack.
///
/// Same reason as `tests/corpus.rs`'s `on_big_stack`: a debug-build
/// recursive-descent parse plus a recursive generator over several thousand
/// small programs is fine, but the ESTree dumper and the parser both recurse
/// and libtest gives a test thread only 2 MiB.
fn on_big_stack<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("the matrix panicked outside a probe")
}

/// The matrix itself. See the module doc comment.
#[test]
fn paren_matrix_every_edge_round_trips() {
    let m = on_big_stack(run_matrix);

    // Failures first: a broken round trip is the finding this file exists
    // for, and whichever assertion fires first is the only one anybody
    // reads.
    assert!(
        m.failures.is_empty(),
        "{} of {} round trips failed:\n{}",
        m.failures.len(),
        m.round_trips,
        m.failures.join("\n")
    );
    assert!(
        m.unclassified.is_empty(),
        "{} unclassified skips (source did not parse for a reason no rule in \
         `predict_skip` accounts for) — add a RULE, never a name:\n{}",
        m.unclassified.len(),
        m.unclassified.join("\n")
    );

    let skips: Vec<(Skip, usize)> = m.skips.iter().map(|(k, v)| (*k, *v)).collect();
    eprintln!(
        "paren matrix: {} probes ({} live, {} did not parse, {} cover-grammar, \
         {} dialect conflicts), {} round trips, {} distinct parent->child \
         pairs, {} predicted-skip but parsed\n  skips: {skips:?}",
        m.probed,
        m.live,
        m.probed - m.live - m.cover_grammar,
        m.cover_grammar,
        m.dialect_conflicts,
        m.round_trips,
        m.pairs.len(),
        m.predicted_but_parsed,
    );
    eprintln!(
        "  payloads whose parenthesized spelling the parser REJECTS ({}): \
         {:?}\n  payloads whose parenthesized spelling denotes a DIFFERENT \
         tree ({}): {:?}",
        m.paren_rejected.len(),
        m.paren_rejected,
        m.paren_changes_tree.len(),
        m.paren_changes_tree,
    );

    // Every edge and every payload must have taken part in at least one
    // live probe: a table entry that never produces a program is dead
    // weight pretending to be coverage.
    let edges = edges();
    let dead_edges: Vec<&str> = edges
        .iter()
        .map(|e| e.name)
        .filter(|n| !m.live_edges.contains(n))
        .collect();
    assert!(
        dead_edges.is_empty(),
        "edges that never produced a parsing probe: {dead_edges:?}"
    );
    let payloads = payloads();
    let dead_payloads: Vec<&str> = payloads
        .iter()
        .map(|p| p.name)
        .filter(|n| !m.live_payloads.contains(n))
        .collect();
    assert!(
        dead_payloads.is_empty(),
        "payloads that never produced a parsing probe: {dead_payloads:?}"
    );

    // ---------------------------------------------------------------------
    // The pins. Everything below is a measurement of the run above, recorded
    // so the matrix cannot quietly shrink — which is the failure mode this
    // whole file is a reaction to. Any of these moving means the tables, the
    // parser or the generator changed; look at *which* one moved before
    // updating a number.
    // ---------------------------------------------------------------------
    assert_eq!(
        m.probed,
        edges
            .iter()
            .map(|e| e.frames.len()
                * payloads
                    .iter()
                    .filter(|p| p.ctx == e.ctx && e.dialect.merge(p.dialect).is_some())
                    .count())
            .sum::<usize>(),
        "probe count must be exactly the cross-product it claims to be"
    );
    assert_eq!(m.probed, 6788, "matrix size");
    assert_eq!(m.live, 6506, "probes whose source is a program");
    assert_eq!(m.round_trips, 13012, "two round trips per live probe");
    assert_eq!(m.round_trips, 2 * m.live);
    assert_eq!(m.cover_grammar, 4, "probes whose tree is not a program");
    assert_eq!(m.dialect_conflicts, 20, "Flow x TypeScript pairs, never generated");

    eprintln!("  tables: {} edges, {} payloads", edges.len(), payloads.len());
    let skips: Vec<(Skip, usize)> = m.skips.iter().map(|(k, v)| (*k, *v)).collect();
    assert_eq!(
        skips,
        vec![
            (Skip::NotAnAssignmentTarget, 6),
            (Skip::SpreadOutsideList, 82),
            (Skip::PrivateNameOutsideIn, 92),
            (Skip::NoYieldInScope, 47),
            (Skip::NoAwaitInScope, 45),
            (Skip::AngleBracketAssertionUnderJsx, 4),
            (Skip::UnparenthesizableInThisHole, 2),
        ],
        "the skip classification changed"
    );
    assert_eq!(
        m.skips.values().sum::<usize>(),
        m.probed - m.live - m.cover_grammar,
        "every non-live probe must be classified"
    );

    // The headline number, and the one directly comparable to the Tier 1
    // corpus gate: 420 real files contain 40 distinct *parenthesized*
    // (parent -> child) edges; this matrix puts 1985 distinct (parent ->
    // child) pairs into a tree, every one of them from a child the source
    // parenthesized.
    assert_eq!(m.pairs.len(), 1985, "distinct parent->child pairs reached");

    // Measured properties of the *parser*, pinned so that fixing either one
    // turns this test red and the manifest gets updated rather than going
    // stale. See `MANIFEST.md`'s "Parser defects" section: PD-1 is the
    // `reparseIdentifierAsTSTypeAnnotation` keyword-map gap (the four
    // `TS*Keyword` rows below), PD-2 is the TypeScript `(`-cover only
    // reaching the full type grammar for an identifier or a nested `(`
    // (every row of the first list, and `TSTupleType`/`TSTypeLiteral`,
    // which the cover hands back as an `ArrayPattern`/`ObjectPattern`).
    assert_eq!(
        m.paren_rejected,
        vec![
            "TSVoidKeyword",
            "TSNullKeyword",
            "TSTypeReference.args",
            "TSQualifiedName",
            "TSArrayType",
            "TSUnionType",
            "TSIntersectionType",
            "TSConstructorType",
            "TSTypeQuery",
            "TSIndexedAccessType",
            "TSConditionalType",
            "TSLiteralType.string",
            "TSLiteralType.number",
            "TSLiteralType.boolean",
        ],
        "payloads whose parenthesized spelling the parser rejects"
    );
    assert_eq!(
        m.paren_changes_tree,
        vec![
            "TSUnknownKeyword",
            "TSNeverKeyword",
            "TSUndefinedKeyword",
            "TSBigIntKeyword",
            "TSTupleType",
            "TSTypeLiteral",
        ],
        "payloads whose parenthesized spelling denotes a different tree"
    );
}
