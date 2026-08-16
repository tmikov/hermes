/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The precedence table and the parenthesization decision, `need_parens`.
//!
//! Ported from juno `gen_js.rs:110-215` (the precedence constants,
//! `get_binary_precedence`, `get_logical_precedence`, `ChildPos`,
//! `NeedParens`, `From<bool> for NeedParens`, `ForceSpace`),
//! `gen_js.rs:3589-3684` (`get_precedence`), `gen_js.rs:3685-3823`
//! (`need_parens`), `gen_js.rs:3824-3926` (`root_starts_with`,
//! `expr_starts_with`), `gen_js.rs:3249-3299` (`print_child`,
//! `print_comma_expression`, `print_parens`), and `gen_js.rs:4006-4174`
//! (`is_unary_op`, `is_binary_op`, `is_update_prefix`, `is_negative_number`,
//! `check_plus`, `check_minus`, `check_and_or`, `check_nullish`,
//! `stmt_skip_semi`, `contains_call`).
//!
//! This is the module the plan's Task 3 isolates on its own: a JS printer is
//! most often silently wrong here (spec §8's #1 review focus).
//!
//! `is_if_without_else` (juno `gen_js.rs:4049-4058`) is deliberately **not**
//! ported here: `need_parens`/`get_precedence`/`stmt_skip_semi` never call
//! it, only `IfStatement`'s own print arm does (`gen_js.rs:801`), which
//! belongs to a later task's `arms/stmt.rs`.
//!
//! # Two structural deviations from juno, both required by the port's own
//! shape, not by choice
//!
//! **1. Operator fields need `ctx` to classify, and classifying is
//! fallible.** juno's AST stores `BinaryExpression::operator` etc. as a
//! typed enum field (`juno_ast/src/node_enums.rs:31-92`); ours stores every
//! operator as a raw `NodeLabel` atom (`crates/ast/src/node_child.rs:12`) —
//! a `Cell`-wrapped interned string, resolved through a `GCLock`.
//! `crates::ast` has no typed operator enums at all (grep confirms zero
//! occurrences of `BinaryExpressionOperator` anywhere outside this file), so
//! this module defines its own — [`BinaryExpressionOperator`],
//! [`LogicalExpressionOperator`], [`UnaryExpressionOperator`],
//! [`UpdateExpressionOperator`] — with the exact variant set and spellings
//! juno's `juno_ast/src/node_enums.rs` uses, plus a `from_label(gc, label)`
//! classifier on each. Every function below that juno writes as inspecting
//! an operator therefore gains a `gc: &GCLock` parameter juno's equivalent
//! does not need: `get_precedence`, `need_parens`, `root_starts_with`,
//! `expr_starts_with`, `is_unary_op`, `is_binary_op`, `is_update_prefix`,
//! `check_plus`, `check_minus`, `check_and_or`, `check_nullish`.
//! `contains_call` loses a parameter in the other direction — see its doc
//! comment.
//!
//! `from_label` is also fallible — `Result<Self, GenJsError>`, erroring with
//! [`crate::GenJsError::UnknownOperator`] on any spelling outside the fixed
//! set — because [`crate::generate`]'s `Node` parameter is not required to
//! have come from this crate's parser (spec §4: a malformed input tree is
//! reported through `GenJsError`, never a panic). This was originally a
//! panicking "internal invariant" assumption; task-3 review (Important
//! finding) correctly pointed out that `BinaryExpression::new` and its
//! siblings are public constructors with no validation on `operator`, so a
//! hand-built or JSON-deserialized tree reaches the panic through this
//! crate's own public API, not hypothetically. Every function this ripples
//! through — the four `from_label`s, `is_unary_op`, `is_binary_op`,
//! `is_update_prefix`, `check_plus`, `check_minus`, `check_and_or`,
//! `check_nullish`, `get_precedence`, `need_parens`, `root_starts_with`, and
//! `expr_starts_with` (whose `pred` closures must themselves become
//! fallible, since every caller in this file builds one from `check_minus`
//! or `check_plus`) — now returns `Result` and propagates with `?`. See
//! `task-3-report.md`'s "Review round 1" addendum for the mechanical trace
//! and the regression test that fails without the fix.
//!
//! **2. `print_child`/`print_comma_expression`/`print_parens`/`get_precedence`/
//! `need_parens`/`root_starts_with`/`expr_starts_with` return `Result`**, not
//! juno's infallible types. juno's `child.visit(ctx, self, Some(path))`
//! cannot fail — an unknown kind is a runtime `unimplemented!()` panic
//! (design spec §1/§4). Ours dispatches through
//! [`crate::dispatch::GenJS::gen_node`], which is fallible by design (spec
//! §4's `GenJsError` replaces the panic), so every caller in the recursion
//! must propagate that `Result` — and, per deviation #1 above, several of
//! these functions are now *also* fallible for the independent reason that
//! they classify an operator.
//!
//! # A correctness fix folded into this port, not a verbatim copy
//!
//! juno's `get_precedence` returns `Assoc::Ltr` for **every** `BinaryExpression`
//! regardless of operator (`gen_js.rs:3645-3649`) — including `**`, which
//! ECMA-262 defines as right-associative (`ExponentiationExpression`).
//! Tracing `need_parens`'s equal-precedence branch (`gen_js.rs:3816-3821`)
//! through that `Ltr` for a `**`-chain: `(a ** b) ** c`'s *left* child, itself
//! `**`, lands on the *safe* side (`Ltr` ⇒ dangerous side is `Right`) and gets
//! **no** parens, printing `a ** b ** c` — which reparses, under `**`'s real
//! right-associative grammar, as `a ** (b ** c)`, a different value whenever
//! `a`, `b`, `c` aren't all equal. This is exactly the kind of silent
//! mis-parenthesization the design spec's §8 flags as the top review risk,
//! and it is squarely inside this task's cited range (`gen_js.rs:3645-3649`),
//! so it is fixed here rather than deferred: [`GenJS::get_precedence`]
//! returns `Assoc::Rtl` for `Exp` and `Assoc::Ltr` for every other binary
//! operator. See `task-3-report.md` for the full trace and the regression
//! tests below that fail without the fix.
//!
//! # A second correctness fix, found by Task 7 and applied here (this
//! module's own bug, not a transcription of one)
//!
//! `get_precedence`'s `match` had no arm for `Node::PrivateName(_)` at all,
//! so it fell into the catch-all `_ => (ALWAYS_PAREN, Assoc::Ltr)` — the
//! same bucket as a node kind this table has genuinely never classified
//! (e.g. a statement). `need_parens`'s very first check after every
//! special-cased branch is `if child_prec == ALWAYS_PAREN { return
//! Ok(NeedParens::Yes); }`, so *any* `PrivateName` reached through
//! `print_child` got wrapped in parens unconditionally. The one place a
//! `PrivateName` is ever a `print_child`'d child is `MemberExpression`'s
//! non-computed `property` (`this.#x`, `arms/expr.rs`'s
//! `gen_member_expression`) — `#x in obj`'s left operand and every class
//! member key print through plain `gen_node`, not `print_child`, so they
//! never hit this. The result: `this.#x` regenerated as `this.(#x)` — not
//! merely ugly, `.(` is a syntax error (`'identifier' expected after '.' or
//! '?.' in member expression`), a live round-trip break for the single most
//! common private-field-access shape there is. Confirmed empirically:
//! `tests/roundtrip.rs`'s `class_private_field_and_private_method_round_trip`
//! (Task 7) failed with exactly that reparse error before this fix.
//!
//! juno has the identical bug: its own `get_precedence`
//! (`gen_js.rs:3589-3681`) also has no `PrivateName` arm and falls into the
//! same `_ => (ALWAYS_PAREN, Assoc::Ltr)` (`gen_js.rs:3680`) — so `this.#x`
//! mis-prints in upstream juno too, not just in this port. `PrivateName`
//! belongs in the same bucket as `Identifier`: both are bare-token leaves
//! that never need parens anywhere (`PRIMARY`, `Assoc::Ltr`), so the fix
//! adds `Node::PrivateName(_)` to that arm's pattern rather than giving it a
//! dedicated one.

// Task 4's `TaggedTemplateExpression` arm was the first outside caller of
// `print_child`/`ChildPos`; Task 5's `arms/expr.rs` is now the main
// consumer of this whole module (`get_precedence`, `need_parens`,
// `print_child`, `print_comma_expression`, the four operator classifiers and
// their new `as_str`, `ForceSpace`). `stmt_skip_semi` stays unreferenced
// outside this file's own tests until Task 6's statement arms exist, so the
// blanket allow stays until then.
#![allow(dead_code)]

use hermes_ast::context::GCLock;
use hermes_ast::node::{
    AssignmentExpression, BinaryExpression, CallExpression, CatchClause, ConditionalExpression,
    DeclareExportDeclaration, ExportDefaultDeclaration, ExportNamedDeclaration,
    ExpressionStatement, Identifier,
    InferTypeAnnotation, LabeledStatement, LogicalExpression, MemberExpression, NewExpression,
    Node, NodeField, NumericLiteral, OptionalCallExpression, OptionalMemberExpression,
    TSModuleMember, TaggedTemplateExpression, TryStatement, TypeParameter, UnaryExpression,
    UpdateExpression,
};
use hermes_ast::node_child::NodeLabel;
use hermes_ast::visitor::{Path, Visitor};

use crate::gen::Assoc;
use crate::{out, GenJS, GenJsError, Pretty};

// ---------------------------------------------------------------------------
// Operator classification (see the module doc comment's deviation #1).
// ---------------------------------------------------------------------------

/// Binary operators, classified from the raw spelling stored on
/// `BinaryExpression::operator`.
///
/// Variant set and spellings from juno `juno_ast/src/node_enums.rs:31-56`
/// (`define_str_enum!(BinaryExpressionOperator, ...)`); juno's field is this
/// type directly, ours is a raw atom (module doc comment).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum BinaryExpressionOperator {
    /// `==`
    LooseEquals,
    /// `!=`
    LooseNotEquals,
    /// `===`
    StrictEquals,
    /// `!==`
    StrictNotEquals,
    /// `<`
    Less,
    /// `<=`
    LessEquals,
    /// `>`
    Greater,
    /// `>=`
    GreaterEquals,
    /// `<<`
    LShift,
    /// `>>`
    RShift,
    /// `>>>`
    RShift3,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Mult,
    /// `/`
    Div,
    /// `%`
    Mod,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `&`
    BitAnd,
    /// `**`
    Exp,
    /// `in`
    In,
    /// `instanceof`
    Instanceof,
}

impl BinaryExpressionOperator {
    /// Classify `label`, the raw contents of a `BinaryExpression`'s
    /// `operator` field.
    ///
    /// # Errors
    /// `Err(GenJsError::UnknownOperator { .. })` if `label`'s spelling is
    /// none of the 22 above. The bundled parser only ever writes one of
    /// these fixed spellings into a `BinaryExpression`'s `operator` field,
    /// but `generate()`'s `Node` parameter is not required to have come from
    /// it — a hand-built or JSON-deserialized tree can hold anything — so
    /// this is a malformed-input-tree case per spec §4, reported through
    /// [`crate::GenJsError`] rather than a panic.
    pub(crate) fn from_label(gc: &GCLock<'_, '_>, label: NodeLabel) -> Result<Self, GenJsError> {
        Ok(match gc.bytes_str_lossy(label) {
            "==" => Self::LooseEquals,
            "!=" => Self::LooseNotEquals,
            "===" => Self::StrictEquals,
            "!==" => Self::StrictNotEquals,
            "<" => Self::Less,
            "<=" => Self::LessEquals,
            ">" => Self::Greater,
            ">=" => Self::GreaterEquals,
            "<<" => Self::LShift,
            ">>" => Self::RShift,
            ">>>" => Self::RShift3,
            "+" => Self::Plus,
            "-" => Self::Minus,
            "*" => Self::Mult,
            "/" => Self::Div,
            "%" => Self::Mod,
            "|" => Self::BitOr,
            "^" => Self::BitXor,
            "&" => Self::BitAnd,
            "**" => Self::Exp,
            "in" => Self::In,
            "instanceof" => Self::Instanceof,
            other => {
                return Err(GenJsError::UnknownOperator {
                    kind: "BinaryExpression",
                    spelling: other.to_string(),
                })
            }
        })
    }

    /// The canonical spelling, for printing. juno stores this type directly
    /// as the AST field and reads it back with `operator.as_str()`
    /// (`node_enums.rs`'s `define_str_enum!`, `str_enum.rs:49-53`); ours
    /// classifies from a raw atom instead (module doc comment's deviation
    /// #1), so printing an operator is `from_label` (validates) then
    /// `as_str` (recovers the exact spelling) rather than one direct field
    /// read. First called by Task 5's `BinaryExpression` arm
    /// (`arms/expr.rs`).
    pub(crate) fn as_str(self) -> &'static str {
        use BinaryExpressionOperator::*;
        match self {
            LooseEquals => "==",
            LooseNotEquals => "!=",
            StrictEquals => "===",
            StrictNotEquals => "!==",
            Less => "<",
            LessEquals => "<=",
            Greater => ">",
            GreaterEquals => ">=",
            LShift => "<<",
            RShift => ">>",
            RShift3 => ">>>",
            Plus => "+",
            Minus => "-",
            Mult => "*",
            Div => "/",
            Mod => "%",
            BitOr => "|",
            BitXor => "^",
            BitAnd => "&",
            Exp => "**",
            In => "in",
            Instanceof => "instanceof",
        }
    }
}

/// Logical operators, classified from the raw spelling stored on
/// `LogicalExpression::operator`.
///
/// Variant set and spellings from juno `juno_ast/src/node_enums.rs:58-64`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum LogicalExpressionOperator {
    /// `&&`
    And,
    /// `||`
    Or,
    /// `??`
    NullishCoalesce,
}

impl LogicalExpressionOperator {
    /// Classify `label`, the raw contents of a `LogicalExpression`'s
    /// `operator` field. Errors under the same rule as
    /// [`BinaryExpressionOperator::from_label`].
    pub(crate) fn from_label(gc: &GCLock<'_, '_>, label: NodeLabel) -> Result<Self, GenJsError> {
        Ok(match gc.bytes_str_lossy(label) {
            "&&" => Self::And,
            "||" => Self::Or,
            "??" => Self::NullishCoalesce,
            other => {
                return Err(GenJsError::UnknownOperator {
                    kind: "LogicalExpression",
                    spelling: other.to_string(),
                })
            }
        })
    }

    /// The canonical spelling, for printing. See
    /// [`BinaryExpressionOperator::as_str`]. First called by Task 5's
    /// `LogicalExpression` arm (`arms/expr.rs`).
    pub(crate) fn as_str(self) -> &'static str {
        use LogicalExpressionOperator::*;
        match self {
            And => "&&",
            Or => "||",
            NullishCoalesce => "??",
        }
    }
}

/// Unary operators, classified from the raw spelling stored on
/// `UnaryExpression::operator`.
///
/// Variant set and spellings from juno `juno_ast/src/node_enums.rs:19-29`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum UnaryExpressionOperator {
    /// `delete`
    Delete,
    /// `void`
    Void,
    /// `typeof`
    Typeof,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `~`
    BitNot,
    /// `!`
    Not,
}

impl UnaryExpressionOperator {
    /// Classify `label`, the raw contents of a `UnaryExpression`'s `operator`
    /// field. Errors under the same rule as
    /// [`BinaryExpressionOperator::from_label`].
    pub(crate) fn from_label(gc: &GCLock<'_, '_>, label: NodeLabel) -> Result<Self, GenJsError> {
        Ok(match gc.bytes_str_lossy(label) {
            "delete" => Self::Delete,
            "void" => Self::Void,
            "typeof" => Self::Typeof,
            "+" => Self::Plus,
            "-" => Self::Minus,
            "~" => Self::BitNot,
            "!" => Self::Not,
            other => {
                return Err(GenJsError::UnknownOperator {
                    kind: "UnaryExpression",
                    spelling: other.to_string(),
                })
            }
        })
    }

    /// The canonical spelling, for printing. See
    /// [`BinaryExpressionOperator::as_str`]. First called by Task 5's
    /// `UnaryExpression` arm (`arms/expr.rs`).
    pub(crate) fn as_str(self) -> &'static str {
        use UnaryExpressionOperator::*;
        match self {
            Delete => "delete",
            Void => "void",
            Typeof => "typeof",
            Plus => "+",
            Minus => "-",
            BitNot => "~",
            Not => "!",
        }
    }
}

/// Update operators, classified from the raw spelling stored on
/// `UpdateExpression::operator`.
///
/// Variant set and spellings from juno `juno_ast/src/node_enums.rs:66-71`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum UpdateExpressionOperator {
    /// `++`
    Increment,
    /// `--`
    Decrement,
}

impl UpdateExpressionOperator {
    /// Classify `label`, the raw contents of an `UpdateExpression`'s
    /// `operator` field. Errors under the same rule as
    /// [`BinaryExpressionOperator::from_label`].
    pub(crate) fn from_label(gc: &GCLock<'_, '_>, label: NodeLabel) -> Result<Self, GenJsError> {
        Ok(match gc.bytes_str_lossy(label) {
            "++" => Self::Increment,
            "--" => Self::Decrement,
            other => {
                return Err(GenJsError::UnknownOperator {
                    kind: "UpdateExpression",
                    spelling: other.to_string(),
                })
            }
        })
    }

    /// The canonical spelling, for printing. See
    /// [`BinaryExpressionOperator::as_str`]. First called by Task 5's
    /// `UpdateExpression` arm (`arms/expr.rs`).
    pub(crate) fn as_str(self) -> &'static str {
        use UpdateExpressionOperator::*;
        match self {
            Increment => "++",
            Decrement => "--",
        }
    }
}

// ---------------------------------------------------------------------------
// Precedence table. juno `gen_js.rs:110-170`, flattened out of juno's
// `mod precedence { ... }` wrapper: this whole file already plays that
// module's role, so nesting a second `mod precedence` inside it (which would
// resolve as `crate::precedence::precedence::SEQ`) would only add a
// confusing extra path segment.
// ---------------------------------------------------------------------------

/// A precedence level. Higher binds tighter. The specific numbers are
/// arbitrary — only the relative order matters (see the tests below, which
/// assert ordering rather than values, per the plan's Task 3 Step 4).
pub(crate) type Precedence = u32;

/// Always needs parens: the child's kind has no precedence of its own (the
/// `get_precedence` catch-all).
pub(crate) const ALWAYS_PAREN: Precedence = 0;
/// `SequenceExpression` (`,`).
pub(crate) const SEQ: Precedence = 1;
/// `ArrowFunctionExpression`.
pub(crate) const ARROW: Precedence = 2;
/// `YieldExpression`.
pub(crate) const YIELD: Precedence = 3;
/// `AssignmentExpression`.
pub(crate) const ASSIGN: Precedence = 4;
/// `ConditionalExpression` (`?:`).
pub(crate) const COND: Precedence = 5;
/// `AsExpression`/`AsConstExpression`/`TSAsExpression` (`x as T`) — Task 13
/// review of Task 12's classification (correctness fix, no juno precedent).
///
/// Deliberately **below every binary and logical operator**, even though
/// `as` is built by the very same precedence-climbing loop as `in`/
/// `instanceof` at the identical operator precedence 8
/// (`crates/parser/src/js/expressions.rs`'s `parse_binary_expression`).
/// The reason is the *right* operand: for `as_operator` that loop does not
/// parse an expression at all, it calls
/// `parse_type_annotation(None, AllowAnonFunctionType::Yes)`, and the type
/// grammar it enters keeps reading `|` (union), `&` (intersection), `[`
/// (postfix), `<` (type arguments) and `.` (qualified name) — every one of
/// which an enclosing expression may have meant for itself. So an
/// as-expression is only safe unparenthesized where no operator token can
/// follow it at all, which is what "lower than every binary/logical
/// precedence" spells.
///
/// Measured against the crate as Task 12 shipped it (as-expression at
/// `In`'s precedence), under `-parse-flow`: `(x as A) | B;` printed
/// `x as A | B;` and reparsed to `As(x, Union[A, B])`; `(x as A) & B;` the
/// same with an intersection; `(x as A) < B;` printed `x as A < B;`, which
/// **fails to parse** (`'>' expected at end of type parameters`); and
/// `b + (x as A) | c;` printed `b + x as A | c;`, wrong in the same way.
/// That last one is what needs a *table* entry rather than a `need_parens`
/// branch keyed on the direct parent's operator: it parses as
/// `(b + (x as A)) | c`, so the as-expression's direct parent operator is
/// the harmless `+` and the absorbed `|` belongs to the **grandparent**,
/// which one level of `Path` cannot see. (`b | (x as A) | c` is left-nested,
/// so its direct parent *is* a `|`; it shows the breakage but not the need
/// for the grandparent — review round 1 finding M-2.) Regression tests:
/// `as_expression_operand_of_bitwise_and_relational_operators_keeps_parens`
/// and its TS sibling in `tests/roundtrip.rs`.
///
/// Everything looser than a binary operator is unaffected, since it sits
/// below this level: `x = y as A`, `y as A ? b : c`, `(a, y as A)`,
/// `() => y as A` and `yield y as A` all still print bare (verified —
/// see `task-13-report.md`'s audit table).
pub(crate) const AS_EXPRESSION: Precedence = 6;
/// Base that every `BinaryExpression`/`LogicalExpression` precedence is
/// offset from (see [`get_binary_precedence`]/[`get_logical_precedence`]).
///
/// Bumped 6 -> 7 to make room for [`AS_EXPRESSION`] just below it. The
/// specific numbers were always arbitrary (see [`Precedence`]) and the
/// highest binary precedence is `Exp`'s `12 + BIN_START` = 19, still well
/// below [`UNARY`]'s 26.
pub(crate) const BIN_START: Precedence = 7;
/// Unary operators (`!`, `~`, `typeof`, `void`, `delete`) and prefix `++`/`--`.
pub(crate) const UNARY: Precedence = 26;
/// Postfix `++`/`--`.
pub(crate) const POST_UPDATE: Precedence = 27;
/// Flow `RecordExpression` (`Point { x: 1 }`) — Task 12 review round 4.
///
/// This is *not* a primary, despite looking like one. `parseLeftHandSideExpressionTail`
/// (`lib/Parser/JSParserImpl.cpp:4026-4089`, ported at
/// `crates/parser/src/js/expressions.rs:2752`) builds the record expression in
/// a trailing `else if` and then **returns immediately** — it never loops back
/// into `parseOptionalExpressionExceptNew_tail`, so no member select, call,
/// index, or template tag can attach to a record expression. Verified against
/// the parser as shipped: `R {p:1}.foo`, `R {p:1}()`, `R {p:1}[0]` and
/// ``R {p:1}`t` `` all fail to parse *anywhere*, including inside
/// `x = …` — while `(R {p:1}).foo` and friends parse fine and are exactly the
/// trees the parser produces (`MemberExpression{object: RecordExpression}`,
/// `CallExpression{callee: …}`, `NewExpression{callee: …}`,
/// `TaggedTemplateExpression{tag: …}`). That is the same shape as
/// [`NEW_NO_ARGS`] — a construct that must be wrapped before any postfix
/// operator can be applied to it — so it sits strictly below
/// [`TAGGED_TEMPLATE`]/[`NEW_NO_ARGS`]/[`MEMBER`], and above [`UNARY`] because
/// `typeof R {p:1}`, `-R {p:1}`, `!R {p:1}` and `void R {p:1}` all parse bare
/// (so wrapping there would be redundant output, the defect review round 3
/// fixed for `InferTypeAnnotation`).
///
/// Inserting this level shifted every constant below it up by one. The
/// specific numbers were always arbitrary (see [`Precedence`]); the tests
/// assert against the named constants, never the literals.
pub(crate) const RECORD_EXPRESSION: Precedence = 28;
/// Tagged templates and dynamic `import()`.
pub(crate) const TAGGED_TEMPLATE: Precedence = 29;
/// `new Foo` with no argument list (as opposed to `new Foo()`).
pub(crate) const NEW_NO_ARGS: Precedence = 30;
/// Member access and calls.
pub(crate) const MEMBER: Precedence = 31;
/// Literals, identifiers, and other atomic primaries.
pub(crate) const PRIMARY: Precedence = 32;
/// The safest level: equal-precedence children here never need parens
/// regardless of associativity (see [`GenJS::need_parens`]).
pub(crate) const TOP: Precedence = 33;

/// `UnionTypeAnnotation` (`|`). A separate numbering space from the
/// expression precedences above — juno reuses small integers here on
/// purpose, since union/intersection types never compare against an
/// expression's precedence.
pub(crate) const UNION_TYPE: Precedence = 1;
/// `IntersectionTypeAnnotation` (`&`).
pub(crate) const INTERSECTION_TYPE: Precedence = 2;

// ---------------------------------------------------------------------------
// The Flow `match` pattern numbering space (Task 12). A third separate
// numbering, alongside `UNION_TYPE`/`INTERSECTION_TYPE`: match patterns never
// compare against expression or Flow-type precedence, since a match pattern's
// `path.parent` in `need_parens` is always another match-pattern kind (the
// only two `print_child` call sites that can receive a match pattern as the
// child are `MatchOrPattern`'s element list and `MatchAsPattern`'s `pattern`).
//
// The three tiers below are exactly `parseMatchPatternFlow`'s own three
// layers (`crates/parser/src/js/flow/match_.rs:428-484`), innermost binding
// tightest:
//
//   MatchPattern := [`|`] Subpattern (`|` Subpattern)* [`as` BindingTarget]
//
// so `as` is the loosest, `|` next, and everything `parseMatchSubpatternFlow`
// returns directly is tightest.
// ---------------------------------------------------------------------------

/// `MatchAsPattern` (`pattern as target`) — the loosest match-pattern tier:
/// the trailing `as` of `parseMatchPatternFlow` wraps whatever the `|`-loop
/// produced.
///
/// `Assoc::Rtl` in [`GenJS::get_precedence`] is what makes an equal-precedence
/// **left** child (`pattern`, printed at [`ChildPos::Left`]) need parens: `as`
/// is in truth *non*-associative here — `a as y as z` does not parse at all,
/// since `parseMatchPatternFlow`'s `as` branch runs once and its target is a
/// binding identifier/pattern, never another pattern — and `Rtl` + `Left` is
/// how this table spells "the left side at equal precedence is the dangerous
/// side". There is no right-hand pattern child to mis-classify: `target` is
/// not a pattern at all.
pub(crate) const MATCH_AS_PATTERN: Precedence = 1;
/// `MatchOrPattern` (`a | b | c`) — the middle match-pattern tier, built by
/// `parseMatchPatternFlow`'s `|`-loop out of subpatterns only.
pub(crate) const MATCH_OR_PATTERN: Precedence = 2;
/// A Flow `match` subpattern: any pattern kind reachable directly from
/// `parseMatchSubpatternFlow` (`crates/parser/src/js/flow/match_.rs`) —
/// i.e. every match-pattern kind except `MatchOrPattern`/`MatchAsPattern`
/// themselves, which are built only by the *enclosing* `parseMatchPatternFlow`
/// (the `|`-loop and trailing `as`) or by unwrapping an explicit `(
/// MatchPattern )` group. The tightest match-pattern tier.
pub(crate) const MATCH_SUBPATTERN: Precedence = 3;

// ---------------------------------------------------------------------------
// The TypeScript type numbering space (Task 13). A fourth separate numbering,
// alongside `UNION_TYPE`/`INTERSECTION_TYPE` (Flow types) and `MATCH_*` (Flow
// match patterns): a TS type's `path.parent` in `need_parens` is always
// another TS type kind, since TypeScript and Flow are mutually-exclusive
// dialects (`ParseFlags::parse_ts`'s own doc comment) and no expression
// production ever `print_child`s a type.
//
// The six tiers below are exactly the layers of
// `crates/parser/src/js/ts/types.rs`'s recursive descent, innermost binding
// tightest:
//
//   Type         := predicate | `new` ctor | `<T>` fn | Union [`extends` …]
//   Union        := Intersection (`|` Intersection)*
//   Intersection := Postfix (`&` Postfix)*
//   Postfix      := Primary (`[` `]` | `[` Type `]`)*
//   Primary      := keyword | literal | `this` | tuple | `typeof` … | `{`…`}`
//                   | `interface` … | TypeReference | `(` Type `)`
//
// Note the values stay BELOW the expression space's `PRIMARY` (32) on
// purpose. `ExistsTypeAnnotation` (`*`) is shared between the Flow and TS
// grammars — `parse_ts_primary_type`'s `star` arm builds one — and is
// already classified `PRIMARY` there; numbering the TS tiers above 32 would
// make every `*` in a TS union/array position compare as *lower* and get
// wrapped, and `(*)` does not parse at all (the `(`-cover calls
// `parse_binding_element`, which rejects `*`).
// ---------------------------------------------------------------------------

/// `TSConditionalType` and `TSTypePredicate` — the loosest TS type tier:
/// both are produced only by `parse_type_annotation_ts` itself (its trailing
/// `extends … ? … :` clause and its leading-identifier `is` backtrack), so
/// neither is reachable from any narrower production without explicit
/// parens.
pub(crate) const TS_CONDITIONAL_TYPE: Precedence = 1;
/// `TSFunctionType`/`TSConstructorType` (`(a: A) => B`, `new (a: A) => B`).
///
/// Below [`TS_UNION_TYPE`] because the *return type* is a full
/// `parse_type_annotation_ts(None)` that runs to the end of the type: a
/// function type printed bare in any narrower position hands its own return
/// type whatever followed it (`(a: A) => B | C` is one function type
/// returning `B | C`, not a union of two things).
pub(crate) const TS_FUNCTION_TYPE: Precedence = 2;
/// `TSUnionType` (`A | B`) — `parse_ts_union_type`.
pub(crate) const TS_UNION_TYPE: Precedence = 3;
/// `TSIntersectionType` (`A & B`) — `parse_ts_intersection_type`.
pub(crate) const TS_INTERSECTION_TYPE: Precedence = 4;
/// `TSArrayType`/`TSIndexedAccessType` (`A[]`, `A[K]`) — the postfix loop of
/// `parse_ts_postfix_type`, which applies only to a *primary* base.
pub(crate) const TS_POSTFIX_TYPE: Precedence = 5;
/// Everything `parse_ts_primary_type` returns directly: the primitive
/// keywords, `TSLiteralType`, `TSThisType`, `TSTupleType`, `TSTypeQuery`,
/// `TSTypeLiteral`, `TSTypeReference`/`TSQualifiedName`, and the
/// `TSInterfaceDeclaration` its `rw_interface` arm builds. The tightest TS
/// type tier: never needs parens anywhere in the type grammar.
pub(crate) const TS_PRIMARY_TYPE: Precedence = 6;

/// The precedence of a `BinaryExpression` with operator `op`.
///
/// juno `gen_js.rs:134-160`. Exhaustive match, no wildcard: adding a new
/// [`BinaryExpressionOperator`] variant without a case here is a compile
/// error (plan Task 3 Step 4).
pub(crate) fn get_binary_precedence(op: BinaryExpressionOperator) -> Precedence {
    use BinaryExpressionOperator::*;
    (match op {
        Exp => 12,
        Mult => 11,
        Mod => 11,
        Div => 11,
        Plus => 10,
        Minus => 10,
        LShift => 9,
        RShift => 9,
        RShift3 => 9,
        Less => 8,
        Greater => 8,
        LessEquals => 8,
        GreaterEquals => 8,
        LooseEquals => 7,
        LooseNotEquals => 7,
        StrictEquals => 7,
        StrictNotEquals => 7,
        BitAnd => 6,
        BitXor => 5,
        BitOr => 4,
        In => 8,
        Instanceof => 8,
    }) + BIN_START
}

/// The precedence of a `LogicalExpression` with operator `op`.
///
/// juno `gen_js.rs:162-169`. Exhaustive match, no wildcard: adding a new
/// [`LogicalExpressionOperator`] variant without a case here is a compile
/// error (plan Task 3 Step 4).
pub(crate) fn get_logical_precedence(op: LogicalExpressionOperator) -> Precedence {
    use LogicalExpressionOperator::*;
    (match op {
        And => 3,
        Or => 2,
        NullishCoalesce => 1,
    }) + BIN_START
}

/// Which structural position a child occupies relative to its parent, for
/// deciding whether it needs parens.
///
/// juno `gen_js.rs:172-178`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum ChildPos {
    /// The left-hand child (e.g. a `BinaryExpression`'s `left`).
    Left,
    /// Neither definitely left nor right (e.g. a `SequenceExpression`
    /// element).
    Anywhere,
    /// The right-hand child (e.g. a `BinaryExpression`'s `right`).
    Right,
}

/// Whether parens (or, in one case, a bare space) are needed around a child
/// expression.
///
/// juno `gen_js.rs:180-190`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum NeedParens {
    /// No parens needed.
    No,
    /// Parens required.
    Yes,
    /// A single space is sufficient to disambiguate (e.g. `- -x` vs `--x`).
    Space,
}

impl From<bool> for NeedParens {
    /// juno `gen_js.rs:192-196`.
    fn from(x: bool) -> NeedParens {
        if x {
            NeedParens::Yes
        } else {
            NeedParens::No
        }
    }
}

/// Whether to force a space when emitting a separator, independent of
/// pretty-mode.
///
/// juno `gen_js.rs:198-203`. First consumed by Task 5's arms (`arms/expr.rs`)
/// via [`GenJS::space`](crate::GenJS): `a in b`/`typeof x` need the space
/// even in compact mode, since `ainb`/`typeofx` would read as one identifier
/// if it were ever omitted (unlike `a+b`, which stays unambiguous without
/// one).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum ForceSpace {
    /// Only add a space if pretty-printing.
    No,
    /// Always add a space.
    Yes,
}

// ---------------------------------------------------------------------------
// The decision functions.
// ---------------------------------------------------------------------------

impl<'s, 'w> GenJS<'s, 'w> {
    /// The precedence and associativity of `node`, for parenthesization
    /// decisions.
    ///
    /// juno `gen_js.rs:3589-3681`. See the module doc comment's "correctness
    /// fix" section for the one intentional deviation, in the
    /// `BinaryExpression` arm. Returns `Result` — unlike juno's infallible
    /// version — because classifying `BinaryExpression`/`LogicalExpression`
    /// operators can fail (module doc comment's deviation #1).
    pub(crate) fn get_precedence<'gc>(
        &self,
        ctx: &GCLock<'_, '_>,
        node: &Node<'gc>,
    ) -> Result<(Precedence, Assoc), GenJsError> {
        Ok(match node {
            Node::Identifier(_)
            | Node::PrivateName(_)
            | Node::NullLiteral(_)
            | Node::BooleanLiteral(_)
            | Node::StringLiteral(_)
            | Node::NumericLiteral(_)
            | Node::RegExpLiteral(_)
            | Node::ThisExpression(_)
            | Node::Super(_)
            | Node::ArrayExpression(_)
            | Node::ObjectExpression(_)
            | Node::ObjectPattern(_)
            | Node::FunctionExpression(_)
            | Node::ClassExpression(_)
            | Node::TemplateLiteral(_)
            | Node::JSXElement(_)
            | Node::JSXFragment(_)
            | Node::TypeCastExpression(_) => (PRIMARY, Assoc::Ltr),

            Node::MemberExpression(_)
            | Node::OptionalMemberExpression(_)
            | Node::MetaProperty(_)
            | Node::CallExpression(_)
            | Node::OptionalCallExpression(_) => (MEMBER, Assoc::Ltr),

            Node::NewExpression(NewExpression {
                metadata: _,
                callee: _,
                type_arguments: _,
                arguments,
            }) => {
                // `new foo()` has higher precedence than `new foo`. In
                // pretty mode we always append the `()`, but otherwise we
                // must check the number of args.
                if self.pretty() == Pretty::Yes || !arguments.is_empty() {
                    (MEMBER, Assoc::Ltr)
                } else {
                    (NEW_NO_ARGS, Assoc::Ltr)
                }
            }

            Node::TaggedTemplateExpression(_) | Node::ImportExpression(_) => {
                (TAGGED_TEMPLATE, Assoc::Ltr)
            }

            Node::UpdateExpression(UpdateExpression {
                metadata: _,
                operator: _,
                argument: _,
                prefix,
            }) => {
                if prefix.get() {
                    (POST_UPDATE, Assoc::Ltr)
                } else {
                    (UNARY, Assoc::Rtl)
                }
            }

            Node::UnaryExpression(_) => (UNARY, Assoc::Rtl),

            // Defect 28 (juno's, ours until Task 17). **DEVIATION from juno
            // — a correctness fix, not a transcription.** juno's
            // `get_precedence` (`gen_js.rs:3589-3681`) has no
            // `AwaitExpression` arm at all, so `await` fell into its
            // `_ => (ALWAYS_PAREN, Assoc::Ltr)` catch-all, and ours
            // inherited that. `ALWAYS_PAREN` is 0, so as a *parent*
            // `AwaitExpression` had the lowest possible precedence and
            // `need_parens` never wrapped its operand: every one of
            // `await (a + b)`, `await (a || b)`, `await (a ?? b)`,
            // `await (a ? b : c)`, `await (a = b)`, `await (yield a)`,
            // `await (() => 1)`, `await (x as T)` regenerated **without**
            // the parens, and the tree changed (or, for the arrow and the
            // yield, the result did not parse at all). Measured by
            // `tests/paren_matrix.rs`'s `AwaitExpression.argument` row: 28
            // of its 2 x 14 round trips failed before this entry existed.
            //
            // ECMA-262 puts it exactly where a prefix `UnaryExpression`
            // sits — `UnaryExpression : AwaitExpression` and
            // `AwaitExpression : await UnaryExpression` (13.5, 13.6) — so
            // the entry is `UnaryExpression`'s, verbatim. That also fixes
            // the *child* direction: `t = await a` no longer emits the
            // gratuitous `t = (await a)` the catch-all forced.
            Node::AwaitExpression(_) => (UNARY, Assoc::Rtl),

            Node::BinaryExpression(BinaryExpression {
                metadata: _,
                left: _,
                right: _,
                operator,
            }) => {
                let op = BinaryExpressionOperator::from_label(ctx, operator.get())?;
                // DEVIATION from juno (`gen_js.rs:3649`), which always
                // returns `Assoc::Ltr` here. See the module doc comment.
                let assoc = if op == BinaryExpressionOperator::Exp {
                    Assoc::Rtl
                } else {
                    Assoc::Ltr
                };
                (get_binary_precedence(op), assoc)
            }

            Node::LogicalExpression(LogicalExpression {
                metadata: _,
                left: _,
                right: _,
                operator,
            }) => {
                let op = LogicalExpressionOperator::from_label(ctx, operator.get())?;
                (get_logical_precedence(op), Assoc::Ltr)
            }

            Node::ConditionalExpression(_) => (COND, Assoc::Rtl),
            Node::AssignmentExpression(_) => (ASSIGN, Assoc::Rtl),
            Node::ArrowFunctionExpression(_) => (ARROW, Assoc::Ltr),
            Node::YieldExpression(_) => (YIELD, Assoc::Ltr),
            Node::SequenceExpression(_) => (SEQ, Assoc::Rtl),

            // Task 12 (`arms/newer.rs`): `x as T`/`x as const` are built by
            // the *same* precedence-climbing binary-operator loop as `+`/
            // `in`/`instanceof` (`crates/parser/src/js/expressions.rs`'s
            // `parse_binary_expression`, `as_operator` precedence 8 —
            // identical to `in`/`instanceof`), left-associative, so
            // `expression` is a real operand `need_parens` must protect the
            // same way it protects any other binary operator's operand. See
            // `arms/newer.rs`'s module doc comment for the full trace.
            //
            // Task 13 correctness fix: the *tier* is [`AS_EXPRESSION`], not
            // `In`'s binary precedence — the right operand is a type, and
            // the type grammar keeps reading past the end of the type.
            // `Assoc::Ltr` is unchanged and is what keeps a chained
            // `x as A as B` (a real, left-associative shape) bare. See
            // `AS_EXPRESSION`'s doc comment for the measured failures.
            // `TSAsExpression` is the TypeScript spelling of the identical
            // production (`make_as_node`'s `parse_ts()` branch) and shares
            // the entry.
            Node::AsExpression(_) | Node::AsConstExpression(_) | Node::TSAsExpression(_) => {
                (AS_EXPRESSION, Assoc::Ltr)
            }

            // ---------------------------------------------------------------
            // Task 13: the TypeScript type grammar. See the `TS_*_TYPE`
            // constants above for the tier-by-tier derivation from
            // `crates/parser/src/js/ts/types.rs`.
            // ---------------------------------------------------------------
            Node::TSConditionalType(_) | Node::TSTypePredicate(_) => {
                (TS_CONDITIONAL_TYPE, Assoc::Ltr)
            }
            Node::TSFunctionType(_) | Node::TSConstructorType(_) => (TS_FUNCTION_TYPE, Assoc::Ltr),
            Node::TSUnionType(_) => (TS_UNION_TYPE, Assoc::Ltr),
            Node::TSIntersectionType(_) => (TS_INTERSECTION_TYPE, Assoc::Ltr),
            Node::TSArrayType(_) | Node::TSIndexedAccessType(_) => (TS_POSTFIX_TYPE, Assoc::Ltr),
            Node::TSAnyKeyword(_)
            | Node::TSNumberKeyword(_)
            | Node::TSBooleanKeyword(_)
            | Node::TSStringKeyword(_)
            | Node::TSSymbolKeyword(_)
            | Node::TSVoidKeyword(_)
            | Node::TSUndefinedKeyword(_)
            | Node::TSUnknownKeyword(_)
            | Node::TSNeverKeyword(_)
            | Node::TSBigIntKeyword(_)
            | Node::TSThisType(_)
            | Node::TSLiteralType(_)
            | Node::TSTupleType(_)
            | Node::TSTypeQuery(_)
            | Node::TSTypeLiteral(_)
            | Node::TSTypeReference(_)
            | Node::TSQualifiedName(_)
            | Node::TSInterfaceDeclaration(_) => (TS_PRIMARY_TYPE, Assoc::Ltr),

            // The two transparent annotation wrappers. Neither has any
            // syntax of its own — `arms/flow_decl.rs`'s `gen_type_annotation`
            // and `arms/ts.rs`'s `gen_ts_type_annotation` both print only
            // the type they wrap — and both are built exclusively for `: T`
            // positions, where `T` is a *full* type annotation
            // (`parse_return_type_annotation_flow`,
            // `parse_type_annotation_ts(Some(..))`). So they never need
            // parens anywhere, which is exactly what [`TOP`] means; before
            // Task 13 neither had an arm and both fell into the
            // `ALWAYS_PAREN` catch-all.
            //
            // **DEVIATION from juno — a correctness fix.** juno's
            // `get_precedence` has no `TypeAnnotation` arm either, and its
            // `ArrowFunctionExpression` arm (`gen_js.rs:490-498`) prints
            // `return_type` through `print_child`, so upstream juno wraps
            // every typed arrow's return type in parens. Under Flow that is
            // merely ugly (`(a: number): (string) => a` reparses to the
            // identical tree — a parenthesized Flow type is still that
            // type). Under TypeScript it is a **hard reparse failure**:
            // measured before this fix, `let f = (a: A): B => a;`
            // regenerated as `let f = (a: A): (B) => a;`, which our parser
            // rejects with `';' expected`. Note the sibling
            // `visit_func_params_body` (`arms/expr.rs`, used by every
            // *non*-arrow function) already printed `return_type` with a
            // bare `gen_node` and was never affected; this makes the two
            // agree. Regression test:
            // `ts_typed_arrow_return_type_keeps_no_parens`.
            Node::TypeAnnotation(_) | Node::TSTypeAnnotation(_) => (TOP, Assoc::Ltr),

            // `<T>expr` is `parse_unary_expression`'s own `less` arm and its
            // operand is a `parse_unary_expression` — the same shape as a
            // prefix `UnaryExpression`, hence the same `(UNARY, Assoc::Rtl)`
            // entry. `Rtl` + `ChildPos::Right` (what
            // `gen_ts_type_assertion` passes) is what keeps a nested
            // `<T><U>x` and a `<T>-x` bare at equal precedence.
            Node::TSTypeAssertion(_) => (UNARY, Assoc::Rtl),

            // Task 12: `MatchExpression` is a genuine primary — it is
            // self-delimited by a trailing `}` and, unlike its
            // `RecordExpression` sibling below, freely takes a postfix tail
            // (`match (x) {…}.foo`, `…()`, `…[0]`, `` …`t` ``, `new …` all
            // parse bare; verified against the parser as shipped). Its one
            // hazard is positional, not precedence-based, and lives in the
            // `ExpressionStatement` branch of `need_parens` — see there.
            Node::MatchExpression(_) => (PRIMARY, Assoc::Ltr),

            // Task 12 review round 4 (correctness fix, no juno precedent):
            // `RecordExpression` was classified `PRIMARY` alongside
            // `MatchExpression`, on the reasoning that both are
            // "self-delimited by a trailing `}`, the same tier as
            // `ObjectExpression`". That is wrong for this kind: `PRIMARY` is
            // *above* `MEMBER`, so `MemberExpression`'s `object` printed a
            // record expression bare — and a record expression cannot carry
            // a postfix tail at all (see `RECORD_EXPRESSION`'s doc comment
            // for the parser evidence, both ours and the upstream C++).
            // Live before this fix: `(R {p: 1}).foo;` regenerated as
            // `R {p: 1}.foo;`, which **fails to reparse** (`';' expected`).
            // Note the fix is a precedence one, NOT an entry in the
            // `ExpressionStatement` statement-start guard: at statement
            // start a bare `R {p: 1};` parses to the identical tree, and
            // wrapping the whole statement expression the way that guard
            // does would produce `(R {p: 1}.foo);`, which does not parse
            // either — the parens have to land on the record expression
            // itself.
            Node::RecordExpression(_) => (RECORD_EXPRESSION, Assoc::Ltr),

            Node::ExistsTypeAnnotation(_)
            | Node::EmptyTypeAnnotation(_)
            | Node::StringTypeAnnotation(_)
            | Node::BigIntTypeAnnotation(_)
            | Node::NumberTypeAnnotation(_)
            | Node::StringLiteralTypeAnnotation(_)
            | Node::NumberLiteralTypeAnnotation(_)
            | Node::BooleanTypeAnnotation(_)
            | Node::BooleanLiteralTypeAnnotation(_)
            | Node::NullLiteralTypeAnnotation(_)
            | Node::SymbolTypeAnnotation(_)
            | Node::AnyTypeAnnotation(_)
            | Node::MixedTypeAnnotation(_)
            | Node::VoidTypeAnnotation(_)
            // Task 12: the same self-delimited primitive-keyword shape as
            // the siblings just above — `never`/`undefined`/`unknown` have
            // no fields and no continuation, exactly like `any`/`mixed`.
            | Node::NeverTypeAnnotation(_)
            | Node::UndefinedTypeAnnotation(_)
            | Node::UnknownTypeAnnotation(_) => (PRIMARY, Assoc::Ltr),
            Node::NullableTypeAnnotation(_) => (UNARY, Assoc::Ltr),
            Node::UnionTypeAnnotation(_) => (UNION_TYPE, Assoc::Ltr),
            Node::IntersectionTypeAnnotation(_) => (INTERSECTION_TYPE, Assoc::Ltr),

            // Task 12 review round 2 (correctness fixes, not transcriptions —
            // there is no juno precedent for any of these five kinds):
            // `TypeOperator`/`KeyofTypeAnnotation` wrap a body parsed at
            // `parsePrefixTypeAnnotationFlow` tier (`flow/types.rs`'s
            // `parse_prefix_type_annotation_flow`, confirmed for `keyof` at
            // the `NamedType::Keyof` arm and for `renders`/`renders?`/
            // `renders*` at both `parseComponentRenderTypeFlow`'s
            // `component_type` branch and the `NamedType::Renders` primary
            // arm) — the exact same restricted tier `NullableTypeAnnotation`
            // itself is built from (`?` also calls
            // `parse_prefix_type_annotation_flow` recursively), so these
            // join it at `UNARY` with the identical `Assoc::Ltr` (mirroring
            // `gen_nullable_type_annotation`'s own `ChildPos::Right`
            // ties-favor-parens choice, for consistency rather than
            // introducing a second tie-break convention for the same
            // grammar shape). Without this, `keyof (A | B)` regenerated as
            // `Union[Keyof(A), B]` — a different top-level kind, not merely
            // missing parens — and `component() renders (A | B)` regenerated
            // as a bare `UnionTypeAnnotation` `renders_type`, same defect.
            Node::TypeOperator(_) | Node::KeyofTypeAnnotation(_) => (UNARY, Assoc::Ltr),

            // `ConditionalTypeAnnotation.check_type`/`.extends_type` are
            // each parsed at
            // `parseUnionTypeAnnotationFlow` tier specifically — confirmed
            // for the former at `parse_conditional_type_annotation_flow`
            // (both calls, `flow/types.rs`), and for the latter at the
            // `infer` arm's speculative bound parse (same file). The one
            // kind looser than union tier — and therefore the only kind
            // that ever needs wrapping in either restricted field — is
            // `ConditionalTypeAnnotation` itself (nothing else in the type
            // grammar sits below union — except an `InferTypeAnnotation`
            // that *has* a bound, classified separately just below), so this
            // is classified at
            // `ALWAYS_PAREN`, *not* `UNION_TYPE`: setting it to
            // `UNION_TYPE` was tried first and is WRONG — it makes a nested
            // `ConditionalTypeAnnotation` compare as *equal* precedence to
            // its parent (both `UNION_TYPE`) rather than strictly lower,
            // so `need_parens`'s tie-break — designed for genuinely
            // directional binary operators, not this "is the child's own
            // tier at least as loose as mine" question — decides the
            // outcome instead of the unconditional "always wrap" this
            // needs, and for at least one `Assoc`/`ChildPos` pairing gets it
            // backwards (confirmed: this exact mistake made
            // `conditional_type_annotation_parenthesizes_restricted_check_and_extends_type`
            // fail during this fix's own development, `tests/roundtrip.rs`).
            // `ALWAYS_PAREN` sidesteps the tie-break entirely: `need_parens`'s
            // very first check (`child_prec == ALWAYS_PAREN ⇒ Yes`) fires
            // before any threshold or `ChildPos` comparison whenever the
            // child is itself unclassified (a nested `ConditionalTypeAnnotation`,
            // correctly wrapped), and every *other* classified child
            // (`UnionTypeAnnotation` at `UNION_TYPE`, primary types at
            // `PRIMARY`, ...) trivially clears `child_prec > path_prec(0)`
            // and prints bare — matching the real grammar exactly, with no
            // dependence on `ChildPos`/`Assoc` at all. Mirrors
            // `FunctionTypeAnnotation`'s existing `ALWAYS_PAREN` entry
            // above, the identical "genuinely needs the wrap whenever not
            // the sole top-level type" shape. Without this,
            // `A extends B ? C : D extends E ? F : G`'s parenthesized
            // `check_type`/`extends_type` lost their grouping on
            // regeneration (a structurally different tree).
            Node::ConditionalTypeAnnotation(_) => (ALWAYS_PAREN, Assoc::Ltr),

            // `InferTypeAnnotation`'s tier depends on whether it HAS a
            // bound — the same shape-dependent classification
            // `NewExpression` above already uses for `new Foo` vs.
            // `new Foo()`. Task 12 review round 3; round 2 grouped this
            // kind into the `ConditionalTypeAnnotation` arm just above and
            // was too coarse (see below).
            //
            // - **With a bound** (`infer A extends B`): the bound is parsed
            //   by the *speculative* `parse_union_type_annotation_flow()`
            //   call in `parse_primary_type_annotation_flow`'s `infer` arm
            //   (`crates/parser/src/js/flow/types.rs:729-751`), so the
            //   construct extends rightwards over a whole union — it binds
            //   *looser* than `UNION_TYPE` itself, and `ALWAYS_PAREN` (0,
            //   below every other tier) is the only value in this space
            //   that expresses that. Verified live: without the wrap,
            //   `?(infer B extends C) | D` regenerates as
            //   `?infer B extends C | D`, whose bound greedily swallows
            //   `C | D` — `Nullable(Infer(B, Union[C, D]))`, not the
            //   original `Union[Nullable(Infer(B, C)), D]`. Regression
            //   test: `infer_type_annotation_with_bound_stays_parenthesized_
            //   in_union_and_nullable_positions`.
            // - **Without a bound** (`infer A`): a plain
            //   `parsePrimaryTypeAnnotationFlow` production — the keyword
            //   plus one identifier and nothing else — exactly as
            //   self-delimited as `any`/`mixed`/`Array<T>`, hence `PRIMARY`.
            //   The single token that could extend it, `extends`, is only
            //   ever emitted right after a child in ONE printer position:
            //   `ConditionalTypeAnnotation`'s `check_type`
            //   (`arms/newer.rs`'s `gen_conditional_type_annotation`); and
            //   `check_type` is parsed with `allow_conditional_type = true`
            //   (`types.rs:193`), which is precisely the flag that arms the
            //   `infer` arm's backtrack — on seeing a `?` after the
            //   speculative bound, it restores and re-reads the `extends` as
            //   the conditional's own (`types.rs:733-747`). So
            //   `(infer A) extends C ? D : E` regenerates bare as
            //   `infer A extends C ? D : E` and reparses to the identical
            //   tree. Every other position an `infer A` can be printed into
            //   (union/intersection member, `?`-nullable operand, `keyof`/
            //   `renders` operand, postfix `[]`/`[K]` base, a conditional's
            //   `extends_type`/`true_type`/`false_type`, another `infer`'s
            //   bound) emits `|`, `&`, `[`, `?`, `:` or end-of-type next,
            //   none of which the `infer` arm consumes. Verified live for
            //   each: regression test
            //   `infer_type_annotation_without_bound_needs_no_parens`.
            //
            // The reviewer's stated worry — that a finer classification
            // could UNDER-parenthesize the `check_type`-with-bound
            // sub-case — cannot materialize, because the with-bound half
            // keeps the strictly-more-conservative `ALWAYS_PAREN` it
            // already had; only the no-bound half loosens.
            Node::InferTypeAnnotation(InferTypeAnnotation {
                metadata: _,
                type_parameter,
            }) => match type_parameter {
                Node::TypeParameter(TypeParameter {
                    metadata: _,
                    name: _,
                    r#const: _,
                    bound: Some(_),
                    variance: _,
                    default: _,
                    uses_extends_bound: _,
                }) => (ALWAYS_PAREN, Assoc::Ltr),
                Node::TypeParameter(_) => (PRIMARY, Assoc::Ltr),
                // A malformed tree (`type_parameter` is not a
                // `TypeParameter` at all — `gen_infer_type_annotation`
                // rejects it with `UnsupportedKind`, but `get_precedence`
                // can be reached first, e.g. through a parent's
                // `print_child`). Stay on the conservative side.
                _ => (ALWAYS_PAREN, Assoc::Ltr),
            },

            // Task 12 review rounds 2 and 3: the Flow `match` pattern
            // grammar (`crates/parser/src/js/flow/match_.rs`) is entirely
            // disjoint from expressions and Flow types, so it gets its own
            // three-tier numbering space (`MATCH_AS_PATTERN` <
            // `MATCH_OR_PATTERN` < `MATCH_SUBPATTERN`, mirroring
            // `parseMatchPatternFlow`'s own three layers — see those
            // constants' doc comments) rather than reusing
            // `PRIMARY`/`UNION_TYPE`/etc.
            //
            // Round 2 classified only the subpattern kinds and left
            // `MatchOrPattern`/`MatchAsPattern` in the `ALWAYS_PAREN`
            // catch-all. That was right for `MatchOrPattern`'s element list
            // but too coarse once `MatchAsPattern`'s own `pattern` field
            // also became a `print_child` (round 3), because the two fields
            // need *different* answers for the same child kind:
            //
            // - As an **element of a `MatchOrPattern`** (`ChildPos::Anywhere`,
            //   `path_prec` `MATCH_OR_PATTERN`), both a `MatchAsPattern`
            //   (1 < 2 ⇒ `Yes`) and a nested `MatchOrPattern` (2 == 2, and
            //   `ChildPos::Anywhere` ⇒ `Yes`) must be re-wrapped: elements
            //   come from `parseMatchSubpatternFlow`, which can only reach
            //   either kind by unwrapping an explicit `( MatchPattern )`
            //   group (its `l_paren` arm calls the *full*
            //   `parseMatchPatternFlow`, not itself, and records no wrapper
            //   node — the same "grouping parens don't survive as a node"
            //   shape as every other parenthesized-group production in this
            //   parser). Without this, `(a as x) | b` regenerated as the
            //   unparseable `a as x | b`, and `(a | b) | c` silently
            //   flattened to the three-element `a | b | c`.
            // - As **`MatchAsPattern`'s own `pattern`** (`ChildPos::Left`,
            //   `path_prec` `MATCH_AS_PATTERN`, `Assoc::Rtl`), a nested
            //   `MatchAsPattern` must be wrapped (1 == 1 on the `Rtl`
            //   dangerous side ⇒ `Yes`) but a `MatchOrPattern` must NOT be
            //   (2 > 1 ⇒ `No`): `parseMatchPatternFlow` runs its `|`-loop
            //   *before* its `as` check inside one call, so `a | b as x`
            //   already parses to `MatchAsPattern(MatchOrPattern[a, b], x)`
            //   with no parens, and adding them would be redundant output —
            //   whereas `a as y as z` does not parse at all. The round-2
            //   `ALWAYS_PAREN` catch-all cannot express this split, which
            //   is why these two kinds are classified now.
            Node::MatchAsPattern(_) => (MATCH_AS_PATTERN, Assoc::Rtl),
            Node::MatchOrPattern(_) => (MATCH_OR_PATTERN, Assoc::Ltr),

            // Every kind `parseMatchSubpatternFlow` can directly return, at
            // the tightest tier: safe bare in both of the positions above
            // (3 > 2 and 3 > 1).
            Node::MatchLiteralPattern(_)
            | Node::MatchIdentifierPattern(_)
            | Node::MatchWildcardPattern(_)
            | Node::MatchBindingPattern(_)
            | Node::MatchMemberPattern(_)
            | Node::MatchInstancePattern(_)
            | Node::MatchArrayPattern(_)
            | Node::MatchObjectPattern(_)
            | Node::MatchUnaryPattern(_) => (MATCH_SUBPATTERN, Assoc::Ltr),

            // Task 10 review round 3: `GenericTypeAnnotation`/
            // `TupleTypeAnnotation`/`TypeofTypeAnnotation`/
            // `InterfaceTypeAnnotation` are `PRIMARY` too, the same tier as
            // the primitive keyword types just above — added *because* the
            // postfix-operator fix just below routes `object_type`/
            // `element_type` through `print_child`, and each of these four
            // kinds is directly reachable there without any wrapping parens
            // in real Flow source. Every one is built straight from
            // `parsePrimaryTypeAnnotationFlow`'s own `switch`
            // (`lib/Parser/JSParserImpl-flow.cpp:3320-3617`) with no
            // wrapper: `GenericTypeAnnotation` via the `identifier`
            // fallthrough's `parseGenericTypeFlow()` call (`:3521-3525`,
            // itself returning a bare `GenericTypeAnnotationNode(id,
            // typeParams)` with no precedence-sensitive continuation —
            // `:5023-5063`), `TupleTypeAnnotation` via `case
            // TokenKind::l_square` (`:3353-3354`), `TypeofTypeAnnotation`
            // via `case TokenKind::rw_typeof` (`:3350-3351`), and
            // `InterfaceTypeAnnotation` via the `interfaceIdent_` check
            // (`:3461-3471`). Confirmed by tracing every one of the four
            // *without* an entry here through `need_parens`'s early
            // `ALWAYS_PAREN` check (`Node::MemberExpression` sibling logic,
            // below): before this fix, `A['b']['c']` printed
            // `(A)['b']['c']` — a visible regression introduced by *this
            // task's own* postfix-operator fix, not inherited debt (unlike
            // the deferred non-primitive-kind cases in other positions the
            // task's own tests still document). Confirmed each of the four
            // is safe at `PRIMARY` — cannot ever drop a load-bearing
            // paren — in every position `need_parens` can place it:
            //
            // - **Union/intersection member** (`ChildPos::Anywhere`,
            //   `path_prec` `UNION_TYPE`/`INTERSECTION_TYPE`): all four are
            //   self-delimited (identifier/dotted-chain plus a
            //   bracket-`<...>`-or-`[...]`-or-`{...}`-delimited suffix, or
            //   nothing), so nothing after the construct's own closing
            //   delimiter can be absorbed into it regardless of whether a
            //   paren wrapped it — `(A)|B` and `A|B` parse to the identical
            //   `Union[GenericTypeAnnotation(A), B]` either way.
            // - **`NullableTypeAnnotation`'s operand** (`ChildPos::Right`,
            //   `path_prec` `UNARY`): same self-delimiting argument —
            //   `?Array<T>` and `?(Array<T>)` both parse to
            //   `Nullable(GenericTypeAnnotation(Array, <T>))`; the `<T>`'s
            //   own `<`/`>` delimiters mean nothing can leak across them
            //   either way.
            // - **A `FunctionTypeAnnotation`'s `return_type`**: not
            //   affected at all by this change — `return_type` is printed
            //   through a bare `gen_node`, never `print_child`
            //   (`arms/flow_type.rs`'s `gen_function_type_annotation`), so
            //   `get_precedence` is never consulted for it either way.
            // - **Postfix base** (`ChildPos::Left`, `path_prec` `MEMBER`,
            //   the case this fix targets): `PRIMARY` > `MEMBER`,
            //   so no parens — matching real unparenthesized source exactly
            //   (`A['b']['c']`, `Array<T>['b']`, `typeof x['y']`,
            //   `[number]['length']` all parse without any grouping).
            //
            // Deliberately still `ALWAYS_PAREN` (unaffected by this
            // change): `FunctionTypeAnnotation` — genuinely needs the
            // wrap whenever it is not the sole top-level type (its own
            // `return_type` swallows the full type grammar including any
            // trailing `|`/`&`/postfix, so it can only ever reach a
            // postfix/union/intersection *parent* position via an explicit
            // extra layer of source parens, which correctly round-trips as
            // exactly one pair, `precedence.rs`'s `ALWAYS_PAREN` catch-all
            // below).
            //
            // Update (Task 12, review round 2): `KeyofTypeAnnotation`/
            // `TypeOperator`/`ConditionalTypeAnnotation`/`InferTypeAnnotation`
            // — named just above as "no dispatch arm yet" by the comment
            // this replaces — now have both an arm (`arms/newer.rs`) and a
            // `get_precedence` entry (see that task's own classifications,
            // just below this block), added specifically because their own
            // arms route a restricted-tier child through `print_child` and
            // demonstrably regressed without one (see those entries' doc
            // comments for the reproduced failures). `TypeCastExpression`
            // (Task 11) already routes both its children through
            // `print_child` at `PRIMARY`, unaffected by this.
            //
            // `TypePredicate`, `QualifiedTypeofIdentifier`,
            // `ObjectTypeAnnotation` and the TS types are left unclassified
            // and so fall to the `ALWAYS_PAREN` catch-all. An earlier version
            // of this comment justified that by asserting they "never" reach
            // `print_child`. **That was false** and the final review disproved
            // it by running: `gen_type_predicate` routes `type_annotation`
            // through `print_child` (`arms/newer.rs`, defect 26's own fix),
            // and `arms/ts.rs` has 32 `print_child` sites. The observable
            // consequence is over-parenthesization, not corruption — e.g.
            // `x is {a: number}` prints as `x is ({a: number})`, which
            // reparses to the same AST.
            //
            // Do not restore a universal claim here. Six such claims have
            // already been deleted from this crate, every one a quantifier
            // rather than an enumeration, and each was disproved by executing
            // the case rather than by reading. If you narrow one of these to a
            // real tier, derive it from the parser's descent and pin it with a
            // shape that fails without it.
            Node::GenericTypeAnnotation(_)
            | Node::TupleTypeAnnotation(_)
            | Node::TypeofTypeAnnotation(_)
            | Node::InterfaceTypeAnnotation(_) => (PRIMARY, Assoc::Ltr),

            // Task 10 review round 2: `ArrayTypeAnnotation`/
            // `IndexedAccessType`/`OptionalIndexedAccessType` are Flow's
            // postfix type operators (`T[]`, `T[K]`, `T?.[K]`) — the
            // grammar only ever builds their left operand
            // (`element_type`/`object_type`) from another postfix-tier type
            // (`lib/Parser/JSParserImpl-flow.cpp`'s
            // `parsePostfixTypeAnnotationFlow:3249-3301`), but a *literal*
            // `(LowerPrecedenceType)` grouping is legal there too — Flow's
            // `( Type )` production returns the inner type unwrapped, with
            // no wrapper node (`parseFunctionOrGroupTypeAnnotationFlow`'s
            // `if (!isFunction) { type->incParens(); return type; }`,
            // `:4016-4018`), so `(?a)[]` parses to
            // `ArrayTypeAnnotation{element_type: NullableTypeAnnotation(a)}`
            // — structurally identical to the unparenthesized `?a[]`, which
            // means something else (`Nullable(Array(a))`). Without an entry
            // here, `arms/flow_type.rs`'s `object_type`/`element_type` arms
            // had no way to tell `need_parens` these three kinds bind
            // *tighter* than `NullableTypeAnnotation`/`UnionTypeAnnotation`/
            // `IntersectionTypeAnnotation`/`FunctionTypeAnnotation`, so a
            // parenthesized lower-precedence operand silently lost its
            // parens on regeneration — a real round-trip corruption bug
            // (juno has the identical bug: `gen_js.rs:2337-2343`/
            // `2388-2397`/`2398-2408` print `element_type`/`object_type`
            // with a bare `.visit()`, never `print_child`). `MEMBER` and
            // `Assoc::Ltr` are the ones already used for the exact
            // analogous expression-precedence shape (`MemberExpression`
            // chains, `object` at `ChildPos::Left`), so this reuses them
            // rather than inventing a fourth "type-land" constant next to
            // `UNION_TYPE`/`INTERSECTION_TYPE`.
            Node::ArrayTypeAnnotation(_)
            | Node::IndexedAccessType(_)
            | Node::OptionalIndexedAccessType(_) => (MEMBER, Assoc::Ltr),

            _ => (ALWAYS_PAREN, Assoc::Ltr),
        })
    }

    /// Whether parens are needed around `child`, situated at `child_pos`
    /// relative to `path`.
    ///
    /// juno `gen_js.rs:3685-3822`. Returns `Result` — unlike juno's
    /// infallible version — because several branches classify an operator,
    /// which can fail (module doc comment's deviation #1).
    pub(crate) fn need_parens<'gc>(
        &self,
        ctx: &GCLock<'_, '_>,
        path: Path<'gc>,
        child: &'gc Node<'gc>,
        child_pos: ChildPos,
    ) -> Result<NeedParens, GenJsError> {
        if matches!(path.parent, Node::ArrowFunctionExpression(_)) {
            // An arrow's Flow return type is the one annotation position the
            // parser reads with `AllowAnonFunctionType::No` — see
            // [`flow_no_anon_region_hazard`] for the full argument, the two
            // node families that are hazards there, and the case that found
            // each.
            if path.field == NodeField::return_type && flow_no_anon_region_hazard(child) {
                return Ok(NeedParens::Yes);
            }
            // (x) => ({x: 10}) needs parens to avoid confusing it with a
            // block and a labelled statement.
            if child_pos == ChildPos::Right
                && self.expr_starts_with(ctx, child, Some(path), |n| {
                    Ok(matches!(n, Node::ObjectExpression(_)))
                })?
            {
                return Ok(NeedParens::Yes);
            }
        } else if matches!(path.parent, Node::ForStatement(_)) {
            // for((a in b);..;..) needs parens to avoid confusing it with
            // for(a in b). juno (`gen_js.rs:3705-3717`) only checks whether
            // `child` is *directly* `a in b`; that misses `for(a in b &&
            // c;;)` (bare `&&`'s LEFT operand) and `for(a && b in c;;)`
            // (its RIGHT operand) just as much as it misses the nested-
            // through-`VariableDeclarator` case the sibling branch below
            // now fixes with the same `contains_bare_in` scanner — see
            // that function's doc comment for why a full subtree walk,
            // not a direct-child check, is what ECMA-262 14.7.4's `[~In]`
            // propagation actually requires.
            //
            // `child` being a `VariableDeclaration` is special-cased to
            // never need parens at *this* level, regardless of what
            // `contains_bare_in` says about it: a `VariableDeclaration` is
            // never itself a value expression (`for(var ...)`'s head never
            // needs the whole declaration wrapped), and each declarator's
            // own `init` already gets `contains_bare_in`-protected
            // individually one level further down, by the
            // `VariableDeclarator` branch just below, when *that* print_child
            // call runs. Without this, `for (var i = (a in b);;)` — the
            // exact case this fix targets — regressed to double-wrapping:
            // `for((var i = (a in b));;)`, which fails to parse at all
            // (`var` cannot appear inside a parenthesized expression),
            // confirmed empirically while adding the `contains_bare_in`
            // fix.
            if matches!(child, Node::VariableDeclaration(_)) {
                return Ok(NeedParens::No);
            }
            return Ok(NeedParens::from(contains_bare_in(ctx, child)));
        } else if matches!(path.parent, Node::NewExpression(_)) {
            // `new(fn())` needs parens to avoid confusing it with `new fn()`.
            // Need to check the entire subtree to ensure there isn't a call
            // anywhere in it, because if there is, it would take precedence
            // and terminate the `new` early. As an example, see the
            // difference between `new(foo().bar)` (which gets `bar` on
            // `foo()`) and `new foo().bar` (which gets `bar` on
            // `new foo()`).
            if child_pos == ChildPos::Left && contains_call(child) {
                return Ok(NeedParens::Yes);
            }
            // Defect 29 (juno's, ours until Task 17). An optional chain
            // cannot be a `new` callee: ECMA-262 spells the production
            // `MemberExpression : new MemberExpression Arguments` (13.3),
            // and `OptionalExpression` is a sibling of `MemberExpression`,
            // not one of its alternatives — so `new a?.b()` is a
            // SyntaxError, and our parser says exactly that
            // ("Constructor calls may not contain an optional chain").
            // juno has no `NewExpression` optional-chain case at all
            // (`gen_js.rs:3718-3730` checks only `contains_call` and
            // `SpreadElement`), so `new (a?.b)()` and `new (a?.(1))()`
            // regenerated as unparseable text. This is the same rule the
            // `MemberExpression`/`CallExpression` branch further down
            // already applies, extended to the two remaining parents that
            // need it (`TaggedTemplateExpression` is the other; see there).
            // Found by `tests/paren_matrix.rs`'s `NewExpression.callee` row.
            if child_pos == ChildPos::Left
                && matches!(
                    child,
                    Node::OptionalMemberExpression(_) | Node::OptionalCallExpression(_)
                )
            {
                return Ok(NeedParens::Yes);
            }
            // It's illegal to place parens around spread arguments.
            if matches!(child, Node::SpreadElement(_)) {
                return Ok(NeedParens::No);
            }
        } else if matches!(path.parent, Node::ExpressionStatement(_)) {
            // Expression statement like (function () {} + 1) needs parens.
            //
            // The `starts_with_let_bracket(kind)` disjunct is a
            // **DEVIATION from juno — a correctness fix, not a
            // transcription.** See that function's doc comment: juno's
            // `ExpressionStatement` branch (`gen_js.rs:3739-3753`) guards
            // `function`/`class`/`{` at statement start but has no case at
            // all for `let[` (confirmed empirically: `"let"` does not occur
            // anywhere in `gen_js.rs`).
            //
            // `Node::MatchExpression(_)` is a second such deviation (Task 12
            // review round 4; juno predates Flow `match` entirely). A
            // statement that begins with the token `match` followed by `(`
            // is taken by `tryParseMatchStatementFlow`
            // (`crates/parser/src/js/flow/match_.rs:115`) as a match
            // *statement*, whose cases take block bodies — so an
            // unparenthesized match *expression* at statement start does not
            // merely reparse as a different node kind, it currently **panics
            // our parser** (`assertion failed: self.check(TokenKind::l_brace)`,
            // `crates/parser/src/js/statements.rs:1196`, reached from
            // `parse_block` on the case body). Reproduced live before this
            // fix: `(match (x) { _ => 1 });` regenerated as
            // `match(x){_=>1};` and `(match (x) { _ => 1 }).foo;` as
            // `match(x){_=>1}.foo;`, both panicking on reparse. This is the
            // exact hazard the three kinds above already guard, and
            // `root_starts_with`'s left-spine walk is the right mechanism
            // here (unlike for `RecordExpression` — see that kind's
            // `get_precedence` entry): wrapping the whole statement
            // expression rescues every tail shape, verified for
            // `(match (x) {…}.foo);`, `(…());`, `(…[0]);`, `(… + 1);`,
            // `(…, 2);` and `(… ? a : b);`.
            // Defect 34 (ours and juno's): the *reverse* of the directive
            // hazard `arms/stmt.rs`'s `gen_expression_statement` already
            // guards.
            //
            // That arm handles "this statement IS a directive, so reprint
            // its raw source spelling rather than the cooked value". This
            // is the other direction: a statement whose expression is a
            // bare `StringLiteral` and whose `directive` is **absent** is
            // not a directive — `("s");` is the canonical spelling, because
            // ECMA-262's `Directive Prologue` (11.2.1) is defined over
            // `ExpressionStatement`s "consisting entirely of a
            // StringLiteral token", and a parenthesized one does not
            // qualify. Reprinting it as `"s";` at the head of a
            // function/script body makes it a directive on reparse, which
            // populates `directive` and — for `"use strict"` — **flips
            // strictness**. Measured: `("s");` regenerated as `'s';`, whose
            // tree differs by the `directive` property, in both `Pretty`
            // modes.
            //
            // The condition is a direct `StringLiteral` child, not
            // `root_starts_with`: a prologue entry must be the *entire*
            // expression, so `"a" + "b";` is already safe. Where the
            // statement is not in prologue position the parens are inert
            // (`directive` is absent either way), which is the conservative
            // side and costs two characters. Found by
            // `tests/paren_matrix.rs`'s `ExpressionStatement.expression`
            // row.
            let ExpressionStatement {
                metadata: _,
                expression: _,
                directive,
            } = match path.parent {
                Node::ExpressionStatement(inner) => inner,
                _ => unreachable!("matched Node::ExpressionStatement just above"),
            };
            if matches!(child, Node::StringLiteral(_))
                && ctx.try_bytes_str(directive.get()).is_none()
            {
                return Ok(NeedParens::Yes);
            }
            let starts_with_disallowed =
                self.root_starts_with(ctx, child, |kind| -> Result<bool, GenJsError> {
                    Ok(matches!(
                        kind,
                        Node::FunctionExpression(_)
                            | Node::ClassExpression(_)
                            | Node::ObjectExpression(_)
                            | Node::ObjectPattern(_)
                            | Node::MatchExpression(_)
                    ) || starts_with_let_bracket(ctx, kind))
                })?;
            return Ok(NeedParens::from(starts_with_disallowed));
        } else if matches!(path.parent, Node::ExportDefaultDeclaration(_)) {
            // `export default function(){}` / `export default class{}`,
            // unparenthesized, always parse back as a `FunctionDeclaration`/
            // `ClassDeclaration` — ECMA-262's own `ExportDeclaration :
            // export default [lookahead ∉ {function, async function,
            // class}] AssignmentExpression ;` production excludes exactly
            // those three leading tokens, so an *expression* (a bare
            // `FunctionExpression`/`ClassExpression`, from source like
            // `export default (function(){})`) whose left-recursive spine
            // starts with one of them must be wrapped, or it silently
            // becomes a `FunctionDeclaration`/`ClassDeclaration` on reparse
            // — a node-kind flip, not a formatting difference. A
            // `declaration` that already IS a `FunctionDeclaration`/
            // `ClassDeclaration` (the grammar's other two productions'
            // shape) is untouched: `root_starts_with`'s predicate below only
            // matches the *Expression* kinds, never the *Declaration* ones.
            //
            // **DEVIATION from juno — a correctness fix, not a
            // transcription.** juno's `ExportDefaultDeclaration` arm
            // (`gen_js.rs:1922-1928`) is a bare `declaration.visit(...)`,
            // and juno's own `need_parens` (`gen_js.rs:3685-3822`) has no
            // `ExportDefaultDeclaration` branch at all — so upstream juno
            // has the identical bug. Modeled on the sibling
            // `ExpressionStatement` branch just above, minus the `{`/
            // `ObjectPattern`/`let[` disjuncts: those guard against
            // `ExpressionStatement`'s *own* ASI/block-vs-object hazards,
            // which `export default` does not share (its production's
            // negative lookahead names only `function`/`async function`/
            // `class`, 14.2.1).
            let starts_with_disallowed =
                self.root_starts_with(ctx, child, |kind| -> Result<bool, GenJsError> {
                    Ok(matches!(
                        kind,
                        Node::FunctionExpression(_) | Node::ClassExpression(_)
                    ))
                })?;
            return Ok(NeedParens::from(starts_with_disallowed));
        } else if matches!(path.parent, Node::VariableDeclarator(_)) {
            // `var x = a in b;` needs parens around `a in b` — not because
            // this particular statement is ambiguous (it isn't, standing
            // alone), but because a `VariableDeclarator`'s `init` is also
            // reached this way when the declarator sits inside a
            // `for(...)` head's `[~In]` clause (ECMA-262 14.7.4):
            // `for (var i = (a in b);;)` only parses *because* of the
            // literal source parens (`( Expression )` resets to `[+In]`),
            // and the AST retains no trace of them — `GenJS::gen_variable_declarator`'s
            // doc comment has the full explanation.
            //
            // **DEVIATION from juno — a correctness fix, not a
            // transcription.** juno's `VariableDeclarator` arm
            // (`gen_js.rs:1369-1387`) prints `init` with a bare
            // `init.visit(...)`, never through `need_parens` at all, so it
            // has no way to protect this case; ours routes `init` through
            // `print_child` specifically so this branch can. `Path` carries
            // only one ancestor level (see its own doc comment on
            // `hermes_ast::visitor::Path`), so — unlike the existing
            // `ForStatement` branch just above, which only protects a
            // *bare* `for((a in b);;)` init — this can't tell from here
            // whether the enclosing `VariableDeclaration` is itself a
            // `for` head or an ordinary statement, and parenthesizes
            // unconditionally. A redundant `(a in b)` in `var x = (a in
            // b);` as a plain top-level statement is semantically inert;
            // spec §7 makes round-trip correctness, not minimal output,
            // this crate's bar, and the plan's spec explicitly rules out a
            // byte-exact oracle for this generator.
            //
            // A *direct* `is_binary_op(ctx, child, In)` check (this
            // branch's original shape) is not enough: ECMA-262's `[~In]`
            // parameter propagates through *both* operands of every
            // binary/logical/conditional/assignment production nested in
            // the declarator's init, not just a bare top-level `a in b` —
            // `for (var i = (a && b in c);;);` is exactly as broken (a
            // live reparse failure, confirmed empirically:
            // `')' expected after 'for(... in/of ...'`), since `b in c`
            // is `&&`'s *right* operand, a position no ordinary
            // precedence-based parenthesization protects either (`in`
            // binds tighter than `&&`, so nothing else adds parens
            // there). `contains_bare_in` is the fix: a full-subtree scan,
            // not a direct-child check.
            return Ok(NeedParens::from(contains_bare_in(ctx, child)));
        } else if matches!(
            path.parent,
            Node::ClassDeclaration(_) | Node::ClassExpression(_)
        ) && path.field == NodeField::super_class
        {
            // `class C extends <heritage> {}` — Task 12 review round 5.
            //
            // **DEVIATION from juno — a correctness fix, not a
            // transcription.** juno's class arms (`gen_js.rs:1553-1587`)
            // print `super_class` with a bare `.visit()`, and juno's own
            // `need_parens` has no class branch at all, so upstream juno
            // drops heritage parens too.
            //
            // The heritage slot is not an arbitrary expression: ECMA-262
            // spells it `ClassHeritage : extends LeftHandSideExpression`,
            // and our parser matches that exactly —
            // `crates/parser/src/js/classes.rs:437-438` calls
            // `parse_left_hand_side_expression(IsClassHeritageArgument::Yes)`,
            // not `parse_assignment_expression`. So anything looser than
            // LHS tier reached this field only through explicit source
            // parens, and printing it bare emits source that does not
            // parse. Measured against the parser as shipped, bare heritage
            // is REJECTED for every one of `a = b`, `a ? b : c`, `a + b`,
            // `() => 1`, `a, b`, `!a`, `typeof a`, `a++`, `++a`, `a || b`,
            // `a ?? b`, `-a`, `a ** b` — and ACCEPTED for every one of
            // `a`, `a.b`, `a[0]`, `a()`, `a.b()`, `new Foo`, `new Foo()`,
            // ``a`t` ``, `import("x")`, `this`, `super.x`, `class {}`,
            // `function(){}`, `[1]`, `{}`, `` `t` ``, `null`, `1`, `"s"`,
            // `/re/`, `a?.b`, `a?.()`.
            //
            // That boundary falls exactly at [`TAGGED_TEMPLATE`] in this
            // table: every accepted kind classifies at `TAGGED_TEMPLATE`
            // (tagged templates, `import()`), `NEW_NO_ARGS`, `MEMBER` or
            // `PRIMARY`; every rejected one at `POST_UPDATE` or below. So
            // the rule is a single threshold rather than a kind list, and
            // an unclassified child (`ALWAYS_PAREN`, 0) is wrapped too,
            // which is the conservative side.
            //
            // The threshold also picks up [`RECORD_EXPRESSION`] (28 <
            // `TAGGED_TEMPLATE`), which needs the parens here for an
            // independent reason: the parser disables the record-expression
            // branch entirely in heritage position
            // (`isClassHeritageArgument != IsClassHeritageArgument::Yes`
            // guards both the type-argument commit and the branch itself,
            // `lib/Parser/JSParserImpl.cpp:4049-4053` and `:4077-4080`). Unlike the
            // plain-JS cases above, dropping these parens corrupts
            // **silently**: `class C extends (R {p:1}) {}` regenerated as
            // `class C extends R {p: 1} {}`, which reparses without error
            // to `super_class: Identifier(R)` plus a `ClassProperty p: 1`
            // plus a stray empty `BlockStatement` — a different tree, no
            // diagnostic. `MatchExpression` by contrast is `PRIMARY` and
            // correctly stays bare here (verified: `class C extends
            // match (x) { _ => 1 } {}` reparses identically).
            //
            // A `SequenceExpression` child gets a redundant second pair
            // (`extends ((a, b))`), since `gen_sequence_expression` already
            // writes its own — the same pre-existing double-wrap
            // `x = (a, 2)` → `x = ((a, 2))` has everywhere else in this
            // crate. Harmless, reparses identically, and left consistent
            // with the rest rather than special-cased here.
            let (child_prec, _) = self.get_precedence(ctx, child)?;
            return Ok(NeedParens::from(child_prec < TAGGED_TEMPLATE));
        } else if is_full_ts_type_field(path) {
            // A TypeScript type position parsed by a full
            // `parse_type_annotation_ts` — Task 13 fix round 1.
            //
            // There is no *precedence* question here: every TS type is legal
            // bare in a full-`Type` slot, which is why these fields printed
            // with a plain `gen_node` until now. The question is whether the
            // child is a TS type **at all**: our parser's `(`-cover hands
            // back `parse_binding_element` results (an `ObjectPattern`, an
            // `ArrayPattern`, an `AssignmentPattern`) as the type, and those
            // print as `{a: A}` / `[a, b]`, which reparse as a
            // `TSTypeLiteral` / `TSTupleType`. See
            // [`is_narrowed_ts_type_field`]'s doc comment for the full
            // mechanism and the measured cases.
            //
            // Note this returns `No` for every genuine TS type, so it adds
            // no parens anywhere the old bare `gen_node` did not — the
            // change is strictly "wrap the intruders".
            return Ok(NeedParens::from(!is_ts_type_node(child)));
        } else if is_narrowed_ts_type_field(path) && !is_ts_type_node(child) {
            // The same intruder rule for the five *narrowed* TS type fields.
            // A genuine TS type child falls through to the tier comparison
            // below (and, for `check_type`, to the threshold branch next),
            // which is what this `&&` preserves.
            return Ok(NeedParens::Yes);
        } else if matches!(path.parent, Node::TSConditionalType(_))
            && path.field == NodeField::check_type
        {
            // `Check extends E ? T : F` — Task 13. A threshold, not the
            // ordinary precedence comparison, for the same reason the
            // class-heritage branch above is one: the *parent's* own tier is
            // not the tier its child is parsed at.
            //
            // `parse_type_annotation_ts`
            // (`crates/parser/src/js/ts/types.rs`) builds the conditional
            // out of a `result` it has already parsed, and only *then*
            // checks for a trailing `extends`. `result` came from the
            // union-tier `parse_ts_union_type` — or, for the three
            // wider-than-union shapes that function can also produce (a
            // `new`-constructor type, a `<T>`-generic function type, and an
            // `id is T` predicate), from a production whose *own* trailing
            // sub-parse is a full `parse_type_annotation_ts` that would
            // swallow the `extends` clause. So the boundary is exactly
            // [`TS_UNION_TYPE`]: union tier and tighter print bare,
            // everything looser must be wrapped.
            //
            // Measured before this branch existed (the plain comparison
            // used the conditional's own [`TS_CONDITIONAL_TYPE`] as
            // `path_prec`, so a function type at [`TS_FUNCTION_TYPE`]
            // compared as *higher* and printed bare):
            // `type T = ((a: X) => Y) extends B ? C : D;` regenerated as
            // `type T = (a: X) => Y extends B ? C : D;`, which reparses to a
            // single `TSFunctionType` whose *return type* is the conditional
            // — a different tree, no diagnostic. Regression test:
            // `ts_conditional_type_check_type_keeps_parens_around_a_function_type`.
            let (child_prec, _) = self.get_precedence(ctx, child)?;
            return Ok(NeedParens::from(child_prec < TS_UNION_TYPE));
        } else if (is_unary_op(ctx, path.parent, UnaryExpressionOperator::Minus)?
            && self.root_starts_with(ctx, child, |n| check_minus(ctx, n))?)
            || (is_unary_op(ctx, path.parent, UnaryExpressionOperator::Plus)?
                && self.root_starts_with(ctx, child, |n| check_plus(ctx, n))?)
            || (child_pos == ChildPos::Right
                && is_binary_op(ctx, path.parent, BinaryExpressionOperator::Minus)?
                && self.root_starts_with(ctx, child, |n| check_minus(ctx, n))?)
            || (child_pos == ChildPos::Right
                && is_binary_op(ctx, path.parent, BinaryExpressionOperator::Plus)?
                && self.root_starts_with(ctx, child, |n| check_plus(ctx, n))?)
        {
            // -(-x) or -(--x) or -(-5)
            // +(+x) or +(++x)
            // a-(-x) or a-(--x) or a-(-5)
            // a+(+x) or a+(++x)
            return Ok(if self.pretty() == Pretty::Yes {
                NeedParens::Yes
            } else {
                NeedParens::Space
            });
        } else if child_pos == ChildPos::Left
            && is_binary_op(ctx, path.parent, BinaryExpressionOperator::Exp)?
            && matches!(
                child,
                Node::UnaryExpression(_) | Node::AwaitExpression(_) | Node::TSTypeAssertion(_)
            )
        {
            // `(-x) ** y` — the left operand of `**` must be parenthesized
            // when it is a unary expression.
            //
            // **DEVIATION from juno — a correctness fix found by the Tier 2
            // sweep** (`test/hermes/bigint-binary-exponentiate.js`'s
            // `(-BigInt(2)) ** BigInt(63)`). This is *not* a precedence
            // question, which is why the tier comparison below cannot catch
            // it: `UNARY` is higher than `Exp` in this table, correctly, and
            // so the child prints bare. ECMA-262 13.6 spells the production
            // `ExponentiationExpression : UpdateExpression ** ExponentiationExpression`
            // — a `UnaryExpression` on the left is a **syntax error**, not a
            // reassociation, and both parsers say so
            // (`crates/parser/src/js/expressions.rs:2566-2576`,
            // "Unary operator before ** must use parens to disambiguate").
            // A *prefix* `UpdateExpression` (`--x ** y`) is legal and is
            // deliberately not in the list. `AwaitExpression` and
            // `TSTypeAssertion` are the other two kinds this table classifies
            // at the unary tier; parenthesizing them is right whether the
            // parser rejects them bare or silently reads them as
            // `await (x ** y)`.
            //
            // `tests/paren_matrix.rs`'s `BinaryExpression.left.exp` row
            // reaches the same conclusion independently, from the other
            // direction: all five of `(-a) ** b`, `(typeof a) ** b`,
            // `(void a) ** b`, `(delete a.b) ** b`, `(!a) ** b` regenerated
            // as text that does not parse, in both `Pretty` modes.
            return Ok(NeedParens::Yes);
        } else if matches!(
            path.parent,
            Node::MemberExpression(_)
                | Node::CallExpression(_)
                | Node::TaggedTemplateExpression(_)
        ) && matches!(
            child,
            Node::OptionalMemberExpression(_) | Node::OptionalCallExpression(_)
        ) && child_pos == ChildPos::Left
        {
            // When optional chains are terminated by non-optional
            // member/calls, we need the left hand side to be
            // parenthesized. Avoids confusing `(a?.b).c` with `a?.b.c`.
            //
            // `TaggedTemplateExpression` is a **DEVIATION from juno** and
            // part of defect 29 (see the `NewExpression` branch above for
            // the other half). juno's branch (`gen_js.rs:3771-3780`) names
            // only `MemberExpression`/`CallExpression`. A tag is also a
            // `MemberExpression` in the grammar
            // (`MemberExpression : MemberExpression TemplateLiteral`,
            // 13.3), so an optional chain cannot be tagged either: measured
            // before this fix, ``(a?.b)`q` `` regenerated as ``a?.b`q` ``,
            // which our parser rejects with "invalid use of tagged template
            // literal in optional chain". Found by
            // `tests/paren_matrix.rs`'s `TaggedTemplateExpression.tag` row.
            return Ok(NeedParens::Yes);
        } else if matches!(
            path.parent,
            Node::IndexedAccessType(_) | Node::ArrayTypeAnnotation(_)
        ) && matches!(child, Node::OptionalIndexedAccessType(_))
            && child_pos == ChildPos::Left
        {
            // Defect 30 (ours and juno's): the Flow type-land twin of the
            // optional-chain rule just above. `A?.[K]` is Flow's optional
            // indexed access, and — exactly like an ES optional chain — the
            // optionality propagates to the whole postfix chain, so
            // `A?.[K][K]` is one `OptionalIndexedAccessType` chain, not an
            // `IndexedAccessType` over an optional one. Both kinds sit at
            // [`MEMBER`]/`Assoc::Ltr` (see their `get_precedence` entry), so
            // the ordinary comparison leaves an equal-precedence *left*
            // child bare and `(A?.[K])[K]` silently lost its parens.
            // Found by `tests/paren_matrix.rs`'s
            // `IndexedAccessType.objectType` row.
            return Ok(NeedParens::Yes);
        } else if matches!(
            path.parent,
            Node::AsExpression(_) | Node::AsConstExpression(_) | Node::TSAsExpression(_)
        ) && child_pos == ChildPos::Left
        {
            // Defect 32 (ours; juno predates `as` entirely). The `as`
            // operand's tier is not the tier [`AS_EXPRESSION`] gives the
            // `as` expression *as a child*.
            //
            // [`AS_EXPRESSION`] is deliberately 6 — below every binary
            // operator — because an `x as T` appearing as somebody else's
            // child has a *type* on its right and the type grammar keeps
            // reading past the end of the type (see that constant's doc
            // comment). But the parser builds `as` in the ordinary
            // precedence-climbing binary loop at precedence 8, the same
            // tier as `in`/`instanceof`
            // (`crates/parser/src/js/expressions.rs`'s `as_operator`), so
            // the *left* operand's real neighbour tier is
            // `get_binary_precedence(In)`, not 6. Using 6 there made
            // `need_parens` believe every binary and logical operand bound
            // tighter than `as`: `(a || b) as T` regenerated as
            // `a || b as T`, which reparses as `a || (b as T)` — a
            // different tree, no diagnostic. Measured on all three
            // spellings (`AsExpression`, `AsConstExpression`,
            // `TSAsExpression`) x `||`/`??`.
            //
            // A nested `as` on the left keeps its own real tier, so the
            // genuinely left-associative `x as A as B` stays bare (which
            // `arms/newer.rs`'s regression test asserts on exact text).
            // Found by `tests/paren_matrix.rs`'s `AsExpression.expression`
            // row.
            let as_tier = get_binary_precedence(BinaryExpressionOperator::In);
            let (child_prec, _) = self.get_precedence(ctx, child)?;
            let child_tier = if matches!(
                child,
                Node::AsExpression(_) | Node::AsConstExpression(_) | Node::TSAsExpression(_)
            ) {
                as_tier
            } else {
                child_prec
            };
            return Ok(NeedParens::from(child_tier < as_tier));
        } else if (check_and_or(ctx, path.parent)? && check_nullish(ctx, child)?)
            || (check_nullish(ctx, path.parent)? && check_and_or(ctx, child)?)
        {
            // Nullish coalescing always requires parens when mixed with any
            // other logical operations.
            return Ok(NeedParens::Yes);
        } else if matches!(
            path.parent,
            Node::CallExpression(_) | Node::OptionalCallExpression(_)
        ) && matches!(child, Node::SpreadElement(_))
        {
            // It's illegal to place parens around spread arguments.
            return Ok(NeedParens::No);
        } else if matches!(path.parent, Node::AssignmentExpression(_))
            && matches!(child, Node::ObjectPattern(_) | Node::ArrayPattern(_))
            && child_pos == ChildPos::Left
        {
            // Avoid parentheses for destructuring patterns.
            return Ok(NeedParens::No);
        } else if matches!(path.parent, Node::AssignmentExpression(_))
            && matches!(child, Node::ObjectExpression(_) | Node::ArrayExpression(_))
            && child_pos == ChildPos::Left
        {
            // `([a, b]) = t` — the mirror image of the branch just above.
            //
            // **DEVIATION from juno — a correctness fix found by the Tier 2
            // sweep** (`test/Parser/es6/reparse-array-destr.js`). When an
            // array/object literal is the assignment target, the parser
            // *reparses* it into an `ArrayPattern`/`ObjectPattern`
            // (`reparseAssignmentPattern`) — unless the source parenthesized
            // it, which is exactly what leaves the `ArrayExpression` in
            // place for sema to reject as an "invalid assignment left-hand
            // side". So a tree with a literal here is only spellable with
            // the parens, and printing it bare reparses to a *Pattern*: a
            // different tree, with no diagnostic. Because a valid program
            // never has a literal in this position, this branch can only
            // fire on trees that are already sema errors, and adds no
            // parens anywhere a correct program would notice.
            return Ok(NeedParens::Yes);
        } else if matches!(path.parent, Node::AssignmentExpression(_))
            && child_pos == ChildPos::Left
        {
            // Defect 33 (ours and juno's): the rest of the same rule, as a
            // tier threshold rather than a kind list.
            //
            // ECMA-262 spells assignment
            // `AssignmentExpression : LeftHandSideExpression = AssignmentExpression`
            // (13.15), so anything looser than LHS tier reached this field
            // only through explicit source parens — the same argument, and
            // the same shape of fix, as the `ClassDeclaration`/
            // `ClassExpression` `super_class` branch above. The threshold
            // is [`MEMBER`]: `MemberExpression`, `OptionalMemberExpression`,
            // `CallExpression`, `OptionalCallExpression`, `MetaProperty`
            // and `NewExpression` all classify there or higher, as do the
            // primaries; everything the grammar excludes classifies below.
            //
            // Measured before this branch: `(a ? b : c) = y` regenerated as
            // `a ? b : c = y`, which reparses **without a diagnostic** to
            // `ConditionalExpression{alternate: AssignmentExpression}` — a
            // completely different tree. Like the `ArrayExpression`/
            // `ObjectExpression` branch just above, every tree this fires on
            // is already a sema error ("invalid assignment left-hand side"),
            // because a valid program cannot put a conditional there; the
            // point is that the *parse* trees must still round-trip, since
            // this crate's domain is what the parser produces, not what sema
            // accepts. Found by `tests/paren_matrix.rs`'s
            // `AssignmentExpression.left` row.
            let (child_prec, _) = self.get_precedence(ctx, child)?;
            return Ok(NeedParens::from(child_prec < MEMBER));
        } else if matches!(path.parent, Node::UpdateExpression(_)) {
            // Defect 33, second half: `(++a)++`.
            //
            // ECMA-262 13.4 spells both update productions over a
            // `LeftHandSideExpression` (`UpdateExpression : LeftHandSideExpression ++`
            // and `UpdateExpression : ++ UnaryExpression` — note the prefix
            // form's operand is a `UnaryExpression`, which *is* wider, but
            // its own `AssignmentTargetType` requirement still excludes
            // everything below LHS tier). The same [`MEMBER`] threshold as
            // the `AssignmentExpression` branch above therefore applies,
            // and for the same reason: a looser child got here only through
            // explicit source parens.
            //
            // Measured before this branch: `(++a)++` regenerated as
            // `++a++`, which reparses as `++(a++)` — the two
            // `UpdateExpression`s swap roles, silently. Found by
            // `tests/paren_matrix.rs`'s
            // `UpdateExpression.argument.postfix` row.
            let (child_prec, _) = self.get_precedence(ctx, child)?;
            return Ok(NeedParens::from(child_prec < MEMBER));
        }

        let (child_prec, _child_assoc) = self.get_precedence(ctx, child)?;
        if child_prec == ALWAYS_PAREN {
            return Ok(NeedParens::Yes);
        }

        let (path_prec, path_assoc) = self.get_precedence(ctx, path.parent)?;

        if child_prec < path_prec {
            // Child is definitely a danger.
            return Ok(NeedParens::Yes);
        }
        if child_prec > path_prec {
            // Definitely cool.
            return Ok(NeedParens::No);
        }
        // Equal precedence, so associativity (rtl/ltr) is what matters.
        if child_pos == ChildPos::Anywhere {
            // Child could be anywhere, so always paren.
            return Ok(NeedParens::Yes);
        }
        if child_prec == TOP {
            // Both precedences are safe.
            return Ok(NeedParens::No);
        }
        // Check if child is on the dangerous side.
        Ok(NeedParens::from(if path_assoc == Assoc::Rtl {
            child_pos == ChildPos::Left
        } else {
            child_pos == ChildPos::Right
        }))
    }

    /// `root_starts_with(ctx, expr, pred)` — whether the left-recursive
    /// spine of `expr` (with no path, i.e. not itself needing parens as
    /// someone's child) starts with a node satisfying `pred`.
    ///
    /// juno `gen_js.rs:3824-3831`. Returns `Result` — see
    /// [`GenJS::expr_starts_with`], which this delegates to.
    pub(crate) fn root_starts_with<'gc, F: Fn(&'gc Node<'gc>) -> Result<bool, GenJsError>>(
        &self,
        ctx: &GCLock<'_, '_>,
        expr: &'gc Node<'gc>,
        pred: F,
    ) -> Result<bool, GenJsError> {
        self.expr_starts_with(ctx, expr, None, pred)
    }

    /// Whether the left-recursive spine of `expr` — the same left child that
    /// would print first with no separating parens — starts with a node
    /// satisfying `pred`. Used to decide statement-level parens (e.g.
    /// whether an `ExpressionStatement` needs to wrap its expression because
    /// it would otherwise start with `function`/`class`/`{`).
    ///
    /// juno `gen_js.rs:3833-3923`. Returns `Result`, and `pred` itself
    /// returns `Result<bool, GenJsError>` rather than juno's plain `bool`:
    /// this walk calls `need_parens`, which is fallible (module doc
    /// comment's deviation #1), and every caller in this file passes a `pred`
    /// built from `check_minus`/`check_plus`, themselves fallible for the
    /// same reason.
    pub(crate) fn expr_starts_with<'gc, F: Fn(&'gc Node<'gc>) -> Result<bool, GenJsError>>(
        &self,
        ctx: &GCLock<'_, '_>,
        expr: &'gc Node<'gc>,
        path: Option<Path<'gc>>,
        pred: F,
    ) -> Result<bool, GenJsError> {
        if let Some(path) = path {
            if self.need_parens(ctx, path, expr, ChildPos::Left)? == NeedParens::Yes {
                return Ok(false);
            }
        }

        if pred(expr)? {
            return Ok(true);
        }

        // Ensure the recursive calls are the last things to run, hopefully
        // the compiler makes this into a loop.
        match expr {
            Node::CallExpression(CallExpression {
                metadata: _,
                callee,
                type_arguments: _,
                arguments: _,
            }) => {
                self.expr_starts_with(ctx, callee, Some(Path::new(expr, NodeField::callee)), pred)
            }
            Node::OptionalCallExpression(OptionalCallExpression {
                metadata: _,
                callee,
                type_arguments: _,
                arguments: _,
                optional: _,
            }) => {
                self.expr_starts_with(ctx, callee, Some(Path::new(expr, NodeField::callee)), pred)
            }
            Node::BinaryExpression(BinaryExpression {
                metadata: _,
                left,
                right: _,
                operator: _,
            }) => self.expr_starts_with(ctx, left, Some(Path::new(expr, NodeField::left)), pred),
            Node::LogicalExpression(LogicalExpression {
                metadata: _,
                left,
                right: _,
                operator: _,
            }) => self.expr_starts_with(ctx, left, Some(Path::new(expr, NodeField::left)), pred),
            Node::ConditionalExpression(ConditionalExpression {
                metadata: _,
                test,
                alternate: _,
                consequent: _,
            }) => self.expr_starts_with(ctx, test, Some(Path::new(expr, NodeField::test)), pred),
            Node::AssignmentExpression(AssignmentExpression {
                metadata: _,
                operator: _,
                left,
                right: _,
            }) => self.expr_starts_with(ctx, left, Some(Path::new(expr, NodeField::left)), pred),
            Node::UpdateExpression(UpdateExpression {
                metadata: _,
                operator: _,
                argument,
                prefix,
            }) => Ok(!prefix.get()
                && self.expr_starts_with(
                    ctx,
                    argument,
                    Some(Path::new(expr, NodeField::argument)),
                    pred,
                )?),
            Node::UnaryExpression(UnaryExpression {
                metadata: _,
                operator: _,
                argument,
                prefix,
            }) => Ok(!prefix.get()
                && self.expr_starts_with(
                    ctx,
                    argument,
                    Some(Path::new(expr, NodeField::argument)),
                    pred,
                )?),
            Node::MemberExpression(MemberExpression {
                metadata: _,
                object,
                property: _,
                computed: _,
            })
            | Node::OptionalMemberExpression(OptionalMemberExpression {
                metadata: _,
                object,
                property: _,
                computed: _,
                optional: _,
            }) => {
                self.expr_starts_with(ctx, object, Some(Path::new(expr, NodeField::object)), pred)
            }
            Node::TaggedTemplateExpression(TaggedTemplateExpression {
                metadata: _,
                tag,
                quasi: _,
            }) => self.expr_starts_with(ctx, tag, Some(Path::new(expr, NodeField::tag)), pred),
            _ => Ok(false),
        }
    }

    /// Print `child`'s parens (or space) then `child` itself, if `child` is
    /// `Some`.
    ///
    /// juno `gen_js.rs:3248-3264`. Returns `Result` — see the module doc
    /// comment's deviation #2.
    pub(crate) fn print_child<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        child: Option<&'gc Node<'gc>>,
        path: Path<'gc>,
        child_pos: ChildPos,
    ) -> Result<(), GenJsError> {
        if let Some(child) = child {
            let np = self.need_parens(ctx, path, child, child_pos)?;
            self.print_parens(ctx, child, path, np)?;
        }
        Ok(())
    }

    /// Print one expression in a comma-separated sequence. It needs parens
    /// if its precedence is `<=` comma's.
    ///
    /// juno `gen_js.rs:3266-3280`. Returns `Result` — see the module doc
    /// comment's deviation #2.
    pub(crate) fn print_comma_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        child: &'gc Node<'gc>,
        path: Path<'gc>,
    ) -> Result<(), GenJsError> {
        let need = NeedParens::from(self.get_precedence(ctx, child)?.0 <= SEQ);
        self.print_parens(ctx, child, path, need)
    }

    /// Print `need_parens`'s parens (or space) around `child`, then `child`
    /// itself.
    ///
    /// juno `gen_js.rs:3282-3298`. `child.visit(ctx, self, Some(path))`
    /// becomes `self.gen_node(ctx, child, Some(path))?` (plan Adaptation
    /// Rules); this — not the parens themselves — is why the function
    /// becomes fallible. See the module doc comment's deviation #2.
    pub(crate) fn print_parens<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        child: &'gc Node<'gc>,
        path: Path<'gc>,
        need_parens: NeedParens,
    ) -> Result<(), GenJsError> {
        if need_parens == NeedParens::Yes {
            out!(self, "(");
        } else if need_parens == NeedParens::Space {
            out!(self, " ");
        }
        self.gen_node(ctx, child, Some(path))?;
        if need_parens == NeedParens::Yes {
            out!(self, ")");
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Free-function helpers.
// ---------------------------------------------------------------------------

/// Whether `node` is a node kind the **TypeScript type grammar** can
/// produce — i.e. something that is legal, unparenthesized, wherever a TS
/// type is expected.
///
/// Task 13 fix round 1. This is the allow-list half of the "an intruder in a
/// TS type position must be wrapped" rule (see [`is_full_ts_type_field`] and
/// [`is_narrowed_ts_type_field`]). It is deliberately an **allow**-list, not
/// a deny-list of the intruders: the set of node kinds
/// `parse_ts_primary_type` and its callees can return is closed and small,
/// while the set of things `parse_binding_element` can hand back through the
/// `(`-cover is open-ended (see the fix-round section of
/// `task-13-report.md`), so an unrecognized kind lands on the conservative
/// side — wrapped.
///
/// The membership is exactly `parse_ts_primary_type`'s own returns
/// (`crates/parser/src/js/ts/types.rs`) plus the three kinds only the wider
/// entry points build (`TSFunctionType`, `TSConstructorType`,
/// `TSTypePredicate`, `TSConditionalType`) and the `TSTypeAnnotation`
/// wrapper. Two entries deserve a note:
///
/// - **`ExistsTypeAnnotation`** (`*`) is in the list even though its name is
///   Flow's: `parse_ts_primary_type`'s `star` arm builds one. It must NOT be
///   wrapped — `(*)` does not parse at all, because the `(`-cover reaches
///   `parse_binding_element`, which rejects `*`.
/// - **`TSQualifiedName`** IS in the list, even though it is a type *name*
///   (`TSTypeReference::type_name`, `TSTypeQuery::expr_name`,
///   `TSModuleMember::id`) rather than a type, and no printer routes it
///   through `print_child` today. It was left out of the first draft of this
///   list on that reasoning, and
///   `the_ts_tier_table_and_the_ts_type_allow_list_agree` immediately caught
///   the disagreement with [`GenJS::get_precedence`], which classifies it at
///   [`TS_PRIMARY_TYPE`]. Being in the list is the correct side of that
///   disagreement: `(A.B)` does not parse (the `(`-cover reaches
///   `parse_binding_element`, which stops at the `.`), so if a future arm
///   ever did `print_child` a qualified name, wrapping it would be wrong.
pub(crate) fn is_ts_type_node(node: &Node<'_>) -> bool {
    matches!(
        node,
        Node::TSAnyKeyword(_)
            | Node::TSNumberKeyword(_)
            | Node::TSBooleanKeyword(_)
            | Node::TSStringKeyword(_)
            | Node::TSSymbolKeyword(_)
            | Node::TSVoidKeyword(_)
            | Node::TSUndefinedKeyword(_)
            | Node::TSUnknownKeyword(_)
            | Node::TSNeverKeyword(_)
            | Node::TSBigIntKeyword(_)
            | Node::TSThisType(_)
            | Node::TSLiteralType(_)
            | Node::TSIndexedAccessType(_)
            | Node::TSArrayType(_)
            | Node::TSTypeReference(_)
            | Node::TSQualifiedName(_)
            | Node::TSFunctionType(_)
            | Node::TSConstructorType(_)
            | Node::TSTypePredicate(_)
            | Node::TSTupleType(_)
            | Node::TSUnionType(_)
            | Node::TSIntersectionType(_)
            | Node::TSTypeQuery(_)
            | Node::TSConditionalType(_)
            | Node::TSTypeLiteral(_)
            | Node::TSInterfaceDeclaration(_)
            | Node::TSTypeAnnotation(_)
            | Node::ExistsTypeAnnotation(_)
    )
}

/// Whether `path` names a TypeScript type position parsed by a **full**
/// `parse_type_annotation_ts` — one where any type at all is legal
/// unparenthesized.
///
/// Task 13 fix round 1. These fields need [`GenJS::print_child`] not for a
/// precedence reason (there is none — every TS type prints bare here) but
/// purely so the intruder rule below can run: our parser can and does put
/// **expression-space** nodes in these slots. See
/// [`is_narrowed_ts_type_field`] for the mechanism and the measured damage.
///
/// Deliberately keyed on `(parent kind, field)` rather than kind alone,
/// because three parents carry both a type field and a non-type field of a
/// different name: `TSIndexedAccessType` (`index_type` here,
/// `object_type` narrowed), `TSConditionalType` (three full fields here,
/// `check_type` narrowed), and `TSAsExpression`/`TSTypeAssertion`
/// (`type_annotation` here, `expression` an *expression* position that must
/// keep its ordinary precedence handling).
pub(crate) fn is_full_ts_type_field(path: Path<'_>) -> bool {
    use NodeField as F;
    match path.parent {
        Node::TSTypeAnnotation(_) => path.field == F::type_annotation,
        Node::TSTypeAliasDeclaration(_) => path.field == F::type_annotation,
        Node::TSIndexedAccessType(_) => path.field == F::index_type,
        Node::TSTupleType(_) => path.field == F::element_types,
        Node::TSFunctionType(_) | Node::TSConstructorType(_) => path.field == F::return_type,
        Node::TSTypePredicate(_) => path.field == F::type_annotation,
        Node::TSConditionalType(_) => {
            matches!(path.field, F::extends_type | F::true_type | F::false_type)
        }
        Node::TSTypeParameter(_) => matches!(path.field, F::constraint | F::default),
        Node::TSCallSignatureDeclaration(_) | Node::TSMethodSignature(_) => {
            path.field == F::return_type
        }
        Node::TSIndexSignature(_) | Node::TSPropertySignature(_) => {
            path.field == F::type_annotation
        }
        Node::TSTypeParameterInstantiation(_) => path.field == F::params,
        Node::TSAsExpression(_) | Node::TSTypeAssertion(_) => path.field == F::type_annotation,
        _ => false,
    }
}

/// Whether `path` names a TypeScript type position parsed at a tier
/// **narrower** than a full `parse_type_annotation_ts`.
///
/// Task 13 fix round 1. These five already went through
/// [`GenJS::print_child`] for their precedence tiers (see the `TS_*_TYPE`
/// constants); this predicate exists so the intruder rule can apply to them
/// too, since precedence alone cannot express it.
///
/// # Why an intruder rule is needed at all
///
/// `parse_ts_function_or_parenthesized_type`
/// (`crates/parser/src/js/ts/function_types.rs`) routes the contents of `(`
/// through the *type* grammar only when they themselves start with `(`.
/// Everything else goes to `parse_binding_element`, and when no `=>`
/// follows, that binding element is handed back **as the type**. So
/// `type T = ({ a: A })[];` parses to
/// `TSArrayType { element_type: ObjectPattern }` — an expression-space node
/// sitting in a type slot. (Faithful port of the C++; upstream behaves the
/// same.)
///
/// Precedence cannot catch this, and the fourth numbering space is exactly
/// why: `ObjectPattern` is classified `PRIMARY` (32, the *expression*
/// space), every TS tier is 1-6, so `32 >= tier` always holds and
/// `print_child` never wrapped it. Measured before this fix, all silently
/// reparsing to a `TSTypeLiteral` — a different tree, no diagnostic:
///
/// ```text
/// type T = ({ a: A })[];                 -> type T={a:A}[];
/// type T = ({ a: A }) | B;               -> type T={a:A}|B;
/// type T = ({ a: A }) & B;               -> type T={a:A}['k'];
/// type T = ({ a: A }) extends B ? C : D; -> type T={a:A} extends B?C:D;
/// type T = [({a: A})];                   -> type T=[{a:A}];
/// type T = ({ a: A });                   -> type T={a:A};
/// let x: ({ a: A })[];                   -> let x:{a:A}[];
/// ```
///
/// `ArrayPattern` escaped only by accident — it is *unclassified*, so it
/// fell into `ALWAYS_PAREN` and was always wrapped in the five narrowed
/// fields; in a full field (`type T = ([a, b]);`) it broke too. Relying on
/// that accident is exactly what this rule replaces.
pub(crate) fn is_narrowed_ts_type_field(path: Path<'_>) -> bool {
    use NodeField as F;
    match path.parent {
        Node::TSArrayType(_) => path.field == F::element_type,
        Node::TSIndexedAccessType(_) => path.field == F::object_type,
        Node::TSUnionType(_) | Node::TSIntersectionType(_) => path.field == F::types,
        Node::TSConditionalType(_) => path.field == F::check_type,
        _ => false,
    }
}

/// Whether `node` is a `UnaryExpression` with operator `op`.
///
/// juno `gen_js.rs:4006-4015`. Gains a `gc` parameter and a `Result` return
/// — see the module doc comment's deviation #1.
pub(crate) fn is_unary_op(
    gc: &GCLock<'_, '_>,
    node: &Node,
    op: UnaryExpressionOperator,
) -> Result<bool, GenJsError> {
    Ok(match node {
        Node::UnaryExpression(UnaryExpression {
            metadata: _,
            operator,
            argument: _,
            prefix: _,
        }) => UnaryExpressionOperator::from_label(gc, operator.get())? == op,
        _ => false,
    })
}

/// Whether `node` is a prefix `UpdateExpression` with operator `op`.
///
/// juno `gen_js.rs:4017-4027`. Gains a `gc` parameter and a `Result` return
/// — see the module doc comment's deviation #1.
fn is_update_prefix(
    gc: &GCLock<'_, '_>,
    node: &Node,
    op: UpdateExpressionOperator,
) -> Result<bool, GenJsError> {
    Ok(match node {
        Node::UpdateExpression(UpdateExpression {
            metadata: _,
            operator,
            argument: _,
            prefix,
        }) => prefix.get() && UpdateExpressionOperator::from_label(gc, operator.get())? == op,
        _ => false,
    })
}

/// Whether `node` is a computed member access (`MemberExpression`/
/// `OptionalMemberExpression`, `a[b]`/`a?.[b]`) whose object is *directly*
/// the identifier `let`.
///
/// Not a juno function at all — see the `ExpressionStatement` branch of
/// [`GenJS::need_parens`]'s doc comment for why this exists: printing a
/// legal (non-strict, non-module) `let` variable used as the base of a
/// computed member access at the very start of a statement, `let[0] = 1;`,
/// would parse back as the start of a `LexicalDeclaration` instead
/// (`crates/parser/src/js/statements.rs`'s `is_let_followed_by_decl_start`
/// / `lexer/lookahead.rs:215-232`, porting `JSParserImpl.cpp`'s directive
/// lookahead: `let` followed immediately by `[` or `{` always commits to a
/// declaration). Only `[` is checked — `{` can never immediately follow an
/// identifier in valid expression output (no expression-continuation
/// grammar production starts with a bare `{`), so it is not a reachable
/// hazard here; `root_starts_with`'s `ExpressionStatement`-branch caller
/// tests every node on the left-recursive print spine (see
/// [`GenJS::expr_starts_with`]'s doc comment), so testing only the
/// immediate `object` here — rather than unwrapping further — is enough:
/// a chain like `let[0][1]` is caught one recursive step later, at the
/// inner `MemberExpression`, by this same check.
fn starts_with_let_bracket<'gc>(gc: &GCLock<'_, '_>, node: &'gc Node<'gc>) -> bool {
    let object = match node {
        Node::MemberExpression(MemberExpression {
            metadata: _,
            object,
            property: _,
            computed,
        }) if computed.get() => *object,
        Node::OptionalMemberExpression(OptionalMemberExpression {
            metadata: _,
            object,
            property: _,
            computed,
            optional: _,
        }) if computed.get() => *object,
        _ => return false,
    };
    match object {
        Node::Identifier(Identifier {
            metadata: _,
            name,
            type_annotation: _,
            optional: _,
            unresolvable: _,
            decl_state: _,
            decl: _,
        }) => gc.try_bytes_str(name.get()) == Some("let"),
        _ => false,
    }
}

/// Whether `node` is a `NumericLiteral` with a negative value.
///
/// juno `gen_js.rs:4029-4036`.
fn is_negative_number(node: &Node) -> bool {
    match node {
        Node::NumericLiteral(NumericLiteral { metadata: _, value }) => value.get() < 0f64,
        _ => false,
    }
}

/// Whether `node` is a `BinaryExpression` with operator `op`.
///
/// juno `gen_js.rs:4038-4047`. Gains a `gc` parameter and a `Result` return
/// — see the module doc comment's deviation #1.
pub(crate) fn is_binary_op(
    gc: &GCLock<'_, '_>,
    node: &Node,
    op: BinaryExpressionOperator,
) -> Result<bool, GenJsError> {
    Ok(match node {
        Node::BinaryExpression(BinaryExpression {
            metadata: _,
            left: _,
            right: _,
            operator,
        }) => BinaryExpressionOperator::from_label(gc, operator.get())? == op,
        _ => false,
    })
}

/// Whether `node` reads as a leading `+`: unary `+` or prefix `++`.
///
/// juno `gen_js.rs:4060-4063`. Gains a `gc` parameter and a `Result` return
/// — see the module doc comment's deviation #1.
fn check_plus(gc: &GCLock<'_, '_>, node: &Node) -> Result<bool, GenJsError> {
    Ok(is_unary_op(gc, node, UnaryExpressionOperator::Plus)?
        || is_update_prefix(gc, node, UpdateExpressionOperator::Increment)?)
}

/// Whether `node` reads as a leading `-`: unary `-`, prefix `--`, or a
/// negative numeric literal.
///
/// juno `gen_js.rs:4065-4069`. Gains a `gc` parameter and a `Result` return
/// — see the module doc comment's deviation #1.
fn check_minus(gc: &GCLock<'_, '_>, node: &Node) -> Result<bool, GenJsError> {
    Ok(is_unary_op(gc, node, UnaryExpressionOperator::Minus)?
        || is_update_prefix(gc, node, UpdateExpressionOperator::Decrement)?
        || is_negative_number(node))
}

/// Whether `node` is a `LogicalExpression` with `&&` or `||`.
///
/// juno `gen_js.rs:4071-4080`. Gains a `gc` parameter and a `Result` return
/// — see the module doc comment's deviation #1.
fn check_and_or(gc: &GCLock<'_, '_>, node: &Node) -> Result<bool, GenJsError> {
    Ok(match node {
        Node::LogicalExpression(LogicalExpression {
            metadata: _,
            left: _,
            right: _,
            operator,
        }) => matches!(
            LogicalExpressionOperator::from_label(gc, operator.get())?,
            LogicalExpressionOperator::And | LogicalExpressionOperator::Or
        ),
        _ => false,
    })
}

/// Whether `node` is a `LogicalExpression` with `??`.
///
/// juno `gen_js.rs:4082-4091`. Gains a `gc` parameter and a `Result` return
/// — see the module doc comment's deviation #1.
fn check_nullish(gc: &GCLock<'_, '_>, node: &Node) -> Result<bool, GenJsError> {
    Ok(match node {
        Node::LogicalExpression(LogicalExpression {
            metadata: _,
            left: _,
            right: _,
            operator,
        }) => {
            LogicalExpressionOperator::from_label(gc, operator.get())?
                == LogicalExpressionOperator::NullishCoalesce
        }
        _ => false,
    })
}

/// Whether to skip the semicolon at the end of `node`. Block statements
/// don't need semicolons at the end, but other statements which contain
/// statements don't need them either. For example:
/// ```js
/// if (x)
///   y();
/// ```
/// The semicolon will be emitted as part of emitting `y()`, which is an
/// `ExpressionStatement`, so the `IfStatement` does not need to emit a
/// semicolon.
///
/// juno `gen_js.rs:4093-4149`. Drops juno's `ctx: &GCLock` parameter: juno
/// threads it through only because every recursive call needs *a* value to
/// pass, mirroring the rest of `gen_js.rs`'s functions, but never actually
/// reads through it — this function never inspects an operator, so nothing
/// here needs a `GCLock` (module doc comment's deviation #1 does not apply).
pub(crate) fn stmt_skip_semi<'gc>(node: Option<&'gc Node<'gc>>) -> bool {
    match node {
        Some(node) => match node {
            Node::BlockStatement(_)
            | Node::FunctionDeclaration(_)
            | Node::WhileStatement(_)
            | Node::ForStatement(_)
            | Node::ForInStatement(_)
            | Node::ForOfStatement(_)
            | Node::IfStatement(_)
            | Node::WithStatement(_) => true,
            Node::InterfaceDeclaration(_)
            | Node::DeclareInterface(_)
            | Node::DeclareClass(_)
            | Node::DeclareModule(_) => true,
            Node::DeclareExportDeclaration(DeclareExportDeclaration {
                metadata: _,
                declaration,
                specifiers: _,
                source: None,
                default: _,
            }) => stmt_skip_semi(*declaration),
            Node::SwitchStatement(_) => true,
            Node::LabeledStatement(LabeledStatement {
                metadata: _,
                label: _,
                body,
                label_index: _,
            }) => stmt_skip_semi(Some(*body)),
            Node::TryStatement(TryStatement {
                metadata: _,
                block: _,
                handler,
                finalizer,
            }) => stmt_skip_semi(finalizer.or(*handler)),
            Node::CatchClause(CatchClause {
                metadata: _,
                param: _,
                body,
                scope: _,
            }) => stmt_skip_semi(Some(*body)),
            Node::ClassDeclaration(_) => true,
            // Task 12: `MatchStatement`/`RecordDeclaration`/
            // `ComponentDeclaration`/`HookDeclaration`/`DeclareNamespace`
            // all close with an unconditional `}` when reached as a bare
            // statement — see `arms/newer.rs`'s module doc comment's
            // "`stmt_skip_semi`" section for the trace through each one's
            // own parse function. `DeclareComponent`/`DeclareHook`
            // deliberately are NOT added here: neither ends in `}` (see the
            // same section).
            Node::MatchStatement(_)
            | Node::RecordDeclaration(_)
            | Node::ComponentDeclaration(_)
            | Node::HookDeclaration(_)
            | Node::DeclareNamespace(_) => true,
            Node::ExportDefaultDeclaration(ExportDefaultDeclaration {
                metadata: _,
                declaration,
            }) => stmt_skip_semi(Some(*declaration)),
            Node::ExportNamedDeclaration(ExportNamedDeclaration {
                metadata: _,
                declaration,
                specifiers: _,
                source: _,
                export_kind: _,
            }) => stmt_skip_semi(*declaration),
            // Task 12: `DeclareEnum` closes with the same `visit_enum_body`
            // `}` as `EnumDeclaration` (see `arms/newer.rs`'s
            // `gen_declare_enum`, which reuses `EnumDeclaration`'s own body
            // arm's kind — both end in the identical brace).
            Node::EnumDeclaration(_) | Node::DeclareEnum(_) => true,
            // Task 13: the TypeScript declarations that close with their own
            // `}`. `TSInterfaceDeclaration`'s body is a `TSInterfaceBody`
            // and `TSEnumDeclaration`'s member list is brace-delimited
            // (`crates/parser/src/js/ts/declarations.rs`), and
            // `gen_ts_interface_declaration`/`gen_ts_enum_declaration` print
            // that brace unconditionally — including for an empty body,
            // where they print `{}`. `TSTypeAliasDeclaration` is deliberately
            // NOT here: `parse_ts_type_alias_declaration` ends with
            // `eat_semi`, exactly like Flow's `TypeAlias`, whose arm also
            // leaves the `;` to `visit_stmt_in_block`.
            Node::TSInterfaceDeclaration(_) | Node::TSEnumDeclaration(_) => true,
            // A `namespace` closes with the `}` of its `TSModuleBlock` — but
            // only when it HAS one. `initializer` is `Option` in the AST
            // (always `Some` from `parse_ts_namespace_declaration`), so a
            // hand-built member without a body still gets its `;`. Same
            // shape as the `DeclareExportDeclaration` arm above.
            Node::TSModuleMember(TSModuleMember {
                metadata: _,
                id: _,
                initializer: Some(_),
            }) => true,
            Node::TSModuleDeclaration(_) | Node::TSModuleBlock(_) => true,
            _ => false,
        },
        None => false,
    }
}

/// Whether `node`'s subtree contains a `CallExpression` (or a non-optional
/// `OptionalCallExpression`) anywhere — used by `need_parens`'s `new`
/// handling to tell `new (foo()).bar` (a call inside the callee, needs
/// parens) from `new foo().bar` (the call is `new`'s own trailing `()`, no
/// parens needed).
///
/// juno `gen_js.rs:4151-4174`. Two adaptations:
/// - Drops the `gc: &GCLock` parameter juno's version takes. juno needs it
///   only to call `node.visit(ctx, &mut finder, None)`; our `Visitor` trait
///   (`crates/ast/src/visitor.rs`) has no `ctx` in its `visit_node` entry
///   point at all (plan Adaptation Rules), so nothing here ever needs one.
/// - `CallFinder`'s `Visitor` impl overrides `visit_node` directly (plan
///   Adaptation Rules: "the one visitor that *does* map onto our `Visitor`
///   trait") instead of juno's `Visitor::call`, and recurses via
///   `node.visit_children(self)` — our version takes no `gc` either.
pub(crate) fn contains_call<'gc>(node: &'gc Node<'gc>) -> bool {
    struct CallFinder {
        found: bool,
    }
    impl<'gc> Visitor<'gc> for CallFinder {
        fn visit_node(&mut self, node: &'gc Node<'gc>) {
            match node {
                Node::CallExpression(_) => {
                    self.found = true;
                }
                Node::OptionalCallExpression(OptionalCallExpression {
                    metadata: _,
                    callee: _,
                    type_arguments: _,
                    arguments: _,
                    optional,
                }) if !optional.get() => {
                    self.found = true;
                }
                _ => {
                    node.visit_children(self);
                }
            }
        }
    }
    let mut finder = CallFinder { found: false };
    finder.visit_node(node);
    finder.found
}

/// Whether `node`, printed unparenthesized where the parser is reading with
/// `AllowAnonFunctionType::No`, would be misparsed because the `=>` that
/// follows it belongs to an enclosing arrow function rather than to the type.
///
/// # Why this exists
///
/// An arrow function's return type is the **only** annotation position our
/// parser reads with `AllowAnonFunctionType::No`
/// (`crates/parser/src/js/expressions.rs:573` and `:2047`; every other
/// return-type site — `functions.rs:181`, `classes.rs:1168`,
/// `expressions.rs:3907`/`:4071`/`:4346` — passes `Yes`). C++
/// `JSParserImpl-flow.cpp:3224`/`4000-4012` is the same flag.
///
/// So `var x = (): (number=>string) => 1` regenerated without the source's
/// parens as `var x = (): (number) => string => 1` **parses, and to a
/// completely different tree**: return type `number`, body the arrow
/// `string => 1`. Silent, not a syntax error. Found by porting juno's own
/// `test_roundtrip_flow("var x = (): (number=>string) => 1")`
/// (`tests/gen_js/mod.rs:196`), which juno passes only by accident: juno's
/// `get_precedence` has no `TypeAnnotation` arm, so the wrapper falls into
/// its `_ => ALWAYS_PAREN` catch-all and every juno-typed arrow return type
/// gets parens whether it needs them or not. Task 13 gave
/// `TypeAnnotation`/`TSTypeAnnotation` a real `TOP` entry (it had to:
/// blanket parens are a hard reparse failure in TypeScript, see that entry's
/// doc comment) and, in doing so, dropped the parens Flow does need here.
///
/// # The recursion is the parser's descent, not a list
///
/// A first version of this function enumerated the spine edges it followed
/// and asserted the list was complete. It was not: it missed `TypePredicate`
/// and `ConditionalTypeAnnotation`, and the assertion is what made the gap
/// invisible. So this comment states the *rule* the recursion implements and
/// where each edge comes from in the parser, and
/// `tests/roundtrip.rs`'s `flow_arrow_return_type_shapes_all_round_trip`
/// checks the result by execution over a generated cross-product rather than
/// by anybody's reading.
///
/// **Rule.** Recurse into exactly those children the parser parses *without*
/// re-entering `parse_type_annotation_flow`/`parse_return_type_annotation_flow`
/// with an explicit `AllowAnonFunctionType::Yes`, i.e. the children that
/// inherit the ambient flag. Stop — answering `true` — at any node the `No`
/// flag actually changes the parse of.
///
/// Inheriting edges, each read off the descent
/// `parse_return_type_annotation_flow` → `parse_type_annotation_flow` →
/// conditional → union → intersection → anon-fn-without-parens → prefix →
/// postfix → primary:
///
/// - `TypePredicate::type_annotation` — `parse_return_type_annotation_flow`
///   threads its own `allow_anon_function_type` parameter into all three
///   predicate forms' operands (`flow/function_types.rs:79-82` for
///   `asserts x is T`, `:155-159` for `implies x is T`, `:190-193` for the
///   bare `x is T`). This edge is walked from `gen_type_predicate`'s
///   `print_child` call (`arms/newer.rs`) rather than from inside this
///   function — see the last section for why.
/// - `TypeAnnotation::type_annotation` — the `: T` wrapper
///   `parse_type_annotation_flow` adds around the type it just parsed under
///   the caller's flag (`flow/types.rs:65-79`).
/// - `UnionTypeAnnotation::types` / `IntersectionTypeAnnotation::types` —
///   every member is parsed by a plain recursive call at the next tier down,
///   with no flag reset (`flow/types.rs:266-277`, `:293-306`).
/// - `NullableTypeAnnotation::type_annotation` — the prefix `?T` operand,
///   likewise (`flow/types.rs:361-373`).
///
/// Terminals that answer `true`:
///
/// - `FunctionTypeAnnotation` — the node the flag exists to suppress
///   (`flow/types.rs:327`, `flow/function_types.rs:478`).
/// - `ConditionalTypeAnnotation` — **unconditionally**, even with no function
///   type anywhere in it. `parse_conditional_type_annotation_flow` parses its
///   `true_type` and `false_type` with an explicit
///   `AllowAnonFunctionType::Yes` (`flow/types.rs:222-223`, `:237-238`), so
///   an unparenthesized conditional in an arrow return type has its
///   `false_type` swallow the arrow's own `=>`: `(): A extends B ? C : D => 1`
///   does not parse at all. (Its `check_type`/`extends_type` do inherit the
///   flag, but recursing into them would be dead code — the node is already
///   a hazard.)
///
/// Everything else stops at `false`. Postfix (`T[]`, `T[K]`, `T?.[K]`) and
/// primary (`(T)`, `Array<T>`, `[T]`, `{…}`, `keyof T`, `infer T`, `typeof T`)
/// either reset the flag on entry or cannot produce either terminal without
/// an intervening bracket, and the *generator* independently parenthesizes a
/// lower-precedence operand in those positions (`get_precedence`'s
/// `MEMBER`/`ALWAYS_PAREN` entries) — checked, not assumed: the
/// cross-product test above includes `Array<%s>`, `[%s]`, `{ p: %s }` and
/// `%s[]` wrappers around each payload.
///
/// # Deliberate over-approximation
///
/// Some function types on the spine would survive without parens
/// (`(a: number) => string` forces `is_function` via its named parameter,
/// `function_types.rs:418-421`, so the `=>` is consumed by the mandatory
/// `eat_at` at `:468` regardless of the flag). Parenthesizing them anyway is
/// deliberate: a Flow `( Type )` group produces **no wrapper node** — it
/// returns the inner type with only its paren *count* bumped
/// (`function_types.rs:485-490`), and the paren count is not an ESTree
/// property — so an extra pair is invisible to the round-trip oracle, while a
/// missing pair is a corrupted tree. This errs in the safe direction.
///
/// # Why `TypePredicate` stops the walk instead of continuing it
///
/// A `TypePredicate`'s operand *does* inherit the flag
/// (`flow/function_types.rs:79-82`, `:155-159`, `:190-193`), so by the rule
/// above this function "should" descend into it. It must not, because the
/// answer is used to wrap the node it was asked about, and **a predicate
/// cannot be wrapped**: `x is T` is not a type, so `(): (x is T) => 1` does
/// not parse. Reporting `true` for a predicate whose operand is a hazard
/// would turn a wrong tree into a syntax error.
///
/// The parens belong on the operand instead, and that is handled where the
/// operand is printed: `arms/newer.rs`'s `gen_type_predicate` routes
/// `type_annotation` through `print_child` rather than `gen_node`. Both
/// hazard kinds are `ALWAYS_PAREN` in `get_precedence` — `FunctionTypeAnnotation`
/// through the catch-all, `ConditionalTypeAnnotation` by an explicit entry
/// (`:1077`) — so `print_child` alone is enough there and no extra
/// `need_parens` branch is warranted — one was written and then deleted for
/// being dead code. **Checked, not assumed:** restoring the bare `gen_node`
/// fails `flow_type_predicate_operand_keeps_its_parens` and **96** shapes of
/// `flow_arrow_return_type_shapes_all_round_trip`; making this arm descend
/// instead of stop fails the same two tests the other way, on **384** shapes,
/// with `(x is ((number) => string))` no longer parsing at all.
fn flow_no_anon_region_hazard<'gc>(node: &'gc Node<'gc>) -> bool {
    match node {
        // Terminals.
        Node::FunctionTypeAnnotation(_) | Node::ConditionalTypeAnnotation(_) => true,
        // A `TypePredicate` is a *stop*, not an edge, even though its
        // operand does inherit the flag: see "Why `TypePredicate` stops the
        // walk instead of continuing it" above — the parens cannot go
        // around the predicate, so `gen_type_predicate` (`arms/newer.rs`)
        // asks this question about the operand directly, via `print_child`,
        // instead.
        Node::TypePredicate(_) => false,
        // Inheriting edges.
        Node::TypeAnnotation(inner) => flow_no_anon_region_hazard(inner.type_annotation),
        Node::NullableTypeAnnotation(inner) => flow_no_anon_region_hazard(inner.type_annotation),
        Node::UnionTypeAnnotation(inner) => inner.types.iter().any(flow_no_anon_region_hazard),
        Node::IntersectionTypeAnnotation(inner) => {
            inner.types.iter().any(flow_no_anon_region_hazard)
        }
        // Everything else resets the flag or cannot reach a terminal without
        // an intervening bracket.
        _ => false,
    }
}

/// Whether `node`'s subtree contains a `BinaryExpression` with operator
/// `in` anywhere — used by `need_parens`'s `ForStatement`/
/// `VariableDeclarator` branches (above) to decide whether the *whole* of
/// a `for(...)` head's init clause (a bare expression or a
/// `VariableDeclarator`'s init) needs a protecting `(...)` around it.
///
/// Not a juno function — those two branches' own doc comments have the
/// full account of why this exists. In short: ECMA-262 14.7.4's `[~In]`
/// grammar parameter, which excludes a bare `in` from a `for` head's init
/// clause, propagates through *both* operands of every binary/logical/
/// conditional/assignment production nested inside it, not just a
/// left-recursive spine — so `precedence.rs`'s own `expr_starts_with`/
/// `root_starts_with` (built for a different hazard, the "does printing
/// this statement's expression start with a dangerous token" question,
/// where only a left-recursive spine is ever printed first) is the wrong
/// shape here: `a && b in c`'s `b in c` is `&&`'s *right* operand, not
/// reachable by walking only left children.
///
/// Mirrors [`contains_call`]'s shape and conservatism deliberately: an
/// unconditional full-subtree walk, with no attempt to recognize a nested
/// `[+In]`-reset context (a parenthesized sub-expression, a function/
/// arrow body, an array/object literal element, a call's arguments — each
/// of which restarts a fresh, unrestricted `Expression[+In]` per the
/// grammar, and so can never actually need this protection). Over-
/// parenthesizing one of those is a redundant-but-harmless `(...)` around
/// something already syntactically safe; recognizing every reset context
/// precisely would mean re-implementing a second copy of the `[~In]`/
/// `[+In]` propagation rules, well beyond what a targeted fix needs —
/// spec §7 makes round-trip correctness, not minimal output, this crate's
/// bar.
///
/// Takes `ctx: &GCLock` (unlike [`contains_call`], which needs none) to
/// read `BinaryExpression::operator`'s spelling; a plain
/// `gc.bytes_str_lossy(operator.get()) == "in"` byte compare is enough —
/// this is a parenthesization heuristic, not a place a malformed
/// `operator` spelling needs to surface as a hard [`GenJsError`], so there
/// is no need to route this through the fallible `BinaryExpressionOperator::from_label`
/// (which would also force `InFinder::visit_node`'s `Result`-less
/// `Visitor` signature to change).
fn contains_bare_in<'gc>(ctx: &GCLock<'_, '_>, node: &'gc Node<'gc>) -> bool {
    struct InFinder<'a, 'b, 'c> {
        gc: &'a GCLock<'b, 'c>,
        found: bool,
    }
    impl<'gc, 'a, 'b, 'c> Visitor<'gc> for InFinder<'a, 'b, 'c> {
        fn visit_node(&mut self, node: &'gc Node<'gc>) {
            if self.found {
                return;
            }
            if let Node::BinaryExpression(BinaryExpression {
                metadata: _,
                left: _,
                right: _,
                operator,
            }) = node
            {
                if self.gc.bytes_str_lossy(operator.get()) == "in" {
                    self.found = true;
                    return;
                }
            }
            node.visit_children(self);
        }
    }
    let mut finder = InFinder {
        gc: ctx,
        found: false,
    };
    finder.visit_node(node);
    finder.found
}

#[cfg(test)]
mod tests {
    use super::*;
    use hermes_ast::node::Program;
    use hermes_parser::{parse, ParseFlags};

    use crate::Opt;

    // -----------------------------------------------------------------
    // Step 4: ordering relationships, not the arbitrary numeric values.
    // -----------------------------------------------------------------

    #[test]
    // These constants are already known at compile time, so clippy would
    // rather this were `const { assert!(...) }` — a build-time check. Kept
    // as ordinary runtime assertions in a named `#[test]` on purpose: the
    // plan's Task 3 Step 5 "prove it can fail" step mutates a constant and
    // expects `cargo test` to report a *named test* failing, not a build
    // failure (see task-3-report.md).
    #[allow(clippy::assertions_on_constants)]
    fn expression_precedence_levels_are_ordered_seq_arrow_yield_assign() {
        assert!(SEQ < ARROW, "SEQ ({SEQ}) should be below ARROW ({ARROW})");
        assert!(
            ARROW < YIELD,
            "ARROW ({ARROW}) should be below YIELD ({YIELD})"
        );
        assert!(
            YIELD < ASSIGN,
            "YIELD ({YIELD}) should be below ASSIGN ({ASSIGN})"
        );
    }

    /// `RECORD_EXPRESSION` must sit strictly between `UNARY` and
    /// `TAGGED_TEMPLATE` (Task 12 review round 4). Below
    /// `TAGGED_TEMPLATE`/`NEW_NO_ARGS`/`MEMBER` because a
    /// `RecordExpression` cannot carry *any* postfix tail
    /// (`R {p:1}.foo`/`R {p:1}()`/`R {p:1}[0]`/``R {p:1}`t` `` all fail to
    /// parse, and `new R {p:1}` too), so every one of those parents must
    /// wrap it; above `UNARY` because `typeof R {p:1}`, `-R {p:1}`,
    /// `!R {p:1}` and `void R {p:1}` all parse bare, so wrapping there
    /// would be redundant output.
    ///
    /// Ordering, not values: inserting this level shifted the four
    /// constants above it, and nothing may depend on the literals.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn record_expression_binds_below_every_postfix_operator_and_above_unary() {
        assert!(
            UNARY < RECORD_EXPRESSION,
            "UNARY ({UNARY}) should be below RECORD_EXPRESSION ({RECORD_EXPRESSION}): \
             `typeof R {{p:1}}` parses bare"
        );
        for (name, prec) in [
            ("TAGGED_TEMPLATE", TAGGED_TEMPLATE),
            ("NEW_NO_ARGS", NEW_NO_ARGS),
            ("MEMBER", MEMBER),
            ("PRIMARY", PRIMARY),
        ] {
            assert!(
                RECORD_EXPRESSION < prec,
                "RECORD_EXPRESSION ({RECORD_EXPRESSION}) must bind looser than {name} \
                 ({prec}), or a record expression under that operator loses its parens"
            );
        }
    }

    /// The three Flow `match`-pattern tiers must stay strictly ordered
    /// loosest-to-tightest, matching `parse_match_pattern_flow`'s own three
    /// layers (Task 12 review rounds 3-4). They deliberately share small
    /// integers with `UNION_TYPE`/`INTERSECTION_TYPE` — three disjoint
    /// numbering spaces in one `Precedence` type, juno's own design — which
    /// is sound only because a match pattern's `path.parent` in
    /// [`GenJS::need_parens`] is always another match-pattern kind.
    ///
    /// That cross-space invariant is **not** enforced at runtime, on
    /// purpose: the natural spelling would be a `debug_assert!` in
    /// `need_parens` asserting "parent is a match kind iff child is", but a
    /// hand-built or deserialized tree can legitimately put any node in
    /// `MatchOrPattern::patterns`, and this crate's spec §4 rule is that a
    /// malformed input tree yields a [`crate::GenJsError`], never a panic —
    /// including in a downstream consumer's debug build. So the invariant
    /// is pinned here instead, where the realistic future breakage lives:
    /// a new match-pattern kind copy-pasted into the `PRIMARY` bucket, or
    /// the three tiers renumbered out of order.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn match_pattern_tiers_are_ordered_as_pattern_or_pattern_subpattern() {
        assert!(
            MATCH_AS_PATTERN < MATCH_OR_PATTERN,
            "MATCH_AS_PATTERN ({MATCH_AS_PATTERN}) must bind looser than MATCH_OR_PATTERN \
             ({MATCH_OR_PATTERN}): `a | b as x` parses with no parens"
        );
        assert!(
            MATCH_OR_PATTERN < MATCH_SUBPATTERN,
            "MATCH_OR_PATTERN ({MATCH_OR_PATTERN}) must bind looser than MATCH_SUBPATTERN \
             ({MATCH_SUBPATTERN})"
        );
    }

    /// Every match-pattern kind that can reach [`GenJS::print_child`] as a
    /// *child* must classify into the match numbering space — none may fall
    /// into the `ALWAYS_PAREN` catch-all (which would wrap it
    /// unconditionally) or into an expression tier like `PRIMARY` (which
    /// would compare against the wrong space entirely). Companion to the
    /// ordering test above; this is the one that catches a newly added
    /// match-pattern kind that nobody classified.
    ///
    /// Three `Match*Pattern*` kinds are deliberately excluded, because they
    /// are structural sub-nodes rather than patterns in their own right and
    /// are each printed from one fixed field with a plain `gen_node`, never
    /// through `print_child` (`arms/newer.rs`): `MatchInstanceObjectPattern`
    /// (the `{ … }` of `MatchInstancePattern::properties`),
    /// `MatchObjectPatternProperty` (an element of an object pattern's
    /// property list), and `MatchRestPattern` (an array/object pattern's
    /// `rest`). Leaving them unclassified cannot manifest, and classifying
    /// them would be a claim no round-trip test could back — this test
    /// found them and this comment records the finding rather than papering
    /// over it.
    #[test]
    fn every_match_pattern_kind_classifies_into_the_match_numbering_space() {
        let src = "const y = match (x) { \
                   1 => 1, a => 2, _ => 3, const q => 4, a.b => 5, \
                   Foo{k: 1} => 6, [1] => 7, {k: 1} => 8, -1 => 9, \
                   (a | b) => 10, (a as z) => 11 };";
        let mut parsed = parse(
            src,
            ParseFlags {
                parse_flow_match: true,
                ..Default::default()
            },
        )
        .expect("test source must parse");
        parsed.with_program(|gc, node| {
            let mut sink = Vec::new();
            let gen_js = GenJS::for_test(&mut sink, Opt::new());
            let mut seen = 0;
            // The structural sub-nodes excluded by the doc comment above,
            // plus the four non-pattern `Match*` kinds.
            const NOT_A_PRINT_CHILD_PATTERN: &[&str] = &[
                "MatchInstanceObjectPattern",
                "MatchObjectPatternProperty",
                "MatchRestPattern",
                "MatchExpression",
                "MatchExpressionCase",
                "MatchStatement",
                "MatchStatementCase",
            ];
            let mut walk = |n: &Node<'_>| {
                let name = format!("{:?}", n.kind());
                let is_pattern =
                    name.starts_with("Match") && !NOT_A_PRINT_CHILD_PATTERN.contains(&name.as_str());
                if is_pattern {
                    seen += 1;
                    let (prec, _) = gen_js
                        .get_precedence(gc, n)
                        .expect("a match pattern has no operator to classify");
                    assert!(
                        (MATCH_AS_PATTERN..=MATCH_SUBPATTERN).contains(&prec),
                        "{:?} classifies at {prec}, outside the match numbering space \
                         [{MATCH_AS_PATTERN}, {MATCH_SUBPATTERN}] — a match pattern must \
                         never land in the ALWAYS_PAREN catch-all or an expression tier",
                        n.kind()
                    );
                }
            };
            visit_all(node, &mut walk);
            assert!(
                seen >= 11,
                "expected to reach every match-pattern kind in the fixture, saw {seen}"
            );
        });
    }

    /// Depth-first walk over `node` and all its descendants, applying `f`.
    /// `hermes_ast`'s `Visitor` needs a struct; this closure form keeps the
    /// test above readable.
    fn visit_all<'gc>(node: &'gc Node<'gc>, f: &mut impl FnMut(&Node<'gc>)) {
        struct W<'a, 'gc, F: FnMut(&Node<'gc>)> {
            f: &'a mut F,
            _m: std::marker::PhantomData<&'gc ()>,
        }
        impl<'gc, F: FnMut(&Node<'gc>)> Visitor<'gc> for W<'_, 'gc, F> {
            fn visit_node(&mut self, node: &'gc Node<'gc>) {
                (self.f)(node);
                node.visit_children(self);
            }
        }
        let mut w = W {
            f,
            _m: std::marker::PhantomData,
        };
        w.visit_node(node);
    }

    /// The TypeScript type tiers must be ordered exactly as
    /// `crates/parser/src/js/ts/types.rs`'s recursive descent nests them:
    /// conditional/predicate loosest, primary tightest. Asserts the ordering,
    /// never the literal values (which are arbitrary — see [`Precedence`]).
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn ts_type_tiers_are_ordered_conditional_function_union_intersection_postfix_primary() {
        assert!(
            TS_CONDITIONAL_TYPE < TS_FUNCTION_TYPE,
            "a conditional type binds looser than a function type"
        );
        assert!(
            TS_FUNCTION_TYPE < TS_UNION_TYPE,
            "a function type binds looser than a union: its return type is a \
             full `parse_type_annotation_ts` that swallows `| …`"
        );
        assert!(
            TS_UNION_TYPE < TS_INTERSECTION_TYPE,
            "`parse_ts_union_type` is built out of `parse_ts_intersection_type`"
        );
        assert!(
            TS_INTERSECTION_TYPE < TS_POSTFIX_TYPE,
            "`parse_ts_intersection_type` is built out of `parse_ts_postfix_type`"
        );
        assert!(
            TS_POSTFIX_TYPE < TS_PRIMARY_TYPE,
            "`parse_ts_postfix_type` applies only to a primary base"
        );
        // The TS space must stay BELOW the expression space's `PRIMARY`:
        // `ExistsTypeAnnotation` (`*`) is shared with the TS grammar and is
        // classified there, and `(*)` does not parse.
        assert!(
            TS_PRIMARY_TYPE < PRIMARY,
            "the TS numbering space must not overlap the expression space's \
             PRIMARY, or a shared `*` type would be wrapped in parens"
        );
    }

    /// Every kind [`GenJS::get_precedence`] classifies into the TS numbering
    /// space must also be in [`is_ts_type_node`]'s allow-list, and vice
    /// versa for the two TS-grammar kinds that live outside that space.
    ///
    /// Task 13 fix round 1. These are two independent lists describing the
    /// same set — "is this a TypeScript type?" — and they are consulted by
    /// the same `need_parens` call, so a kind added to one and not the other
    /// silently mis-parenthesizes: present in the tier table but missing
    /// from the allow-list means the intruder rule wraps a legitimate type
    /// (and `(*)`-style output that cannot reparse); present in the
    /// allow-list but missing from the tier table means it falls to
    /// `ALWAYS_PAREN` and gets wrapped for the opposite reason. This walks a
    /// fixture holding one of every TS type kind our parser can build and
    /// checks the two agree.
    #[test]
    fn the_ts_tier_table_and_the_ts_type_allow_list_agree() {
        // One of every TS type kind reachable from our parser, in one
        // program. `interface`-as-a-type needs strict mode.
        let src = "'use strict';\n\
                   type A1 = any | number | boolean | string | symbol | void;\n\
                   type A2 = undefined | unknown | never | bigint | this | *;\n\
                   type A3 = 'l' | 42 | 123n | true | null;\n\
                   type A4 = R<X> | R.S | R['k'] | R[] | [R, R] | typeof v;\n\
                   type A5 = { m(): R } & R;\n\
                   type A6 = (a: R) => a is R;\n\
                   type A7 = new (a: R) => R;\n\
                   type A8 = R extends R ? R : R;\n\
                   type A9 = interface I { a: R };\n";
        let mut parsed = parse(
            src,
            ParseFlags {
                parse_ts: true,
                ..Default::default()
            },
        )
        .expect("test source must parse");
        parsed.with_program(|gc, node| {
            let mut sink = Vec::new();
            let gen_js = GenJS::for_test(&mut sink, Opt::new());
            let mut seen = 0;
            let mut walk = |n: &Node<'_>| {
                let (prec, _) = gen_js
                    .get_precedence(gc, n)
                    .expect("no operator to classify in a type");
                let in_ts_space = (TS_CONDITIONAL_TYPE..=TS_PRIMARY_TYPE).contains(&prec);
                let allowed = is_ts_type_node(n);
                if in_ts_space {
                    seen += 1;
                    assert!(
                        allowed,
                        "{:?} is classified in the TS numbering space (tier {prec}) but is \
                         missing from `is_ts_type_node`'s allow-list, so the intruder rule \
                         would wrap it",
                        n.kind()
                    );
                }
                // The converse, for the two allow-list members that sit
                // outside the TS space on purpose: `ExistsTypeAnnotation`
                // (`PRIMARY`, shared with Flow) and `TSTypeAnnotation`
                // (`TOP`, a transparent wrapper). Anything else in the
                // allow-list must be in the space.
                if allowed
                    && !matches!(
                        n,
                        Node::ExistsTypeAnnotation(_) | Node::TSTypeAnnotation(_)
                    )
                {
                    assert!(
                        in_ts_space,
                        "{:?} is in `is_ts_type_node`'s allow-list but classifies at {prec}, \
                         outside the TS numbering space [{TS_CONDITIONAL_TYPE}, \
                         {TS_PRIMARY_TYPE}] — it would be wrapped by the ALWAYS_PAREN rule",
                        n.kind()
                    );
                }
            };
            visit_all(node, &mut walk);
            // 10 keyword kinds + this + literal + reference + qualified name
            // + indexed access + array + tuple + query + type literal + union
            // + intersection + function + constructor + predicate +
            // conditional + interface = 26 distinct kinds, many repeated.
            assert!(
                seen >= 26,
                "expected the fixture to reach every TS type kind, saw {seen} classified nodes"
            );
        });
    }

    /// An as-expression must bind looser than **every** binary and logical
    /// operator: its right operand is a type, and the type grammar reads on
    /// past `|`, `&` and `<`. This is the invariant behind
    /// `tests/roundtrip.rs`'s
    /// `as_expression_operand_of_bitwise_and_relational_operators_keeps_parens`.
    ///
    /// Review round 1 finding M-4: the first version of this test compared
    /// the named constants only (`AS_EXPRESSION < get_binary_precedence(op)`)
    /// and never called [`GenJS::get_precedence`] — so reverting the
    /// `AsExpression | AsConstExpression | TSAsExpression` arm to
    /// `get_binary_precedence(In)`, the exact defect it claims to guard, left
    /// it **passing**. It now asks `get_precedence` about real parsed nodes
    /// of all three kinds, which is the thing that can actually regress.
    #[test]
    fn get_precedence_puts_as_expressions_below_every_binary_and_logical_operator() {
        // All three kinds that share the arm: Flow `as`, Flow `as const`,
        // and the TypeScript spelling.
        for (src, ts) in [("x as A; x as const;", false), ("x as A;", true)] {
            let mut parsed = parse(
                src,
                if ts {
                    ParseFlags {
                        parse_ts: true,
                        ..Default::default()
                    }
                } else {
                    ParseFlags {
                        parse_flow: true,
                        ..Default::default()
                    }
                },
            )
            .expect("test source must parse");
            parsed.with_program(|gc, node| {
                let mut sink = Vec::new();
                let gen_js = GenJS::for_test(&mut sink, Opt::new());
                let mut seen = 0;
                let mut walk = |n: &Node<'_>| {
                    if !matches!(
                        n,
                        Node::AsExpression(_)
                            | Node::AsConstExpression(_)
                            | Node::TSAsExpression(_)
                    ) {
                        return;
                    }
                    seen += 1;
                    let (prec, assoc) = gen_js
                        .get_precedence(gc, n)
                        .expect("an as-expression has no operator to classify");
                    assert_eq!(assoc, Assoc::Ltr, "{:?}: `x as A as B` is left-associative", n.kind());
                    use BinaryExpressionOperator::*;
                    for op in [
                        LooseEquals,
                        LooseNotEquals,
                        StrictEquals,
                        StrictNotEquals,
                        Less,
                        LessEquals,
                        Greater,
                        GreaterEquals,
                        LShift,
                        RShift,
                        RShift3,
                        Plus,
                        Minus,
                        Mult,
                        Div,
                        Mod,
                        BitOr,
                        BitXor,
                        BitAnd,
                        In,
                        Instanceof,
                        Exp,
                    ] {
                        let p = get_binary_precedence(op);
                        assert!(
                            prec < p,
                            "{:?} classifies at {prec}, which does NOT bind looser than \
                             {op:?} ({p}) — its right operand is a type, and the type \
                             grammar reads past `|`/`&`/`<`",
                            n.kind()
                        );
                    }
                    for op in [
                        LogicalExpressionOperator::And,
                        LogicalExpressionOperator::Or,
                        LogicalExpressionOperator::NullishCoalesce,
                    ] {
                        let p = get_logical_precedence(op);
                        assert!(
                            prec < p,
                            "{:?} classifies at {prec}, not looser than {op:?} ({p})",
                            n.kind()
                        );
                    }
                    // ...but tighter than `?:` and everything below it, so
                    // `x as A ? b : c` and `x = y as A` stay bare.
                    assert!(
                        prec > COND,
                        "{:?} classifies at {prec}, which is not tighter than COND \
                         ({COND}) — `x as A ? b : c` would gain redundant parens",
                        n.kind()
                    );
                };
                visit_all(node, &mut walk);
                assert!(
                    seen >= 1,
                    "expected to reach an as-expression in {src:?}, saw {seen}"
                );
            });
        }
    }

    #[test]
    fn binary_precedence_orders_plus_below_mult() {
        let plus = get_binary_precedence(BinaryExpressionOperator::Plus);
        let mult = get_binary_precedence(BinaryExpressionOperator::Mult);
        assert!(
            plus < mult,
            "`+` ({plus}) should bind looser than `*` ({mult})"
        );
    }

    #[test]
    fn binary_precedence_orders_exp_above_mult() {
        // `**` binds tighter than `*`: `2 * 3 ** 2` is `2 * (3 ** 2)`.
        let exp = get_binary_precedence(BinaryExpressionOperator::Exp);
        let mult = get_binary_precedence(BinaryExpressionOperator::Mult);
        assert!(
            exp > mult,
            "`**` ({exp}) should bind tighter than `*` ({mult})"
        );
    }

    /// Every `BinaryExpressionOperator`/`LogicalExpressionOperator` variant
    /// maps to a real precedence. The actual "new operator is a compile
    /// error" guarantee (plan Task 3 Step 4) lives in `get_binary_precedence`
    /// and `get_logical_precedence` themselves, above: both are `match`
    /// expressions with no wildcard arm, so the compiler already refuses to
    /// build this crate if either enum grows a variant without a
    /// corresponding case. This test exercises every variant that exists
    /// today and checks the map is total and non-degenerate (never
    /// `ALWAYS_PAREN`) for all of them.
    #[test]
    fn every_binary_and_logical_operator_has_a_real_precedence() {
        use BinaryExpressionOperator::*;
        for op in [
            LooseEquals,
            LooseNotEquals,
            StrictEquals,
            StrictNotEquals,
            Less,
            LessEquals,
            Greater,
            GreaterEquals,
            LShift,
            RShift,
            RShift3,
            Plus,
            Minus,
            Mult,
            Div,
            Mod,
            BitOr,
            BitXor,
            BitAnd,
            Exp,
            In,
            Instanceof,
        ] {
            let p = get_binary_precedence(op);
            assert!(p > ALWAYS_PAREN, "{op:?} has degenerate precedence {p}");
            assert!(p >= BIN_START, "{op:?} has precedence {p} below BIN_START");
        }

        use LogicalExpressionOperator::*;
        for op in [And, Or, NullishCoalesce] {
            let p = get_logical_precedence(op);
            assert!(p > ALWAYS_PAREN, "{op:?} has degenerate precedence {p}");
            assert!(p >= BIN_START, "{op:?} has precedence {p} below BIN_START");
        }
    }

    // -----------------------------------------------------------------
    // `**` right-associativity: the fix in `get_precedence`'s
    // `BinaryExpression` arm (module doc comment). These are the tests that
    // fail without it — see task-3-report.md for the demonstrated failure.
    // -----------------------------------------------------------------

    /// Parse `src` (expected to be a single expression statement) and hand
    /// its top-level expression, plus the locked `GCLock`, to `f`.
    fn with_expr<R>(
        src: &str,
        f: impl for<'gc> FnOnce(&'gc GCLock<'static, '_>, &'gc Node<'gc>) -> R,
    ) -> R {
        let mut parsed = parse(src, ParseFlags::default()).expect("test source must parse");
        parsed.with_program(|gc, node| {
            let Node::Program(Program {
                metadata: _,
                body,
                scope: _,
                sem_info: _,
                strictness: _,
                is_method_definition: _,
                decorations: _,
                dummy_param_list: _,
            }) = node
            else {
                panic!("root is not a Program");
            };
            let stmt = body.iter().next().expect("source has a statement");
            let Node::ExpressionStatement(es) = stmt else {
                panic!("statement is not an ExpressionStatement: {stmt:?}");
            };
            f(gc, es.expression)
        })
    }

    #[test]
    fn get_precedence_treats_exp_as_right_associative() {
        with_expr("a ** b;", |gc, expr| {
            assert!(matches!(expr, Node::BinaryExpression(_)));
            let mut sink = Vec::new();
            let gen_js = GenJS::for_test(&mut sink, Opt::new());
            let (_, assoc) = gen_js
                .get_precedence(gc, expr)
                .expect("BinaryExpression's operator spelling is well-formed");
            assert_eq!(
                assoc,
                Assoc::Rtl,
                "`**` is right-associative in ECMA-262 (ExponentiationExpression); \
                 `get_precedence` must not report Assoc::Ltr for it"
            );
        });
    }

    // -----------------------------------------------------------------
    // `PrivateName` precedence: the fix in `get_precedence`'s `match`
    // (module doc comment's second correctness-fix section). These are the
    // tests that fail without it — `this.#x` regenerates as the
    // syntactically-invalid `this.(#x)`; see task-7-report.md for the
    // demonstrated failure via `tests/roundtrip.rs`'s
    // `class_private_field_and_private_method_round_trip`.
    // -----------------------------------------------------------------

    #[test]
    fn get_precedence_treats_private_name_as_primary_not_always_paren() {
        with_expr("this.#x;", |gc, expr| {
            let Node::MemberExpression(MemberExpression { property, .. }) = expr else {
                panic!("statement is not a MemberExpression: {expr:?}");
            };
            assert!(matches!(property, Node::PrivateName(_)));
            let mut sink = Vec::new();
            let gen_js = GenJS::for_test(&mut sink, Opt::new());
            let (prec, assoc) = gen_js
                .get_precedence(gc, property)
                .expect("PrivateName has no operator to classify, cannot fail");
            assert_eq!(
                prec, PRIMARY,
                "PrivateName must not fall into the ALWAYS_PAREN catch-all"
            );
            assert_eq!(assoc, Assoc::Ltr);
        });
    }

    #[test]
    fn need_parens_omits_parens_around_member_expression_private_name_property() {
        with_expr("this.#x;", |gc, expr| {
            let Node::MemberExpression(MemberExpression { property, .. }) = expr else {
                panic!("statement is not a MemberExpression: {expr:?}");
            };
            let mut sink = Vec::new();
            let gen_js = GenJS::for_test(&mut sink, Opt::new());
            let need = gen_js
                .need_parens(
                    gc,
                    Path::new(expr, NodeField::property),
                    property,
                    ChildPos::Right,
                )
                .expect("well-formed tree classifies without error");
            assert_eq!(
                need,
                NeedParens::No,
                "this.#x must not gain parens around #x -- `.(` is a syntax error"
            );
        });
    }

    #[test]
    fn need_parens_requires_parens_around_exp_left_child_but_not_right_child() {
        // `(a ** b) ** c`: the left child is itself `**`. Printing it
        // without parens (`a ** b ** c`) would reparse, under `**`'s real
        // right-associative grammar, as `a ** (b ** c)` -- a different
        // value whenever a, b, c differ. This is the concrete failure the
        // module doc comment traces through `need_parens`'s equal-precedence
        // branch.
        with_expr("(a ** b) ** c;", |gc, outer| {
            let Node::BinaryExpression(BinaryExpression {
                metadata: _,
                left,
                right: _,
                operator: _,
            }) = outer
            else {
                panic!("expected a BinaryExpression: {outer:?}");
            };
            assert!(matches!(left, Node::BinaryExpression(_)));
            let mut sink = Vec::new();
            let gen_js = GenJS::for_test(&mut sink, Opt::new());
            let need = gen_js
                .need_parens(gc, Path::new(outer, NodeField::left), left, ChildPos::Left)
                .expect("well-formed tree classifies without error");
            assert_eq!(
                need,
                NeedParens::Yes,
                "(a ** b) ** c must keep parens around its left `**` child"
            );
        });

        // `a ** (b ** c)`: the right child is itself `**` too, but real
        // right-associativity means `a ** b ** c` already means this, so no
        // parens are needed on the right.
        with_expr("a ** (b ** c);", |gc, outer| {
            let Node::BinaryExpression(BinaryExpression {
                metadata: _,
                left: _,
                right,
                operator: _,
            }) = outer
            else {
                panic!("expected a BinaryExpression: {outer:?}");
            };
            assert!(matches!(right, Node::BinaryExpression(_)));
            let mut sink = Vec::new();
            let gen_js = GenJS::for_test(&mut sink, Opt::new());
            let need = gen_js
                .need_parens(
                    gc,
                    Path::new(outer, NodeField::right),
                    right,
                    ChildPos::Right,
                )
                .expect("well-formed tree classifies without error");
            assert_eq!(
                need,
                NeedParens::No,
                "a ** (b ** c) does not need parens around its right `**` child"
            );
        });
    }

    // -----------------------------------------------------------------
    // Unrecognized operator spellings: task-3 review finding. `generate()`'s
    // `Node` parameter is not required to have come from this crate's
    // parser, so nothing at the type level stops a hand-built tree from
    // holding an `operator` spelling none of the four classifiers know.
    // Spec §4 requires that be a returned `GenJsError`, never a panic.
    // -----------------------------------------------------------------

    #[test]
    fn unknown_binary_operator_spelling_is_a_gen_js_error_not_a_panic() {
        use hermes_ast::node_child::NodeMetadata;

        // Start from a real parse so `left`/`right`/metadata (source range,
        // debug loc) are all valid; only the operator atom is hand-built,
        // simulating exactly the finding's scenario -- a caller assembling a
        // `Node` graph that never went through the parser's fixed operator
        // set.
        with_expr("a + b;", |gc, expr| {
            let Node::BinaryExpression(BinaryExpression {
                metadata,
                left,
                right,
                operator: _,
            }) = expr
            else {
                panic!("expected a BinaryExpression: {expr:?}");
            };
            let bogus_op = gc.atom_bytes("not_a_real_operator");
            let hand_built = gc.alloc(Node::BinaryExpression(BinaryExpression::new(
                NodeMetadata::new(metadata.range.get()),
                left,
                right,
                bogus_op,
            )));

            let mut sink = Vec::new();
            let gen_js = GenJS::for_test(&mut sink, Opt::new());
            let err = gen_js
                .get_precedence(gc, hand_built)
                .expect_err("an unrecognized operator spelling must be a GenJsError, not a panic");
            match err {
                GenJsError::UnknownOperator { kind, spelling } => {
                    assert_eq!(kind, "BinaryExpression");
                    assert_eq!(spelling, "not_a_real_operator");
                }
                other => panic!("expected GenJsError::UnknownOperator, got {other:?}"),
            }
        });
    }
}
