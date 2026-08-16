/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The 53 ES/Flow kinds juno's generator predates (plan Task 12, spec §4's
//! "53 ES/Flow" list). **There is no juno source for any arm in this file**
//! — juno's `gen_js.rs` (frozen) was written before Flow `match`, `record`,
//! `component`/`hook`, and several of these type-annotation kinds existed at
//! all. Every arm here is derived directly from our own parser's production
//! for that kind — `crates/parser/src/js/{classes,expressions}.rs` (Step 1),
//! `crates/parser/src/js/flow/match_.rs` (Step 2), `crates/parser/src/js/
//! flow/{mod,declarations}.rs` (Step 3), `crates/parser/src/js/flow/
//! {declarations,types}.rs` (Step 4), `crates/parser/src/js/flow/{types,
//! object_types,declarations}.rs` (Step 5) — not from any intuition about
//! what the syntax "should" look like. Citations below point at the specific
//! parse function read for each arm.
//!
//! # Shared conventions with the rest of the crate
//!
//! Every arm follows the same rules the ported tasks established:
//! exhaustive destructuring (no `..`), `gc.try_bytes_str` for identifiers,
//! `print_child`/`get_precedence` only where the child position is genuinely
//! operator-precedence-sensitive (see each arm's own note), and a private
//! `from_label`-classifier enum (mirroring `precedence.rs`'s
//! `BinaryExpressionOperator` and `arms/func.rs`'s `MethodDefinitionKind`)
//! for every `Cell<NodeLabel>`/`Cell<NodeString>` field with a small fixed
//! spelling set — `GenJsError::UnknownOperator` on an unrecognized spelling,
//! never a panic (spec §4).
//!
//! # Two visibility bumps in `arms/flow_decl.rs`, not new code
//!
//! This task reuses two items Task 11 already built the exact shape for,
//! rather than duplicating them:
//! - [`crate::arms::flow_decl::declare_prefix_needed`] (was private to that
//!   module) — `DeclareComponent`/`DeclareHook`/`DeclareEnum` are reachable
//!   as a `DeclareExportDeclaration`'s `declaration` exactly like Task 11's
//!   four `Declare*` kinds (confirmed against `declarations.rs`'s
//!   `parse_declare_export_flow`, which routes `hook`/`component`/`enum`
//!   through the same `declare`-flag machinery as `function`/`class`), so
//!   they need the identical "print `declare ` only when *not* already
//!   inside a `declare export`" logic.
//! - `GenJS::gen_enum_member_with_init` (was private) — `EnumBigIntMember`
//!   is byte-for-byte the same shape as `EnumStringMember`/`EnumNumberMember`/
//!   `EnumBooleanMember` (`Id = init`), which `arms/flow_decl.rs`'s own
//!   module doc comment already flags as deliberately left for this task.
//!
//! Both are visibility-only changes (`fn` → `pub(crate) fn`); neither
//! function's body changes.
//!
//! # A real bug avoided: `InferTypeAnnotation` must NOT reuse
//! `GenJS::gen_type_parameter`
//!
//! `infer A extends B`'s `A extends B` is built as a real `TypeParameter`
//! node (`crates/parser/src/js/flow/types.rs`'s `infer` arm inside
//! `parse_primary_type_annotation_flow`, ~line 756), but the *existing*
//! `TypeParameter` printer (`arms/flow_decl.rs`'s `gen_type_parameter`,
//! Task 11) unconditionally prints any `bound` as `name: Bound` — the
//! established, deliberate juno-matching choice for an *ordinary*
//! `<T: Bound>`/`<T extends Bound>` type-parameter-list entry, where both
//! spellings reparse to the same tree. `infer`'s hand-built `TypeParameter`
//! is not reached through that shared list parser at all: its own arm reads
//! `if self.check(TokenKind::rw_extends) { ... }` and has **no `:`-bound
//! path whatsoever** — so reusing `gen_type_parameter` here would print
//! `infer A: B`, which does not reparse as an `InferTypeAnnotation` with a
//! bound at all (the `infer` arm never looks for a `:`). This is exactly
//! the "prints a child via a generic helper that changes shape based on a
//! field it doesn't consult" hazard the task brief's parenthesization note
//! generalizes to non-precedence bugs too. [`GenJS::gen_infer_type_annotation`]
//! destructures the `TypeParameter` inline instead and prints `extends`
//! literally, never delegating to `gen_type_parameter`.
//!
//! # A design choice, not a bug: `Decorator` always parenthesizes
//!
//! A `Decorator`'s `expression` field is built one of two ways
//! (`crates/parser/src/js/classes.rs`'s `parse_decorator`): either
//! `@( Expression )` (any expression at all, parens **not preserved** as a
//! wrapper node — `parse_decorator` just returns the inner expression
//! directly) or `@`*DecoratorMemberExpression* (an identifier, `.name`/
//! `.#name` chain, and an optional trailing call — a syntactically much
//! narrower shape). Since the AST cannot distinguish which branch produced a
//! given `expression` after the fact, and only the second (narrow) shape is
//! legal directly after a bare `@`, [`GenJS::gen_decorator`] always prints
//! `@(expression)`: `@(` `)` accepts *any* expression (the first branch's own
//! production), so this is unconditionally round-trip-correct, at the cost
//! of a redundant paren pair on the common `@identifier`/`@a.b.c()` case —
//! acceptable per spec §7 (round-trip correctness, not minimal output, is
//! this crate's bar).
//!
//! # Parenthesization: what needed `print_child`, and what didn't
//!
//! **`AsExpression`/`AsConstExpression`'s `expression` needs `print_child`.**
//! `x as T` is built by the *same* precedence-climbing binary-operator loop
//! as `+`/`in`/`instanceof` (`crates/parser/src/js/expressions.rs`'s
//! `parse_binary_expression`, `as_operator` precedence 8 — identical to
//! `in`/`instanceof`, confirmed at `Self::get_precedence`'s `as_operator`
//! arm), left-associative. So `expression` is a real operator operand that
//! can be any lower- *or* higher-precedence expression depending on source
//! shape (`(a, b) as T`, `a || b as T`, ...) and must be parenthesized by
//! the same `need_parens` machinery every other binary operator's operand
//! uses — plain `gen_node` would silently drop required parens.
//! `precedence.rs`'s `get_precedence` gains an arm classifying both kinds at
//! `get_binary_precedence(BinaryExpressionOperator::In)` (same tier as
//! `in`/`instanceof`), `Assoc::Ltr`. `type_annotation`, by contrast, is
//! plain `gen_node`: it is parsed via `parse_type_annotation` — the *full*
//! top-level type grammar, self-delimited (nothing can follow a type in this
//! position except what already ends the statement/expression), so no
//! wrapping is ever needed there.
//!
//! **`match`'s `argument` needs no `print_child`, but its patterns do — one
//! of them badly.** `argument` is always printed inside explicit `(`/`)`
//! this crate writes itself (mirroring `crates/parser/src/js/flow/match_.rs`'s
//! own `reparseArgumentsAsMatchArgumentFlow`, which already collapses a
//! multi-argument list into one `SequenceExpression` at *parse* time — by
//! the time generation sees it, `argument` is a single node whose own
//! internal commas, if any, are exactly what the source `match(a, b)` had),
//! so a plain `gen_node` inside those parens can never leak.
//!
//! **`MatchOrPattern`'s elements need `print_child` — an earlier draft of
//! this comment claimed the parser makes it "structurally impossible" for
//! an element to itself be a `MatchOrPattern`/`MatchAsPattern`, which is
//! wrong and was caught in review.** `parseMatchSubpatternFlow`'s `l_paren`
//! arm (`match_.rs`, ~line 591) calls the *full* `parseMatchPatternFlow`,
//! not itself, and — the same "grouping parens don't survive as a wrapper
//! node" shape as every parenthesized-group production in this parser
//! (Flow types, JS expressions, ...) — unwraps with nothing recorded, so
//! `(a as x) | b` and `(a | b) | c` both reach `MatchOrPattern`'s element
//! list carrying a `MatchAsPattern`/nested `MatchOrPattern` directly. Fixed
//! (review round 2) with `print_child` plus a dedicated `MATCH_SUBPATTERN`
//! precedence space in `precedence.rs` (match patterns are entirely
//! disjoint from expressions and Flow types, so this is a third separate
//! numbering, alongside `UNION_TYPE`/`INTERSECTION_TYPE` — see that
//! constant's own doc comment); regression test
//! `match_or_pattern_parenthesizes_as_pattern_and_nested_or_pattern_elements`
//! (`tests/roundtrip.rs`) is the one that failed before this fix — without
//! it, `(a as x) | b` regenerated as the unparseable `a as x | b`, and
//! `(a | b) | c` silently flattened to a different (flat, 3-element) tree.
//!
//! **`MatchAsPattern`'s own `pattern` needs `print_child` too — and round
//! 2's re-audit got this one wrong in the other direction, asserting the
//! field "is parsed at full pattern tier … plain `gen_node` there is
//! correct, not merely untested".** It is not. `parse_match_pattern_flow`'s
//! `l_paren` group arm recurses into the *full* `parse_match_pattern_flow`,
//! so the `first_pattern` that becomes `MatchAsPattern.pattern` can itself
//! be another `MatchAsPattern`, reachable only through an explicit
//! `( MatchPattern )` group. Reproduced live before the fix:
//! `(a as y) as z` regenerated as `a as y as z`, which fails to reparse
//! (`'=>' expected after match pattern`) — the `as` branch runs once and
//! its target is a binding identifier/pattern, never another pattern.
//!
//! The naive fix — routing `pattern` through `print_child` under round 2's
//! classification, where both `MatchAsPattern` and `MatchOrPattern` sat in
//! the `ALWAYS_PAREN` catch-all — over-wraps, because this field needs a
//! *different* answer than `MatchOrPattern`'s element list does for the
//! same two child kinds: `parse_match_pattern_flow` runs its `|`-loop
//! *before* its `as` check within one call, so `a | b as z` already parses
//! to `MatchAsPattern(MatchOrPattern[a, b], z)` with no parens, and
//! wrapping it would emit a redundant `(a | b) as z` (confirmed: doing so
//! fails this file's own regression test). The fix therefore replaces the
//! catch-all with the real three-tier ordering `MATCH_AS_PATTERN` <
//! `MATCH_OR_PATTERN` < `MATCH_SUBPATTERN` (`precedence.rs`), which is
//! exactly `parse_match_pattern_flow`'s own three layers; `MatchAsPattern`
//! carries `Assoc::Rtl` so that an equal-precedence *left* child is the one
//! that gets wrapped. Regression test:
//! `match_as_pattern_parenthesizes_nested_as_pattern_but_not_or_pattern`,
//! which asserts both halves (structural AST equality for `(a as y) as z`,
//! exact generated text for `a | b as z`) and was confirmed to fail against
//! *each* of the two wrong answers separately.
//!
//! `MatchAsPattern`'s `target` is separately safe as a plain `gen_node`:
//! `match_.rs:461-475` restricts it to `parse_match_binding_pattern_flow`/
//! `parse_match_binding_identifier_flow`, so it is only ever an
//! `Identifier` or a `MatchBindingPattern` — never an As/Or pattern, and
//! nothing with a precedence question.
//!
//! `MatchExpressionCase`'s `body` (an assignment-level expression sitting in
//! a bare, unparenthesized comma-separated case list — `pattern => body,
//! pattern2 => body2`) shares the ordinary `,`-list `SequenceExpression`
//! hazard every other comma list in this crate has (`ArrayExpression`,
//! `CallExpression` arguments, ...), so it goes through the existing
//! [`GenJS::print_comma_expression`] helper, not a bare `gen_node`, even
//! though `body` can never *structurally* be a bare `SequenceExpression`
//! from this crate's own parser (assignment expressions never include one)
//! — this guards a hand-built tree too, at zero cost for a well-formed one.
//! Every other match-pattern position is a plain `gen_node`, and each was
//! **verified by running a parenthesized source through generate→reparse
//! and comparing the two ASTs**, not merely by reading the parser (round 2
//! did the latter and still missed `MatchAsPattern.pattern`):
//! `MatchArrayPattern`'s elements, `MatchObjectPatternProperty`'s
//! non-shorthand `pattern`, and both case kinds' `pattern` are parsed by
//! the *full* `parse_match_pattern_flow`, so an As/Or pattern there needs
//! no parens — `[(a as y) as z, b | c, (d | e) as f]`,
//! `{k: (a as y) as z, m: b | c}`, and `Foo{k: (a | b) as c}` all
//! regenerate to a structurally identical tree. `MatchMemberPattern`'s
//! `base`/`property`, `MatchInstancePattern`'s `target_constructor`, and
//! `MatchBindingPattern`'s `id` come only from the subpattern-only
//! identifier/member chain (`parse_match_identifier_subpattern_flow`),
//! which cannot produce an As/Or pattern; `MatchUnaryPattern`'s and
//! `MatchRestPattern`'s `argument` are restricted to a fixed node-kind set
//! (numeric/bigint literal; `MatchBindingPattern` only) with no precedence
//! question at all. A `match` construct's `argument` and a case's `guard`
//! are printed inside explicit literal `(`/`)` this crate writes itself, so
//! their tier cannot matter (`match (a, b) { … }` and
//! `_ if ((c, d)) => 1` both verified).
//!
//! **Four more restricted-tier child positions, also found only by the
//! reviewer reproducing them against the real crates, not by re-reading the
//! parser closely enough the first time:** `ConditionalTypeAnnotation`'s
//! `check_type`/`extends_type`, `InferTypeAnnotation`'s `bound`,
//! `KeyofTypeAnnotation`'s `argument`, and `TypeOperator`'s
//! `type_annotation` are each parsed at a grammar tier *narrower* than the
//! full type grammar (union tier for the first two, prefix tier for the
//! last two — see each arm's own doc comment and `precedence.rs`'s
//! corresponding `get_precedence` entries for the exact parser evidence),
//! so a value that reached that field via explicit source parentheses lost
//! them on a bare `gen_node`. Now `print_child`, with regression tests
//! `conditional_type_annotation_parenthesizes_restricted_check_and_extends_type`,
//! `infer_type_annotation_parenthesizes_restricted_conditional_bound`,
//! `keyof_type_annotation_parenthesizes_restricted_union_argument`, and
//! `type_operator_parenthesizes_restricted_union_in_component_type_renders`
//! (`tests/roundtrip.rs`) — each confirmed to fail without its fix. The
//! `InferTypeAnnotation` case was the worst of the five: dropping the
//! parens did not just change the tree, it made the regenerated source
//! **fail to reparse** (see that arm's own doc comment for why).
//!
//! **`InferTypeAnnotation`'s own precedence is shape-dependent, not a flat
//! `ALWAYS_PAREN` (review round 3).** Round 2 grouped the kind into
//! `ConditionalTypeAnnotation`'s `ALWAYS_PAREN` arm, which wrapped *every*
//! `infer` reached through *any* `print_child` — so the canonical unnested
//! idiom `A extends infer B ? B : never` regenerated as
//! `A extends (infer B) ? B : never`. That is output quality, not
//! corruption, but it contradicted the same fix's own comment. `infer A`
//! with no bound is a plain primary production (keyword plus one
//! identifier, self-delimited) and is now `PRIMARY`; `infer A extends B`
//! extends rightwards over a whole union-tier bound and binds looser than
//! `UnionTypeAnnotation` itself, so it keeps `ALWAYS_PAREN`. The
//! `check_type`-with-bound sub-case the reviewer flagged as an
//! under-parenthesization risk cannot regress, because the with-bound half
//! keeps the strictly-more-conservative classification it already had; only
//! the no-bound half loosens. `precedence.rs`'s entry carries the full
//! derivation, including why the one token that could extend a bare
//! `infer A` (`extends`, emitted only after a conditional's `check_type`)
//! is neutralized by the parser's own speculative-bound backtrack. Tests:
//! `infer_type_annotation_without_bound_needs_no_parens` and
//! `infer_type_annotation_with_bound_stays_parenthesized`, each confirmed
//! to fail against the opposite mistake.
//!
//! **Every other child position among this file's 53 kinds was re-audited
//! (review round 3) by *running* a parenthesized lowest-precedence type
//! through generate→reparse and comparing ASTs, not only by reading the
//! parser:** `RecordDeclarationProperty`/`RecordDeclarationStaticProperty`'s
//! `type_annotation`/`default_value`/`value`, `RecordDeclaration`/
//! `RecordExpression`'s `type_parameters`/`type_arguments`,
//! `ComponentDeclaration`/`DeclareComponent`/`ComponentTypeAnnotation`'s
//! `params`/`rest`/`renders_type`, `ComponentTypeParameter`'s
//! `type_annotation`, `HookDeclaration`/`HookTypeAnnotation`/
//! `DeclareHook`'s `params`/`return_type`, `ConditionalTypeAnnotation`'s
//! `true_type`/`false_type`, `TypePredicate`'s `type_annotation`,
//! `TupleTypeLabeledElement`/`TupleTypeSpreadElement`'s
//! `element_type`/`type_annotation`, `ObjectTypeMappedTypeProperty`'s
//! `source_type`/`prop_type`, and `QualifiedTypeofIdentifier`'s
//! `qualification`/`id`. Each was fed both a parenthesized union
//! (`(X | Y)`) and a parenthesized conditional
//! (`(A extends B ? C : D)`) — the loosest kind in the type grammar, so it
//! is the sharpest probe a full-tier field can be given — and every one
//! regenerated to a structurally identical AST under both pretty modes.
//! `precedence.rs`'s `get_precedence` gains two more
//! low-risk, obviously-correct entries beyond the restricted-tier
//! fixes above: `NeverTypeAnnotation`/`UndefinedTypeAnnotation`/
//! `UnknownTypeAnnotation` join the existing primitive-keyword-type
//! `PRIMARY` bucket (the exact same self-delimited shape as its
//! `any`/`mixed`/`empty` siblings — Task 10's own comment on that bucket
//! already anticipated this), and `MatchExpression` is added at `PRIMARY`
//! too. `RecordExpression` was originally added alongside it, on the
//! reasoning that both are "self-delimited by a trailing `}`, the same tier
//! as `ObjectExpression`" — **that analogy was wrong twice over, and review
//! round 4 corrected both halves; see the next section.** Every other
//! newly introduced kind is left
//! unclassified (falls into `get_precedence`'s `ALWAYS_PAREN` catch-all when
//! reached through some *other* arm's `print_child`, e.g. as a
//! `UnionTypeAnnotation` member) — safe by construction, since a redundant
//! `(Type)` always reparses to the identical value (spec §7 does not
//! require minimal output).
//!
//! # Review round 4: `MatchExpression`/`RecordExpression` are not
//! "the same tier as `ObjectExpression`"
//!
//! Two Critical round-trip breaks, both pre-existing since this task's
//! first commit and missed by rounds 2 and 3, traced to the one sentence
//! above. The analogy fails in a different way for each kind, so each got a
//! different fix — the shared `PRIMARY` classification was wrong for one
//! and insufficient for the other.
//!
//! **For `ObjectExpression` the precedence tier was never the whole story.**
//! `({a: 1});` keeps its parens not because of `get_precedence` but because
//! `ObjectExpression` is listed in the *statement-start* guard inside
//! `need_parens`'s `ExpressionStatement` branch — a positional hazard the
//! precedence table cannot express. `MatchExpression` shares that hazard
//! exactly and was not listed: a statement beginning with `match` + `(` is
//! taken by `try_parse_match_statement_flow` as a match *statement*, so
//! `(match (x) { _ => 1 });` regenerated as `match(x){_=>1};` and
//! `(match (x) { _ => 1 }).foo;` as `match(x){_=>1}.foo;` — which do not
//! merely reparse as a different node kind, they **panic the parser**
//! (`assertion failed: self.check(TokenKind::l_brace)`,
//! `crates/parser/src/js/statements.rs:1196`, reached from `parse_block` on
//! the non-block case body). Fixed by adding `Node::MatchExpression(_)` to
//! that guard, which is the right mechanism here: wrapping the whole
//! statement expression rescues every tail shape (verified for `.foo`,
//! `()`, `[0]`, `+ 1`, `, 2` and `? a : b`). Regression test
//! `match_expression_at_statement_start_keeps_its_parens`. Everywhere off
//! statement start a match expression parses bare, including with a postfix
//! tail, so it gains no parens there — asserted by the same test.
//!
//! **`RecordExpression` is not a primary at all.**
//! `parseLeftHandSideExpressionTail` (`lib/Parser/JSParserImpl.cpp:4026-4089`,
//! ported at `crates/parser/src/js/expressions.rs:2752`) builds the record
//! expression in a trailing `else if` and returns immediately, never
//! looping back into the member-select tail — so `R {p: 1}.foo`,
//! `R {p: 1}()`, `R {p: 1}[0]`, ``R {p: 1}`t` `` and `new R {p: 1}` fail to
//! parse *anywhere*, not just at statement start, while the parser does
//! build `MemberExpression{object: RecordExpression}` and friends from the
//! parenthesized source. Classified `PRIMARY` — above `MEMBER` — the
//! generator printed those bare: `(R {p: 1}).foo;` regenerated as the
//! unparseable `R {p: 1}.foo;`. Fixed with a new `RECORD_EXPRESSION`
//! precedence level below `TAGGED_TEMPLATE`/`NEW_NO_ARGS`/`MEMBER` and
//! above `UNARY`, the same "must be wrapped before any postfix operator"
//! shape `NEW_NO_ARGS` already has. Regression test
//! `record_expression_under_postfix_operators_keeps_its_parens`.
//!
//! **`RecordExpression` deliberately did NOT go into the statement-start
//! guard**, even though the round-4 review listed
//! `record R { p: number } (R { p: 1 });` as a third instance of the same
//! defect. Measured: a bare `R {p: 1};` at statement start parses to the
//! identical tree, so that case is a cosmetic paren drop, not a corruption
//! — and the guard would not have fixed the real one anyway, because it
//! wraps the *whole* statement expression and `(R {p: 1}.foo);` does not
//! parse either. The parens have to land on the record expression itself.
//! `record_expression_at_statement_start_needs_no_parens` pins that
//! conclusion so it is not silently "fixed" later.
//!
//! # `stmt_skip_semi`: six kinds close with `}` and need no caller-added `;`
//!
//! `MatchStatement`, `RecordDeclaration`, `ComponentDeclaration`,
//! `HookDeclaration`, `DeclareEnum` (merged into the existing
//! `EnumDeclaration` arm), and `DeclareNamespace` all end in an
//! unconditional closing `}` when reachable as a bare statement
//! (`Program`/`BlockStatement`'s `body`, via `visit_stmt_in_block`) —
//! confirmed by tracing each one's own parse function above. `DeclareComponent`/
//! `DeclareHook` deliberately are **not** added: both end in `)` or a
//! `renders`/return-type annotation, never a brace, so `visit_stmt_in_block`
//! must keep adding their `;` (mirroring `DeclareFunction`/`DeclareVariable`,
//! already in this same "no brace, needs the caller's `;`" bucket).

use hermes_ast::context::GCLock;
use hermes_ast::node::{
    AsConstExpression, AsExpression, ComponentDeclaration, ComponentParameter,
    ComponentTypeAnnotation, ComponentTypeParameter, ConditionalTypeAnnotation, DeclareComponent,
    DeclareEnum, DeclareHook, DeclareNamespace, Decorator, EnumBigIntBody, EnumBigIntMember,
    HookDeclaration, HookTypeAnnotation, Identifier, InferTypeAnnotation, KeyofTypeAnnotation,
    MatchArrayPattern, MatchAsPattern, MatchBindingPattern, MatchExpression, MatchExpressionCase,
    MatchIdentifierPattern, MatchInstanceObjectPattern, MatchInstancePattern, MatchLiteralPattern,
    MatchMemberPattern, MatchObjectPattern, MatchObjectPatternProperty, MatchOrPattern,
    MatchRestPattern, MatchStatement, MatchStatementCase, MatchUnaryPattern, Node, NodeField,
    ObjectTypeMappedTypeProperty, QualifiedTypeofIdentifier, RecordDeclaration,
    RecordDeclarationBody, RecordDeclarationImplements, RecordDeclarationProperty,
    RecordDeclarationStaticProperty, RecordExpression, RecordExpressionProperties, StaticBlock,
    TupleTypeLabeledElement, TupleTypeSpreadElement, TypeAnnotation, TypeOperator, TypeParameter,
    TypePredicate,
};
use hermes_ast::node_child::{NodeList, NodeLabel};
use hermes_ast::visitor::Path;

use crate::arms::flow_decl::declare_prefix_needed;
use crate::precedence::{ChildPos, ForceSpace};
use crate::{out, GenJS, GenJsError, Pretty};

// ---------------------------------------------------------------------------
// Step 1: ES-level kinds. `StaticBlock` (class { static { ... } }, plain
// ES2022 — juno simply predates it, per spec §4), `Decorator`,
// `AsExpression`, `AsConstExpression`.
// ---------------------------------------------------------------------------

impl<'s, 'w> GenJS<'s, 'w> {
    /// `StaticBlock`: `static { ...body }` — a class member.
    ///
    /// `crates/parser/src/js/classes.rs`, the `ClassStaticBlock` arm inside
    /// the class-member parse loop (~line 850): `static` immediately
    /// followed by `{`, then an ordinary statement list, then `}`. Mirrors
    /// `GenJS::gen_block_statement`'s own empty/non-empty shape (Task 6)
    /// with a `static` keyword prefix.
    pub(crate) fn gen_static_block<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &StaticBlock<'gc>,
    ) -> Result<(), GenJsError> {
        let StaticBlock { metadata: _, body, scope: _, function_info: _ } = inner;
        out!(self, "static");
        self.space(ForceSpace::No);
        if body.is_empty() {
            out!(self, "{{}}");
        } else {
            out!(self, "{{");
            self.inc_indent();
            self.newline();
            self.visit_stmt_list(ctx, *body, Path::new(node, NodeField::body))?;
            self.dec_indent();
            self.newline();
            out!(self, "}}");
        }
        Ok(())
    }

    /// `Decorator`: `@(expression)`. See the module doc comment's "A design
    /// choice, not a bug" section for why this always parenthesizes rather
    /// than trying to reproduce which of the two source shapes
    /// (`crates/parser/src/js/classes.rs`'s `parse_decorator`, ~line 79)
    /// `expression` came from.
    pub(crate) fn gen_decorator<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &Decorator<'gc>,
    ) -> Result<(), GenJsError> {
        let Decorator { metadata: _, expression } = inner;
        out!(self, "@(");
        self.gen_node(ctx, expression, Some(Path::new(node, NodeField::expression)))?;
        out!(self, ")");
        Ok(())
    }

    /// `AsExpression`: `expression as type_annotation`. See the module doc
    /// comment's "Parenthesization" section for why `expression` goes
    /// through `print_child` (a real binary-operator operand, precedence 8 —
    /// same tier as `in`/`instanceof`) while `type_annotation` is plain
    /// `gen_node` (the full, self-delimited type grammar).
    ///
    /// `crates/parser/src/js/expressions.rs`'s `make_as_node` (~line 2172),
    /// the general (non-`as const`) case.
    pub(crate) fn gen_as_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &AsExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let AsExpression { metadata: _, expression, type_annotation } = inner;
        self.print_child(
            ctx,
            Some(*expression),
            Path::new(node, NodeField::expression),
            ChildPos::Left,
        )?;
        self.space(ForceSpace::Yes);
        out!(self, "as");
        self.space(ForceSpace::Yes);
        // `x as const` is not an `AsExpression` at all — the parser folds it
        // into `AsConstExpression` — so an `AsExpression` whose annotation
        // would print as bare `const` must keep the parens the source had.
        // See [`is_as_const_shape`].
        let parens = is_as_const_shape(ctx, type_annotation);
        if parens {
            out!(self, "(");
        }
        self.gen_node(
            ctx,
            type_annotation,
            Some(Path::new(node, NodeField::type_annotation)),
        )?;
        if parens {
            out!(self, ")");
        }
        Ok(())
    }

    /// `AsConstExpression`: `expression as const`. Same `print_child`
    /// reasoning as [`GenJS::gen_as_expression`] — `make_as_node`
    /// (~line 2172-2211) special-cases `x as const` into this kind, but
    /// `expression` is still the identical left operand of the same
    /// precedence-8 `as_operator`.
    pub(crate) fn gen_as_const_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &AsConstExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let AsConstExpression { metadata: _, expression } = inner;
        self.print_child(
            ctx,
            Some(*expression),
            Path::new(node, NodeField::expression),
            ChildPos::Left,
        )?;
        self.space(ForceSpace::Yes);
        out!(self, "as const");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Step 2: the Flow `match` family (18 kinds), gated on `parse_flow_match`.
// All derived from `crates/parser/src/js/flow/match_.rs`.
// ---------------------------------------------------------------------------

/// `MatchBindingPattern::kind`, classified from its raw spelling
/// (`match_.rs`'s `parse_match_binding_pattern_flow`, ~line 781: `const`/
/// `var`/the contextual `let`).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum MatchBindingKind {
    Const,
    Var,
    Let,
}

impl MatchBindingKind {
    fn from_label(gc: &GCLock<'_, '_>, label: NodeLabel) -> Result<Self, GenJsError> {
        Ok(match gc.bytes_str_lossy(label) {
            "const" => Self::Const,
            "var" => Self::Var,
            "let" => Self::Let,
            other => {
                return Err(GenJsError::UnknownOperator {
                    kind: "MatchBindingPattern",
                    spelling: other.to_string(),
                })
            }
        })
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Const => "const",
            Self::Var => "var",
            Self::Let => "let",
        }
    }
}

/// `MatchUnaryPattern::operator`, classified from its raw spelling
/// (`match_.rs`'s `parse_match_subpattern_flow`, ~line 545: `+`/`-`).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum MatchUnaryOperator {
    Plus,
    Minus,
}

impl MatchUnaryOperator {
    fn from_label(gc: &GCLock<'_, '_>, label: NodeLabel) -> Result<Self, GenJsError> {
        Ok(match gc.bytes_str_lossy(label) {
            "+" => Self::Plus,
            "-" => Self::Minus,
            other => {
                return Err(GenJsError::UnknownOperator {
                    kind: "MatchUnaryPattern",
                    spelling: other.to_string(),
                })
            }
        })
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Plus => "+",
            Self::Minus => "-",
        }
    }
}

impl<'s, 'w> GenJS<'s, 'w> {
    /// `MatchExpression`: `match(argument) { pattern => body, ... }`.
    ///
    /// `match_.rs`'s `parse_match_call_or_match_expression_flow` (~line 239,
    /// the `{`-follows-arguments branch) + `parse_match_expression_flow`
    /// (~line 301). `argument` is printed via plain `gen_node` inside the
    /// `(`/`)` this method writes itself — see the module doc comment's
    /// "Parenthesization" section for why that is safe even when `argument`
    /// is itself a multi-element `SequenceExpression` (from a source
    /// `match(a, b)`). Cases are comma-separated (`,` is required between
    /// expression cases, trailing comma allowed — confirmed via
    /// `check_and_eat`'s `break`-on-absence at `parse_match_expression_flow`
    /// ~line 356).
    pub(crate) fn gen_match_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchExpression { metadata: _, argument, cases } = inner;
        out!(self, "match");
        self.space(ForceSpace::No);
        out!(self, "(");
        self.gen_node(ctx, argument, Some(Path::new(node, NodeField::argument)))?;
        out!(self, ")");
        self.space(ForceSpace::No);
        out!(self, "{{");
        self.inc_indent();
        self.newline();
        for (i, case) in cases.iter().enumerate() {
            if i > 0 {
                self.comma();
                self.newline();
            }
            self.gen_node(ctx, case, Some(Path::new(node, NodeField::cases)))?;
        }
        self.dec_indent();
        self.newline();
        out!(self, "}}");
        Ok(())
    }

    /// `MatchStatement`: `match(argument) { pattern => { ... } ... }`.
    ///
    /// `match_.rs`'s `try_parse_match_statement_flow` (~line 115). Unlike
    /// the expression form, the comma between statement cases is entirely
    /// *optional* (`~line 964`: eaten if present, never required, never
    /// checked for absence) — each case's block body (`{ ... }`) already
    /// self-delimits, so no separator is needed at all; this prints a bare
    /// newline between cases rather than a comma, which the grammar accepts
    /// identically to a comma-separated form.
    pub(crate) fn gen_match_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchStatement { metadata: _, argument, cases } = inner;
        out!(self, "match");
        self.space(ForceSpace::No);
        out!(self, "(");
        self.gen_node(ctx, argument, Some(Path::new(node, NodeField::argument)))?;
        out!(self, ")");
        self.space(ForceSpace::No);
        out!(self, "{{");
        self.inc_indent();
        self.newline();
        for (i, case) in cases.iter().enumerate() {
            if i > 0 {
                self.newline();
            }
            self.gen_node(ctx, case, Some(Path::new(node, NodeField::cases)))?;
        }
        self.dec_indent();
        self.newline();
        out!(self, "}}");
        Ok(())
    }

    /// Shared `pattern [if (guard)]` prefix for both case kinds.
    fn gen_match_case_head<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        pattern: &'gc Node<'gc>,
        guard: Option<&'gc Node<'gc>>,
    ) -> Result<(), GenJsError> {
        self.gen_node(ctx, pattern, Some(Path::new(node, NodeField::pattern)))?;
        if let Some(guard) = guard {
            self.space(ForceSpace::Yes);
            out!(self, "if");
            self.space(ForceSpace::No);
            out!(self, "(");
            self.gen_node(ctx, guard, Some(Path::new(node, NodeField::guard)))?;
            out!(self, ")");
        }
        self.space(ForceSpace::No);
        self.space_before_equals("=>");
        out!(self, "=>");
        self.space(ForceSpace::No);
        Ok(())
    }

    /// `MatchExpressionCase`: `pattern [if (guard)] => body`. `body` goes
    /// through [`GenJS::print_comma_expression`], not a bare `gen_node` —
    /// see the module doc comment's "Parenthesization" section: this case
    /// sits in a bare comma-separated list, the same hazard every other
    /// comma list in this crate guards against.
    ///
    /// `match_.rs`'s `parse_match_expression_flow` (~line 301), the
    /// per-case loop body (~line 311-359).
    pub(crate) fn gen_match_expression_case<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchExpressionCase<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchExpressionCase { metadata: _, pattern, body, guard } = inner;
        self.gen_match_case_head(ctx, node, pattern, *guard)?;
        self.print_comma_expression(ctx, body, Path::new(node, NodeField::body))
    }

    /// `MatchStatementCase`: `pattern [if (guard)] => body` — `body` is
    /// always a `BlockStatement` (self-delimited by its own `{`/`}`), so a
    /// plain `gen_node` is safe here, unlike the expression-case body.
    ///
    /// `match_.rs`'s `try_parse_match_statement_flow` (~line 115), the
    /// per-case loop body (~line 164-205).
    pub(crate) fn gen_match_statement_case<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchStatementCase<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchStatementCase { metadata: _, pattern, body, guard } = inner;
        self.gen_match_case_head(ctx, node, pattern, *guard)?;
        self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))
    }

    /// `MatchArrayPattern`: `[elem0, elem1, ...[rest]]`. `rest` (a
    /// `MatchRestPattern`) already prints its own leading `...` — see
    /// [`GenJS::gen_match_rest_pattern`] — so no extra `...` is added here.
    ///
    /// `match_.rs`'s `parse_match_array_pattern_flow` (~line 1044).
    pub(crate) fn gen_match_array_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchArrayPattern<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchArrayPattern { metadata: _, elements, rest } = inner;
        out!(self, "[");
        for (i, elem) in elements.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.gen_node(ctx, elem, Some(Path::new(node, NodeField::elements)))?;
        }
        if let Some(rest) = rest {
            if !elements.is_empty() {
                self.comma();
            }
            self.gen_node(ctx, rest, Some(Path::new(node, NodeField::rest)))?;
        }
        out!(self, "]");
        Ok(())
    }

    /// `MatchAsPattern`: `pattern as target`. `as` is a contextual
    /// identifier, not a token, so `ForceSpace::Yes` on both sides is
    /// required (`patternasx` would otherwise misparse as one identifier if
    /// `target` starts with a letter) — matches how `in`/`instanceof` are
    /// spaced elsewhere in this crate.
    ///
    /// `pattern` goes through `print_child` (review round 3): it is *not*
    /// restricted to `parseMatchSubpatternFlow`, but neither is it safe
    /// bare. `parse_match_pattern_flow` runs its `|`-loop before the `as`
    /// check within one call, so a `MatchOrPattern` here needs no parens
    /// (`a | b as x` already parses to `MatchAsPattern(MatchOrPattern, x)`),
    /// but a nested `MatchAsPattern` — reachable only through an explicit
    /// `( MatchPattern )` group, whose `l_paren` arm recurses into the
    /// *full* `parse_match_pattern_flow` and records no wrapper node — does:
    /// `(a as y) as z` printed bare is `a as y as z`, which fails to reparse
    /// (`'=>' expected after match pattern`), since the `as` branch runs
    /// once and its target is a binding identifier/pattern, never another
    /// pattern. `precedence.rs`'s `MATCH_AS_PATTERN`/`MATCH_OR_PATTERN`
    /// entries encode exactly that split; regression test
    /// `match_as_pattern_parenthesizes_nested_as_pattern_but_not_or_pattern`.
    ///
    /// `target` stays plain `gen_node`: it is restricted to
    /// `parse_match_binding_pattern_flow`/`parse_match_binding_identifier_flow`
    /// (`match_.rs:461-475`), so it can only ever be an `Identifier` or a
    /// `MatchBindingPattern` — never an As/Or pattern, and never anything
    /// with a precedence question.
    ///
    /// `match_.rs`'s `parse_match_pattern_flow` (~line 428), the trailing
    /// `as` handling (~line 451-482).
    pub(crate) fn gen_match_as_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchAsPattern<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchAsPattern { metadata: _, pattern, target } = inner;
        self.print_child(
            ctx,
            Some(*pattern),
            Path::new(node, NodeField::pattern),
            ChildPos::Left,
        )?;
        self.space(ForceSpace::Yes);
        out!(self, "as");
        self.space(ForceSpace::Yes);
        self.gen_node(ctx, target, Some(Path::new(node, NodeField::target)))
    }

    /// `MatchBindingPattern`: `const|var|let id`.
    ///
    /// `match_.rs`'s `parse_match_binding_pattern_flow` (~line 781).
    pub(crate) fn gen_match_binding_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchBindingPattern<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchBindingPattern { metadata: _, id, kind } = inner;
        let kw = MatchBindingKind::from_label(ctx, kind.get())?.as_str();
        out!(self, "{}", kw);
        self.space(ForceSpace::Yes);
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))
    }

    /// `MatchIdentifierPattern`: a bare `id`.
    ///
    /// `match_.rs`'s `parse_match_identifier_subpattern_flow` (~line 626).
    pub(crate) fn gen_match_identifier_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchIdentifierPattern<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchIdentifierPattern { metadata: _, id } = inner;
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))
    }

    /// Shared `{ properties, ...[rest] }` body for `MatchObjectPattern` and
    /// `MatchInstanceObjectPattern` — identical shape
    /// (`match_.rs`'s `parse_match_object_pattern_properties_flow`,
    /// ~line 860, shared by both parse functions).
    fn gen_match_object_pattern_body<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        properties: NodeList<'gc>,
        rest: Option<&'gc Node<'gc>>,
    ) -> Result<(), GenJsError> {
        out!(self, "{{");
        for (i, prop) in properties.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.gen_node(ctx, prop, Some(Path::new(node, NodeField::properties)))?;
        }
        if let Some(rest) = rest {
            if !properties.is_empty() {
                self.comma();
            }
            self.gen_node(ctx, rest, Some(Path::new(node, NodeField::rest)))?;
        }
        out!(self, "}}");
        Ok(())
    }

    /// `MatchInstanceObjectPattern`: the `{ ... }` fields of an instance
    /// pattern (`Ctor { ... }`) — see [`GenJS::gen_match_instance_pattern`]
    /// for the enclosing `targetConstructor` prefix.
    ///
    /// `match_.rs`'s `parse_match_instance_object_pattern_flow` (~line 1018).
    pub(crate) fn gen_match_instance_object_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchInstanceObjectPattern<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchInstanceObjectPattern { metadata: _, properties, rest } = inner;
        self.gen_match_object_pattern_body(ctx, node, *properties, *rest)
    }

    /// `MatchInstancePattern`: `targetConstructor { ...properties }` (an
    /// identifier/member pattern immediately followed by its own
    /// `MatchInstanceObjectPattern`).
    ///
    /// `match_.rs`'s `parse_match_identifier_subpattern_flow` (~line 626),
    /// the trailing `{`-fields branch (~line 728-737).
    pub(crate) fn gen_match_instance_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchInstancePattern<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchInstancePattern { metadata: _, target_constructor, properties } = inner;
        self.gen_node(
            ctx,
            target_constructor,
            Some(Path::new(node, NodeField::target_constructor)),
        )?;
        self.space(ForceSpace::No);
        self.gen_node(ctx, properties, Some(Path::new(node, NodeField::properties)))
    }

    /// `MatchLiteralPattern`: a bare literal (`null`/`true`/`false`/number/
    /// bigint/string).
    ///
    /// `match_.rs`'s `wrap_match_literal_pattern` (~line 1153), called from
    /// every literal arm of `parse_match_subpattern_flow`.
    pub(crate) fn gen_match_literal_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchLiteralPattern<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchLiteralPattern { metadata: _, literal } = inner;
        self.gen_node(ctx, literal, Some(Path::new(node, NodeField::literal)))
    }

    /// `MatchMemberPattern`: `base.property` (property an `Identifier`) or
    /// `base[property]` (property a numeric/bigint/string literal) — there
    /// is no `computed` flag on this kind (confirmed against
    /// `include/hermes/AST/ESTree.def`'s `MatchMemberPattern` entry:
    /// `property: Identifier | Literal`), so which spelling to print is
    /// determined purely by `property`'s own node kind, mirroring exactly
    /// how the parser itself chose between the two
    /// (`match_.rs`'s `parse_match_identifier_subpattern_flow`, ~line 657:
    /// `.` consumes an identifier, `[` consumes only a numeric/bigint/string
    /// literal).
    pub(crate) fn gen_match_member_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchMemberPattern<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchMemberPattern { metadata: _, base, property } = inner;
        self.gen_node(ctx, base, Some(Path::new(node, NodeField::base)))?;
        match property {
            Node::Identifier(_) => {
                out!(self, ".");
                self.gen_node(ctx, property, Some(Path::new(node, NodeField::property)))
            }
            _ => {
                out!(self, "[");
                self.gen_node(ctx, property, Some(Path::new(node, NodeField::property)))?;
                out!(self, "]");
                Ok(())
            }
        }
    }

    /// `MatchObjectPattern`: `{ properties, ...[rest] }`.
    ///
    /// `match_.rs`'s `parse_match_object_pattern_flow` (~line 996).
    pub(crate) fn gen_match_object_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchObjectPattern<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchObjectPattern { metadata: _, properties, rest } = inner;
        self.gen_match_object_pattern_body(ctx, node, *properties, *rest)
    }

    /// `MatchObjectPatternProperty`: `key: pattern`, or — when `shorthand`
    /// — just `pattern` alone (a `const`/`var`/`let` binding pattern whose
    /// own printed `id` already equals `key`, e.g. `{ const x }` rather than
    /// `{ x: const x }`).
    ///
    /// `match_.rs`'s `parse_match_object_pattern_properties_flow`
    /// (~line 860), the shorthand branch (~line 877-895) vs the normal
    /// `key: pattern` branch (~line 897-969).
    pub(crate) fn gen_match_object_pattern_property<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchObjectPatternProperty<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchObjectPatternProperty { metadata: _, key, pattern, shorthand } = inner;
        if shorthand.get() {
            self.gen_node(ctx, pattern, Some(Path::new(node, NodeField::pattern)))
        } else {
            self.gen_node(ctx, key, Some(Path::new(node, NodeField::key)))?;
            out!(self, ":");
            self.space(ForceSpace::No);
            self.gen_node(ctx, pattern, Some(Path::new(node, NodeField::pattern)))
        }
    }

    /// `MatchOrPattern`: `pattern0 | pattern1 | ...`. An element needs
    /// parens exactly when it is itself a `MatchAsPattern`/`MatchOrPattern`
    /// — reachable here only through an explicit `( MatchPattern )` group
    /// (`parseMatchSubpatternFlow`'s `l_paren` arm calls the *full*
    /// `parseMatchPatternFlow`, not itself, and unwraps with no wrapper
    /// node), never bare (`parseMatchSubpatternFlow` itself has no `|`/`as`
    /// case at all). See `precedence.rs`'s `MATCH_SUBPATTERN`/
    /// `MatchOrPattern` review-round-2 comment for the full trace —
    /// `print_child` (via each element's own `get_precedence`) makes this
    /// exact, rather than the "grammar makes it structurally impossible"
    /// claim this doc comment used to (incorrectly) make.
    ///
    /// `match_.rs`'s `parse_match_pattern_flow` (~line 428), the or-pattern
    /// loop (~line 437-449) and the `l_paren` group arm of
    /// `parse_match_subpattern_flow` (~line 591-605).
    pub(crate) fn gen_match_or_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchOrPattern<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchOrPattern { metadata: _, patterns } = inner;
        for (i, pattern) in patterns.iter().enumerate() {
            if i > 0 {
                self.space(ForceSpace::No);
                out!(self, "|");
                self.space(ForceSpace::No);
            }
            self.print_child(
                ctx,
                Some(pattern),
                Path::new(node, NodeField::patterns),
                ChildPos::Anywhere,
            )?;
        }
        Ok(())
    }

    /// `MatchRestPattern`: `...[const|var|let id]`. The binding is
    /// optional — a bare `...` with no following binding keyword is legal
    /// here (`match_.rs`'s `parse_match_rest_pattern_flow`, ~line 831:
    /// `arg` stays `None` unless `const`/`var`/`let` follows).
    pub(crate) fn gen_match_rest_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchRestPattern<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchRestPattern { metadata: _, argument } = inner;
        out!(self, "...");
        if let Some(argument) = argument {
            self.gen_node(ctx, argument, Some(Path::new(node, NodeField::argument)))?;
        }
        Ok(())
    }

    /// `MatchUnaryPattern`: `+`/`-` immediately followed by a numeric/bigint
    /// literal (no space — mirrors the source spelling `-5`/`+5n`).
    ///
    /// `match_.rs`'s `parse_match_subpattern_flow` (~line 545), the
    /// `plus`/`minus` arm.
    pub(crate) fn gen_match_unary_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MatchUnaryPattern<'gc>,
    ) -> Result<(), GenJsError> {
        let MatchUnaryPattern { metadata: _, argument, operator } = inner;
        let op = MatchUnaryOperator::from_label(ctx, operator.get())?.as_str();
        out!(self, "{}", op);
        self.gen_node(ctx, argument, Some(Path::new(node, NodeField::argument)))
    }

    /// `MatchWildcardPattern`: the literal `_`. No fields besides metadata —
    /// mirrors `GenJS::gen_any_type_annotation`'s zero-argument shape for a
    /// fixed-spelling, field-less kind.
    ///
    /// `match_.rs`'s `parse_match_identifier_subpattern_flow` (~line 626),
    /// the `_` wildcard check (~line 630-639).
    pub(crate) fn gen_match_wildcard_pattern(&mut self) -> Result<(), GenJsError> {
        out!(self, "_");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Step 3: the Flow `record` family (7 kinds — the brief's "6" undercounts
// `RecordExpressionProperties`), gated on `parse_flow_records`. Derived from
// `crates/parser/src/js/flow/{mod,declarations}.rs`.
// ---------------------------------------------------------------------------

impl<'s, 'w> GenJS<'s, 'w> {
    /// `RecordDeclaration`: `record Id[<T>] [implements I0, I1] body`.
    ///
    /// `declarations.rs`'s `parse_record_declaration_flow` (~line 815).
    pub(crate) fn gen_record_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &RecordDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let RecordDeclaration { metadata: _, id, type_parameters, implements, body } = inner;
        out!(self, "record");
        self.space(ForceSpace::Yes);
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        if !implements.is_empty() {
            self.space(ForceSpace::Yes);
            out!(self, "implements");
            self.space(ForceSpace::Yes);
            for (i, im) in implements.iter().enumerate() {
                if i > 0 {
                    self.comma();
                }
                self.gen_node(ctx, im, Some(Path::new(node, NodeField::implements)))?;
            }
        }
        self.space(ForceSpace::No);
        self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))
    }

    /// `RecordDeclarationBody`: `{ elements }`, where each element is a
    /// `RecordDeclarationProperty`/`RecordDeclarationStaticProperty`
    /// (mandatory trailing `,` — `declarations.rs` ~line 1013-1025) or a
    /// `MethodDefinition` (no trailing separator at all —
    /// `declarations.rs`'s method branch, ~line 1026-1119, never eats a
    /// comma/semicolon after pushing). Which kind decides the separator, not
    /// list position.
    ///
    /// `declarations.rs`'s `parse_record_declaration_flow` (~line 815), the
    /// body loop (~line 878-1128).
    pub(crate) fn gen_record_declaration_body<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &RecordDeclarationBody<'gc>,
    ) -> Result<(), GenJsError> {
        let RecordDeclarationBody { metadata: _, elements } = inner;
        out!(self, "{{");
        self.inc_indent();
        self.newline();
        for (i, el) in elements.iter().enumerate() {
            if i > 0 {
                self.newline();
            }
            self.gen_node(ctx, el, Some(Path::new(node, NodeField::elements)))?;
            if matches!(
                el,
                Node::RecordDeclarationProperty(_) | Node::RecordDeclarationStaticProperty(_)
            ) {
                out!(self, ",");
            }
        }
        self.dec_indent();
        self.newline();
        out!(self, "}}");
        Ok(())
    }

    /// `RecordDeclarationImplements`: `Id[<TypeArgs>]` (one entry of a
    /// `record`'s `implements` clause).
    ///
    /// `declarations.rs`'s `parse_record_declaration_implements_flow`
    /// (~line 1190).
    pub(crate) fn gen_record_declaration_implements<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &RecordDeclarationImplements<'gc>,
    ) -> Result<(), GenJsError> {
        let RecordDeclarationImplements { metadata: _, id, type_arguments } = inner;
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(type_arguments) = type_arguments {
            self.gen_node(
                ctx,
                type_arguments,
                Some(Path::new(node, NodeField::type_arguments)),
            )?;
        }
        Ok(())
    }

    /// `RecordDeclarationProperty`: `key: type_annotation[ = default_value]`.
    /// The caller ([`GenJS::gen_record_declaration_body`]) adds the
    /// mandatory trailing `,`.
    ///
    /// `declarations.rs`'s `parse_record_declaration_flow` (~line 815), the
    /// non-`static` property branch (~line 995-1010).
    pub(crate) fn gen_record_declaration_property<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &RecordDeclarationProperty<'gc>,
    ) -> Result<(), GenJsError> {
        let RecordDeclarationProperty { metadata: _, key, type_annotation, default_value } =
            inner;
        self.gen_node(ctx, key, Some(Path::new(node, NodeField::key)))?;
        out!(self, ":");
        self.space(ForceSpace::No);
        self.gen_node(
            ctx,
            type_annotation,
            Some(Path::new(node, NodeField::type_annotation)),
        )?;
        if let Some(default_value) = default_value {
            self.space(ForceSpace::No);
            self.space_before_equals("=");
            out!(self, "=");
            self.space(ForceSpace::No);
            self.gen_node(
                ctx,
                default_value,
                Some(Path::new(node, NodeField::default_value)),
            )?;
        }
        Ok(())
    }

    /// `RecordDeclarationStaticProperty`: `static key: type_annotation =
    /// value` — the initializer is mandatory (`declarations.rs` ~line
    /// 976-985 errors without one). The caller adds the trailing `,`.
    ///
    /// `declarations.rs`'s `parse_record_declaration_flow` (~line 815), the
    /// `is_static` property branch (~line 975-994).
    pub(crate) fn gen_record_declaration_static_property<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &RecordDeclarationStaticProperty<'gc>,
    ) -> Result<(), GenJsError> {
        let RecordDeclarationStaticProperty { metadata: _, key, type_annotation, value } = inner;
        out!(self, "static");
        self.space(ForceSpace::Yes);
        self.gen_node(ctx, key, Some(Path::new(node, NodeField::key)))?;
        out!(self, ":");
        self.space(ForceSpace::No);
        self.gen_node(
            ctx,
            type_annotation,
            Some(Path::new(node, NodeField::type_annotation)),
        )?;
        self.space(ForceSpace::No);
        self.space_before_equals("=");
        out!(self, "=");
        self.space(ForceSpace::No);
        self.gen_node(ctx, value, Some(Path::new(node, NodeField::value)))
    }

    /// `RecordExpression`: `Constructor[<TypeArgs>] { properties }` — no
    /// `record` keyword; the record-ness is purely `check_record_expression_flow`'s
    /// disambiguation (an uppercase-leading identifier or a member
    /// expression, immediately followed by `{` with no newline).
    ///
    /// `mod.rs`'s `parse_record_expression_flow` (~line 91).
    pub(crate) fn gen_record_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &RecordExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let RecordExpression { metadata: _, record_constructor, type_arguments, properties } =
            inner;
        self.gen_node(
            ctx,
            record_constructor,
            Some(Path::new(node, NodeField::record_constructor)),
        )?;
        if let Some(type_arguments) = type_arguments {
            self.gen_node(
                ctx,
                type_arguments,
                Some(Path::new(node, NodeField::type_arguments)),
            )?;
        }
        self.space(ForceSpace::No);
        self.gen_node(ctx, properties, Some(Path::new(node, NodeField::properties)))
    }

    /// `RecordExpressionProperties`: `{ properties }` — the same
    /// `Property`/`SpreadElement` shape as an `ObjectExpression`
    /// (`mod.rs`'s `parse_record_expression_flow` calls the shared
    /// `parse_object_properties`, ~line 103), so this reuses
    /// [`GenJS::visit_props`] (Task 5) directly.
    pub(crate) fn gen_record_expression_properties<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &RecordExpressionProperties<'gc>,
    ) -> Result<(), GenJsError> {
        let RecordExpressionProperties { metadata: _, properties } = inner;
        self.visit_props(ctx, *properties, Path::new(node, NodeField::properties))
    }
}

// ---------------------------------------------------------------------------
// Step 4: Flow `component`/`hook` (8 kinds), gated on
// `parse_flow_component_syntax`. Derived from `crates/parser/src/js/flow/
// {declarations,types}.rs`.
// ---------------------------------------------------------------------------

impl<'s, 'w> GenJS<'s, 'w> {
    /// `ComponentDeclaration`: `[async ]component Id[<T>](params)[ renders
    /// R] body`. `params` mixes `ComponentParameter` and (for a `...rest`)
    /// plain `RestElement` nodes in one list — both already have their own
    /// arms, so a uniform `gen_node` per element is correct
    /// (`declarations.rs`'s `parse_component_parameters_flow`, ~line 390,
    /// pushes a `parse_binding_rest_element` result directly into the same
    /// `param_list`, not a separate field).
    ///
    /// `declarations.rs`'s `parse_component_declaration_flow` (~line 264),
    /// the non-`declare` tail (~line 345-380).
    pub(crate) fn gen_component_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ComponentDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let ComponentDeclaration {
            metadata: _,
            id,
            params,
            body,
            type_parameters,
            renders_type,
            r#async,
            scope: _,
            sem_info: _,
            strictness: _,
            is_method_definition: _,
            decorations: _,
        } = inner;
        if r#async.get() {
            out!(self, "async ");
        }
        out!(self, "component");
        self.space(ForceSpace::Yes);
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        out!(self, "(");
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.gen_node(ctx, param, Some(Path::new(node, NodeField::params)))?;
        }
        out!(self, ")");
        if let Some(renders_type) = renders_type {
            self.space(ForceSpace::Yes);
            self.gen_node(ctx, renders_type, Some(Path::new(node, NodeField::renders_type)))?;
        }
        self.space(ForceSpace::No);
        self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))
    }

    /// `ComponentParameter`: `local` alone when `shorthand` (an identifier
    /// local whose printed form, including any `?`/type annotation/default,
    /// already spells the same name as `name`), else `name as local`
    /// (covers both the string-literal-name and identifier-name-with-`as`
    /// source shapes — both build `shorthand: false`).
    ///
    /// `declarations.rs`'s `parse_component_parameter_flow` (~line 444).
    pub(crate) fn gen_component_parameter<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ComponentParameter<'gc>,
    ) -> Result<(), GenJsError> {
        let ComponentParameter { metadata: _, name, local, shorthand } = inner;
        if shorthand.get() {
            self.gen_node(ctx, local, Some(Path::new(node, NodeField::local)))
        } else {
            self.gen_node(ctx, name, Some(Path::new(node, NodeField::name)))?;
            self.space(ForceSpace::Yes);
            out!(self, "as");
            self.space(ForceSpace::Yes);
            self.gen_node(ctx, local, Some(Path::new(node, NodeField::local)))
        }
    }

    /// `ComponentTypeAnnotation`: `component[<T>](params[, ...rest])[
    /// renders R]` — a component TYPE value (e.g. inside `type F =
    /// component(x: number);`), unlike [`GenJS::gen_component_declaration`]'s
    /// statement form. `rest`'s leading `...` is printed here, not by
    /// [`GenJS::gen_component_type_parameter`] itself — see that method's
    /// doc comment.
    ///
    /// `types.rs`'s `parse_component_type_annotation_flow` (~line 1337).
    pub(crate) fn gen_component_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ComponentTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let ComponentTypeAnnotation { metadata: _, params, rest, type_parameters, renders_type } =
            inner;
        out!(self, "component");
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        out!(self, "(");
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.gen_node(ctx, param, Some(Path::new(node, NodeField::params)))?;
        }
        if let Some(rest) = rest {
            if !params.is_empty() {
                self.comma();
            }
            out!(self, "...");
            self.gen_node(ctx, rest, Some(Path::new(node, NodeField::rest)))?;
        }
        out!(self, ")");
        if let Some(renders_type) = renders_type {
            self.space(ForceSpace::Yes);
            self.gen_node(ctx, renders_type, Some(Path::new(node, NodeField::renders_type)))?;
        }
        Ok(())
    }

    /// `ComponentTypeParameter`: `[name[?]: ]type_annotation` — `name` is
    /// absent only for an unlabeled `...T` rest parameter
    /// (`types.rs`'s `parse_component_type_rest_parameter_flow`, ~line 1450,
    /// leaves `name: None` when no `:`/`?` follows). The caller prints any
    /// leading `...` for the `rest` field — this method is reused verbatim
    /// for both an ordinary `params`-list entry and the `rest` entry (see
    /// [`GenJS::gen_component_type_annotation`]/[`GenJS::gen_declare_component`]),
    /// and the node itself carries no flag distinguishing the two.
    ///
    /// `types.rs`'s `parse_component_type_parameter_flow` (~line 1506) and
    /// `parse_component_type_rest_parameter_flow` (~line 1450).
    pub(crate) fn gen_component_type_parameter<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ComponentTypeParameter<'gc>,
    ) -> Result<(), GenJsError> {
        let ComponentTypeParameter { metadata: _, name, type_annotation, optional } = inner;
        if let Some(name) = name {
            self.gen_node(ctx, name, Some(Path::new(node, NodeField::name)))?;
            if optional.get() {
                out!(self, "?");
            }
            out!(self, ":");
            self.space(ForceSpace::No);
        }
        self.gen_node(
            ctx,
            type_annotation,
            Some(Path::new(node, NodeField::type_annotation)),
        )
    }

    /// `DeclareComponent`: `[declare ]component Id[<T>](params[,
    /// ...rest])[ renders R]` — no trailing `;`; the caller
    /// (`visit_stmt_in_block`) adds it, since this never ends in `}` (see
    /// the module doc comment's `stmt_skip_semi` section). No `async`: the
    /// struct has no such field at all — `declarations.rs`'s
    /// `parse_component_declaration_flow` (~line 264) discards any
    /// `is_async` argument on the `declare` early-return path (~line
    /// 325-343), so there is nothing for this arm to print even for a
    /// (hypothetically reachable) `declare async component`.
    ///
    /// `declare_prefix_needed` (Task 11, bumped `pub(crate)` for this task —
    /// see the module doc comment) — `DeclareComponent` is reachable as a
    /// `DeclareExportDeclaration`'s `declaration` exactly like Task 11's
    /// four `Declare*` kinds (`declarations.rs`'s `parse_declare_export_flow`,
    /// ~line 2395-2402/2306-2313).
    pub(crate) fn gen_declare_component<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclareComponent<'gc>,
        path: Option<Path<'gc>>,
    ) -> Result<(), GenJsError> {
        let DeclareComponent { metadata: _, id, params, rest, type_parameters, renders_type } =
            inner;
        if declare_prefix_needed(path) {
            out!(self, "declare component");
        } else {
            out!(self, "component");
        }
        self.space(ForceSpace::Yes);
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        out!(self, "(");
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.gen_node(ctx, param, Some(Path::new(node, NodeField::params)))?;
        }
        if let Some(rest) = rest {
            if !params.is_empty() {
                self.comma();
            }
            out!(self, "...");
            self.gen_node(ctx, rest, Some(Path::new(node, NodeField::rest)))?;
        }
        out!(self, ")");
        if let Some(renders_type) = renders_type {
            self.space(ForceSpace::Yes);
            self.gen_node(ctx, renders_type, Some(Path::new(node, NodeField::renders_type)))?;
        }
        Ok(())
    }

    /// `DeclareHook`: `[declare ]hook Id(params)[: R]` — no trailing `;`,
    /// same reasoning as [`GenJS::gen_declare_component`]. Unlike every
    /// other `Declare*` kind, the params/return-type signature is not a
    /// direct field of `DeclareHook` at all: it lives inside `id`'s own
    /// `type_annotation` as a `TypeAnnotation`-wrapped `HookTypeAnnotation`
    /// (`declarations.rs`'s `parse_declare_function_or_hook_flow`, ~line
    /// 1848-1875) — the exact same shape `DeclareFunction` uses for a
    /// `FunctionTypeAnnotation` (`arms/flow_decl.rs`'s `gen_declare_function`,
    /// Task 11), which this mirrors: destructure through `id`, then reuse
    /// [`GenJS::visit_func_type_params`] (Task 7) directly (`this: None` —
    /// `HookTypeAnnotation` has no `this` field; hooks never take one) with
    /// a `:`-style return type, rather than
    /// [`GenJS::gen_hook_type_annotation`]'s `=>`-style — the two spellings
    /// are chosen by *context* (declaration vs. value position), not by the
    /// `HookTypeAnnotation` node's own shape, exactly mirroring
    /// `FunctionTypeAnnotation`'s existing `:`-vs-`=>` split between
    /// `gen_declare_function`/`gen_function_type_annotation`.
    pub(crate) fn gen_declare_hook<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclareHook<'gc>,
        path: Option<Path<'gc>>,
    ) -> Result<(), GenJsError> {
        let DeclareHook { metadata: _, id } = inner;
        let Node::Identifier(Identifier {
            metadata: _,
            name,
            type_annotation,
            optional: _,
            unresolvable: _,
            decl_state: _,
            decl: _,
        }) = id
        else {
            return Err(GenJsError::UnsupportedKind(id.kind()));
        };
        if declare_prefix_needed(path) {
            out!(self, "declare hook");
        } else {
            out!(self, "hook");
        }
        self.space(ForceSpace::Yes);
        let name_str = ctx
            .try_bytes_str(name.get())
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        self.write_utf8(name_str);
        let Some(annot) = type_annotation else {
            return Err(GenJsError::UnsupportedKind(id.kind()));
        };
        let Node::TypeAnnotation(TypeAnnotation { metadata: _, type_annotation: hta }) = annot
        else {
            return Err(GenJsError::UnsupportedKind(annot.kind()));
        };
        let Node::HookTypeAnnotation(HookTypeAnnotation {
            metadata: _,
            params,
            return_type,
            rest,
            type_parameters,
        }) = hta
        else {
            return Err(GenJsError::UnsupportedKind(hta.kind()));
        };
        self.visit_func_type_params(ctx, *params, None, *rest, *type_parameters, node)?;
        out!(self, ":");
        self.space(ForceSpace::No);
        self.gen_node(ctx, return_type, Some(Path::new(node, NodeField::return_type)))
    }

    /// `HookDeclaration`: `[async ]hook Id[<T>](params)[: R] body`. Reuses
    /// [`GenJS::visit_func_type_params`]'s sibling,
    /// [`GenJS::visit_func_params_body`] (Task 7) directly (`predicate:
    /// None` — hooks have no `%checks` predicate field at all; `declarations.rs`'s
    /// `parse_hook_declaration_flow`, ~line 741-752, explicitly rejects
    /// `checks` after `hook`'s return type).
    ///
    /// `declarations.rs`'s `parse_hook_declaration_flow` (~line 688).
    pub(crate) fn gen_hook_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &HookDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let HookDeclaration {
            metadata: _,
            id,
            params,
            body,
            type_parameters,
            return_type,
            r#async,
            scope: _,
            sem_info: _,
            strictness: _,
            is_method_definition: _,
            decorations: _,
        } = inner;
        if r#async.get() {
            out!(self, "async ");
        }
        out!(self, "hook");
        self.space(ForceSpace::Yes);
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        self.visit_func_params_body(ctx, *params, *type_parameters, *return_type, None, body, node)
    }

    /// `HookTypeAnnotation`: `hook[<T>](params[, ...rest]) => R` — the
    /// *value*-position spelling (e.g. `type F = hook() => void;`), `=>`
    /// not `:`; see [`GenJS::gen_declare_hook`]'s doc comment for the
    /// context-dependent split with the `:`-style declaration spelling.
    ///
    /// `types.rs`'s `parse_hook_type_annotation_flow` (~line 1597), which
    /// delegates the actual node construction to the shared
    /// `parse_function_or_hook_type_annotation_flow(true)`.
    pub(crate) fn gen_hook_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &HookTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let HookTypeAnnotation { metadata: _, params, return_type, rest, type_parameters } =
            inner;
        out!(self, "hook");
        self.visit_func_type_params(ctx, *params, None, *rest, *type_parameters, node)?;
        if self.pretty() == Pretty::Yes {
            out!(self, " => ");
        } else {
            self.space_before_equals("=>");
            out!(self, "=>");
        }
        self.gen_node(ctx, return_type, Some(Path::new(node, NodeField::return_type)))
    }
}

// ---------------------------------------------------------------------------
// Step 5: the remaining type kinds (16), plus `DeclareEnum`/`DeclareNamespace`
// and the `EnumBigInt*` pair. Derived from `crates/parser/src/js/flow/
// {types,object_types,declarations}.rs`.
// ---------------------------------------------------------------------------

/// `TypeOperator::operator`, classified from its raw spelling
/// (`declarations.rs`'s `parse_component_render_type_flow` ~line 654 and
/// `types.rs`'s `parse_render_type_operator` ~line 600: `renders`/
/// `renders?`/`renders*` — `TypeOperator` has no other producer in this
/// parser).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum TypeOperatorKeyword {
    Renders,
    RendersOptional,
    RendersStar,
}

impl TypeOperatorKeyword {
    fn from_label(gc: &GCLock<'_, '_>, label: NodeLabel) -> Result<Self, GenJsError> {
        Ok(match gc.bytes_str_lossy(label) {
            "renders" => Self::Renders,
            "renders?" => Self::RendersOptional,
            "renders*" => Self::RendersStar,
            other => {
                return Err(GenJsError::UnknownOperator {
                    kind: "TypeOperator",
                    spelling: other.to_string(),
                })
            }
        })
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Renders => "renders",
            Self::RendersOptional => "renders?",
            Self::RendersStar => "renders*",
        }
    }
}

impl<'s, 'w> GenJS<'s, 'w> {
    /// `ConditionalTypeAnnotation`: `check_type extends extends_type ?
    /// true_type : false_type`.
    ///
    /// `types.rs`'s `parse_conditional_type_annotation_flow` (~line 186).
    pub(crate) fn gen_conditional_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ConditionalTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let ConditionalTypeAnnotation { metadata: _, check_type, extends_type, true_type, false_type } =
            inner;
        // `check_type`/`extends_type` are both parsed at union tier
        // (`parse_conditional_type_annotation_flow`), not the full type
        // grammar — `print_child` (`ChildPos::Right`; the precedence
        // threshold comes from this node's own `get_precedence` entry,
        // `ALWAYS_PAREN` — see precedence.rs's "review round 2" comment for
        // why `ChildPos` is irrelevant here) protects a parenthesized-in-
        // source looser construct (chiefly a nested
        // `ConditionalTypeAnnotation`) from losing its grouping.
        // `true_type`/`false_type`, by contrast,
        // are parsed via the *full* `parse_type_annotation_flow` (right-
        // recursive nesting, e.g. `A ? B : C ? D : E`), so they stay plain
        // `gen_node`.
        self.print_child(
            ctx,
            Some(*check_type),
            Path::new(node, NodeField::check_type),
            ChildPos::Right,
        )?;
        self.space(ForceSpace::Yes);
        out!(self, "extends");
        self.space(ForceSpace::Yes);
        self.print_child(
            ctx,
            Some(*extends_type),
            Path::new(node, NodeField::extends_type),
            ChildPos::Right,
        )?;
        self.space(ForceSpace::No);
        out!(self, "?");
        self.space(ForceSpace::No);
        self.gen_node(ctx, true_type, Some(Path::new(node, NodeField::true_type)))?;
        self.space(ForceSpace::No);
        out!(self, ":");
        self.space(ForceSpace::No);
        self.gen_node(ctx, false_type, Some(Path::new(node, NodeField::false_type)))
    }

    /// `InferTypeAnnotation`: `infer name[ extends bound]`. See the module
    /// doc comment's "A real bug avoided" section for why `type_parameter`
    /// is destructured inline here rather than delegated to
    /// `GenJS::gen_type_parameter` (which would print an unreparseable
    /// `infer name: bound`).
    ///
    /// `types.rs`'s `parse_primary_type_annotation_flow`'s `infer` arm
    /// (~line 625-773).
    pub(crate) fn gen_infer_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &InferTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let InferTypeAnnotation { metadata: _, type_parameter } = inner;
        out!(self, "infer");
        self.space(ForceSpace::Yes);
        let Node::TypeParameter(TypeParameter {
            metadata: _,
            name,
            r#const: _,
            bound,
            variance: _,
            default: _,
            uses_extends_bound: _,
        }) = type_parameter
        else {
            return Err(GenJsError::UnsupportedKind(type_parameter.kind()));
        };
        let name_str = ctx
            .try_bytes_str(name.get())
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        self.write_utf8(name_str);
        if let Some(bound) = bound {
            self.space(ForceSpace::Yes);
            out!(self, "extends");
            self.space(ForceSpace::Yes);
            // `bound` is parsed at union tier (the speculative
            // `parse_union_type_annotation_flow()` call in the `infer` arm
            // of `parse_primary_type_annotation_flow`), so it needs the
            // same `print_child` protection as `ConditionalTypeAnnotation`'s
            // `check_type`/`extends_type` — see precedence.rs's
            // "review round 2" comment for why dropping this is worse than
            // the other four defects: it does not merely change the tree,
            // it makes the regenerated source **fail to reparse** (the
            // speculative-bound backtrack bails to bare `infer B` the
            // moment a bare `extends` follows a union-tier parse).
            self.print_child(
                ctx,
                Some(bound),
                Path::new(node, NodeField::type_parameter),
                ChildPos::Right,
            )?;
        }
        Ok(())
    }

    /// `KeyofTypeAnnotation`: `keyof argument`.
    ///
    /// `types.rs`'s `parse_primary_type_annotation_flow`'s `keyof` arm
    /// (~line 635-649).
    pub(crate) fn gen_keyof_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &KeyofTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let KeyofTypeAnnotation { metadata: _, argument } = inner;
        out!(self, "keyof");
        self.space(ForceSpace::Yes);
        // `argument` is parsed at prefix tier (`parse_prefix_type_annotation_flow`,
        // the `NamedType::Keyof` arm) — the same restricted tier
        // `NullableTypeAnnotation`'s own operand is built from, so this
        // mirrors `gen_nullable_type_annotation`'s `print_child`/
        // `ChildPos::Right` exactly rather than a bare `gen_node`. Without
        // this, `keyof (A | B)` regenerated as `Union[Keyof(A), B]` — a
        // different top-level kind.
        self.print_child(
            ctx,
            Some(*argument),
            Path::new(node, NodeField::argument),
            ChildPos::Right,
        )
    }

    /// `NeverTypeAnnotation`: `never`.
    ///
    /// `types.rs`'s `parse_primary_type_annotation_flow`, the `b"never"`
    /// primitive arm (~line 573-575).
    pub(crate) fn gen_never_type_annotation(&mut self) -> Result<(), GenJsError> {
        out!(self, "never");
        Ok(())
    }

    /// `UndefinedTypeAnnotation`: `undefined`.
    ///
    /// `types.rs`'s `parse_primary_type_annotation_flow`, the
    /// `b"undefined"` primitive arm (~line 577-581).
    pub(crate) fn gen_undefined_type_annotation(&mut self) -> Result<(), GenJsError> {
        out!(self, "undefined");
        Ok(())
    }

    /// `UnknownTypeAnnotation`: `unknown`.
    ///
    /// `types.rs`'s `parse_primary_type_annotation_flow`, the `b"unknown"`
    /// primitive arm (~line 567-571).
    pub(crate) fn gen_unknown_type_annotation(&mut self) -> Result<(), GenJsError> {
        out!(self, "unknown");
        Ok(())
    }

    /// `TypeOperator`: `renders|renders?|renders* type_annotation` — the
    /// only producer of this kind in this parser is the `renders`
    /// component-render-type operator (see [`TypeOperatorKeyword`]'s doc
    /// comment).
    pub(crate) fn gen_type_operator<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TypeOperator<'gc>,
    ) -> Result<(), GenJsError> {
        let TypeOperator { metadata: _, operator, type_annotation } = inner;
        let kw = TypeOperatorKeyword::from_label(ctx, operator.get())?.as_str();
        out!(self, "{}", kw);
        self.space(ForceSpace::Yes);
        // `type_annotation` is parsed at prefix tier in two of this kind's
        // three construction sites (`parse_component_render_type_flow`'s
        // `component_type: true` branch, i.e. `ComponentTypeAnnotation`'s
        // `renders_type`; and the `NamedType::Renders` primary-tier arm) —
        // both call `parse_prefix_type_annotation_flow()` for the body. The
        // third site (`component_type: false`, `ComponentDeclaration`/
        // `DeclareComponent`'s `renders_type`) uses the full
        // `parse_type_annotation_flow` instead, but the AST cannot
        // distinguish which site built a given node, so this always applies
        // the more restrictive prefix-tier protection — safe for the
        // full-tier case too (a redundant paren pair there still reparses
        // identically). Mirrors `gen_nullable_type_annotation`'s
        // `print_child`/`ChildPos::Right` exactly, the same restricted tier.
        // Without this, `component() renders (A | B)` regenerated with a
        // bare `UnionTypeAnnotation` `renders_type` — the top-level kind of
        // the `renders` clause's own body changed.
        self.print_child(
            ctx,
            Some(*type_annotation),
            Path::new(node, NodeField::type_annotation),
            ChildPos::Right,
        )
    }

    /// `TypePredicate`: `[asserts|implies ]parameter_name[ is
    /// type_annotation]` — `kind` is an *optional* `NodeString` (a bare `x
    /// is T` return-type predicate has no prefix keyword at all, encoded as
    /// `INVALID_ATOM_BYTES` — `flow/function_types.rs`'s
    /// `parse_return_type_annotation_flow`, ~line 199-204, mirrors the same
    /// "null `NodeString` for absent" idiom
    /// `arms/stmt.rs`'s `gen_expression_statement` already established for
    /// `ExpressionStatement::directive`).
    ///
    /// `function_types.rs`'s `parse_return_type_annotation_flow` (~line 39),
    /// all three shapes (`asserts` ~line 46-99, `implies` ~line 100-176, bare
    /// `is` ~line 177-213).
    pub(crate) fn gen_type_predicate<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TypePredicate<'gc>,
    ) -> Result<(), GenJsError> {
        let TypePredicate { metadata: _, parameter_name, type_annotation, kind } = inner;
        match ctx.try_bytes_str(kind.get()) {
            Some("asserts") => {
                out!(self, "asserts");
                self.space(ForceSpace::Yes);
            }
            Some("implies") => {
                out!(self, "implies");
                self.space(ForceSpace::Yes);
            }
            Some(other) => {
                return Err(GenJsError::UnknownOperator {
                    kind: "TypePredicate",
                    spelling: other.to_string(),
                })
            }
            None => {}
        }
        self.gen_node(
            ctx,
            parameter_name,
            Some(Path::new(node, NodeField::parameter_name)),
        )?;
        if let Some(type_annotation) = type_annotation {
            self.space(ForceSpace::Yes);
            out!(self, "is");
            self.space(ForceSpace::Yes);
            // `print_child`, not `gen_node`: the operand inherits the
            // caller's `AllowAnonFunctionType`, so in an arrow's return type
            // it is parsed in the same `No` region the return type is, and a
            // `FunctionTypeAnnotation`/`ConditionalTypeAnnotation` there
            // loses its source parens and either reparses differently
            // (`x is (number=>string)`) or not at all
            // (`x is (A extends B ? C : D)`). A `TypePredicate` cannot
            // itself be wrapped in parens (`x is T` is not a type), so the
            // fix has to live on this call rather than in `need_parens`:
            // both hazard kinds are `ALWAYS_PAREN` in `get_precedence`
            // (`precedence.rs`), so routing through `print_child` alone is
            // what forces the parens — see `precedence.rs`'s
            // `flow_no_anon_region_hazard` ("Why `TypePredicate` stops the
            // walk instead of continuing it") for why a matching branch in
            // `need_parens` was tried and deleted as dead code. This was the
            // "bare `gen_node` where the parser accepts a narrower tier"
            // shape, found in Task 15's review.
            self.print_child(
                ctx,
                Some(type_annotation),
                Path::new(node, NodeField::type_annotation),
                ChildPos::Right,
            )?;
        }
        Ok(())
    }

    /// `ObjectTypeMappedTypeProperty`: `[variance][key_tparam in
    /// source_type][+?|-?|?]: prop_type` — the `[`/`in`/`]` punctuation has
    /// no dedicated node (the caller consumed it directly), so this method
    /// prints all of it itself; `optional`'s three named states
    /// (`"PlusOptional"`/`"MinusOptional"`/`"Optional"`) translate to actual
    /// punctuation, unlike [`TypeOperatorKeyword`]'s verbatim-spelling
    /// fields — the *absent* state (`None` via `try_bytes_str`) is a normal,
    /// expected outcome here (not an error), same idiom as
    /// [`GenJS::gen_type_predicate`]'s `kind`.
    ///
    /// `object_types.rs`'s `parse_type_mapped_type_property_flow`
    /// (~line 763).
    pub(crate) fn gen_object_type_mapped_type_property<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ObjectTypeMappedTypeProperty<'gc>,
    ) -> Result<(), GenJsError> {
        let ObjectTypeMappedTypeProperty {
            metadata: _,
            key_tparam,
            prop_type,
            source_type,
            variance,
            optional,
        } = inner;
        if let Some(variance) = variance {
            self.gen_node(ctx, variance, Some(Path::new(node, NodeField::variance)))?;
        }
        out!(self, "[");
        self.gen_node(ctx, key_tparam, Some(Path::new(node, NodeField::key_tparam)))?;
        self.space(ForceSpace::Yes);
        out!(self, "in");
        self.space(ForceSpace::Yes);
        self.gen_node(ctx, source_type, Some(Path::new(node, NodeField::source_type)))?;
        out!(self, "]");
        match ctx.try_bytes_str(optional.get()) {
            Some("PlusOptional") => out!(self, "+?"),
            Some("MinusOptional") => out!(self, "-?"),
            Some("Optional") => out!(self, "?"),
            Some(other) => {
                return Err(GenJsError::UnknownOperator {
                    kind: "ObjectTypeMappedTypeProperty",
                    spelling: other.to_string(),
                })
            }
            None => {}
        }
        out!(self, ":");
        self.space(ForceSpace::No);
        self.gen_node(ctx, prop_type, Some(Path::new(node, NodeField::prop_type)))
    }

    /// `QualifiedTypeofIdentifier`: `qualification.id` (one link of a
    /// `typeof a.b.c` dotted chain).
    ///
    /// `types.rs`'s `parse_typeof_type_annotation_flow` (~line 927), the
    /// `.`-chain loop (~line 963-1007).
    pub(crate) fn gen_qualified_typeof_identifier<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &QualifiedTypeofIdentifier<'gc>,
    ) -> Result<(), GenJsError> {
        let QualifiedTypeofIdentifier { metadata: _, qualification, id } = inner;
        self.gen_node(ctx, qualification, Some(Path::new(node, NodeField::qualification)))?;
        out!(self, ".");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))
    }

    /// `TupleTypeLabeledElement`: `[variance]label[?]: element_type` (one
    /// labeled entry of a tuple type, e.g. `[+foo?: number]`).
    ///
    /// `types.rs`'s `parse_tuple_element_flow` (~line 1109), the labeled
    /// branch (~line 1195-1237).
    pub(crate) fn gen_tuple_type_labeled_element<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TupleTypeLabeledElement<'gc>,
    ) -> Result<(), GenJsError> {
        let TupleTypeLabeledElement { metadata: _, label, element_type, optional, variance } =
            inner;
        if let Some(variance) = variance {
            self.gen_node(ctx, variance, Some(Path::new(node, NodeField::variance)))?;
        }
        self.gen_node(ctx, label, Some(Path::new(node, NodeField::label)))?;
        if optional.get() {
            out!(self, "?");
        }
        out!(self, ":");
        self.space(ForceSpace::No);
        self.gen_node(ctx, element_type, Some(Path::new(node, NodeField::element_type)))
    }

    /// `TupleTypeSpreadElement`: `...[label: ]type_annotation` — `label` is
    /// present only when the spread element was written `...Identifier:
    /// Type` (`types.rs`'s `parse_tuple_element_flow`, ~line 1119-1141);
    /// a bare `...Type` has no label at all (~line 1143-1154).
    pub(crate) fn gen_tuple_type_spread_element<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TupleTypeSpreadElement<'gc>,
    ) -> Result<(), GenJsError> {
        let TupleTypeSpreadElement { metadata: _, label, type_annotation } = inner;
        out!(self, "...");
        if let Some(label) = label {
            self.gen_node(ctx, label, Some(Path::new(node, NodeField::label)))?;
            out!(self, ":");
            self.space(ForceSpace::No);
        }
        self.gen_node(
            ctx,
            type_annotation,
            Some(Path::new(node, NodeField::type_annotation)),
        )
    }

    /// `DeclareEnum`: `[declare ]enum Id body` — reachable inside `declare
    /// export` exactly like Task 11's four `Declare*` kinds
    /// (`declarations.rs`'s `parse_declare_export_flow`, ~line 2403-2407),
    /// so this uses [`declare_prefix_needed`] the same way.
    ///
    /// `declarations.rs`'s `parse_enum_declaration_flow` (~line 2680), the
    /// `declare` branch (~line 2752-2760).
    pub(crate) fn gen_declare_enum<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclareEnum<'gc>,
        path: Option<Path<'gc>>,
    ) -> Result<(), GenJsError> {
        let DeclareEnum { metadata: _, id, body } = inner;
        if declare_prefix_needed(path) {
            out!(self, "declare enum ");
        } else {
            out!(self, "enum ");
        }
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))
    }

    /// `DeclareNamespace`: `declare namespace Id body` — unlike
    /// `DeclareEnum`/`DeclareComponent`/`DeclareHook`, this is never
    /// reachable inside `declare export` (absent from
    /// `parse_declare_export_flow`'s entire dispatch chain,
    /// `declarations.rs` ~line 2247-2420+: only `function`/`hook`/`class`/
    /// `component`/`enum`/`var`/`const`/`let`/interface/module/type are
    /// routed there), so it always prints `declare ` unconditionally — no
    /// `path`/`declare_prefix_needed` needed.
    ///
    /// `declarations.rs`'s `parse_declare_namespace_flow` (~line 2027).
    pub(crate) fn gen_declare_namespace<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclareNamespace<'gc>,
    ) -> Result<(), GenJsError> {
        let DeclareNamespace { metadata: _, id, body } = inner;
        out!(self, "declare namespace ");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        self.space(ForceSpace::No);
        self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))
    }

    /// `EnumBigIntBody`: an enum body of `bigint`-valued members — the
    /// fifth `visit_enum_body` element kind, deliberately left out of Task
    /// 11's `EnumStringBody`/`EnumNumberBody`/`EnumBooleanBody`/
    /// `EnumSymbolBody` quartet (that module's own doc comment names this
    /// task as the owner).
    ///
    /// `declarations.rs`'s `parse_enum_body_flow`, the `EnumKind::BigInt`
    /// arm (~line 2949).
    pub(crate) fn gen_enum_bigint_body<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &EnumBigIntBody<'gc>,
    ) -> Result<(), GenJsError> {
        let EnumBigIntBody { metadata: _, members, explicit_type, has_unknown_members } = inner;
        self.visit_enum_body(
            ctx,
            "bigint",
            *members,
            explicit_type.get(),
            has_unknown_members.get(),
            node,
        )
    }

    /// `EnumBigIntMember`: `Id = 42n` — byte-for-byte the same shape as
    /// `EnumStringMember`/`EnumNumberMember`/`EnumBooleanMember`, so this
    /// reuses `GenJS::gen_enum_member_with_init` (Task 11, bumped
    /// `pub(crate)` for this task — see the module doc comment).
    ///
    /// `declarations.rs`'s `parse_enum_body_flow`, the `EnumKind::BigInt`
    /// member arm (~line 3122).
    pub(crate) fn gen_enum_bigint_member<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &EnumBigIntMember<'gc>,
    ) -> Result<(), GenJsError> {
        let EnumBigIntMember { metadata: _, id, init } = inner;
        self.gen_enum_member_with_init(ctx, node, id, init)
    }
}

/// Whether `type_annotation` would print as bare `const`, i.e. whether it is
/// the exact shape the parser folds into an `AsConstExpression`.
///
/// **No juno counterpart — a correctness fix found by the Tier 2 sweep**
/// (`test/Parser/flow/as-const.js`, whose second statement is `x as (const);`).
/// `crates/parser/src/js/expressions.rs`'s `make_as_node` builds an
/// `AsConstExpression` for `x as const` and a plain `AsExpression` for
/// everything else — including `x as (const)`, because its test requires
/// `right.metadata().parens.get() == 0`. So the parenthesized spelling is the
/// *only* one that yields an `AsExpression` over a `GenericTypeAnnotation`
/// named `const`, and dropping the parens on regeneration silently turns the
/// node into an `AsConstExpression`. This mirrors `make_as_node`'s condition
/// exactly (no type parameters, `id` an `Identifier` named `const` that is
/// neither optional nor annotated); the `parens` disjunct is the one part not
/// mirrored, since this crate never preserves source parens.
fn is_as_const_shape<'gc>(ctx: &GCLock<'_, '_>, type_annotation: &'gc Node<'gc>) -> bool {
    let Node::GenericTypeAnnotation(generic) = type_annotation else {
        return false;
    };
    if generic.type_parameters.is_some() {
        return false;
    }
    let Node::Identifier(ident) = generic.id else {
        return false;
    };
    ctx.bytes_str_lossy(ident.name.get()) == "const"
        && !ident.optional.get()
        && ident.type_annotation.is_none()
}
