/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Flow type annotations: the primitive keyword types, `StringLiteralTypeAnnotation`
//! through `VoidTypeAnnotation`, `FunctionTypeAnnotation`/`FunctionTypeParam`,
//! `NullableTypeAnnotation`, `QualifiedTypeIdentifier`, `TypeofTypeAnnotation`,
//! `TupleTypeAnnotation`, `ArrayTypeAnnotation`, `UnionTypeAnnotation`,
//! `IntersectionTypeAnnotation`, `GenericTypeAnnotation`, `IndexedAccessType`,
//! `OptionalIndexedAccessType`, `InterfaceTypeAnnotation`.
//!
//! Ported from juno `gen_js.rs:2161-2430`. This is the plan's Task 10.
//! `get_precedence`'s entries for these kinds (`gen_js.rs:3661-3684`,
//! `UNION_TYPE`/`INTERSECTION_TYPE`) were verified already landed in
//! `precedence.rs` by Task 3 (`precedence.rs:707-723`) before this task
//! started; they did, so nothing here re-ports them.
//!
//! # A tightly-coupled dependency ported early, not deferred
//!
//! **`TypeParameterDeclaration` / `TypeParameterInstantiation`
//! (`gen_js.rs:3026-3041`, one shared juno arm) are ported here too**, even
//! though that line range sits past this task's own 2161-2430 citation and
//! neither kind is named in this task's brief. They are ported here because
//! two of *this task's own* headline kinds are non-functional without them:
//! `GenericTypeAnnotation::type_parameters` (`Array<string>`'s `<string>`)
//! and `FunctionTypeAnnotation::type_parameters` (a generic function type's
//! `<T>`) both hold one of these two kinds whenever present, and juno prints
//! both through the identical shared arm regardless of which one it is (its
//! own `Node::TypeParameterDeclaration(..) | Node::TypeParameterInstantiation(..)`
//! match). Leaving it out would make `GenericTypeAnnotation` — explicitly
//! this task's own — unable to print a single instantiated generic type,
//! the single most common Flow type shape there is (confirmed empirically:
//! every attempt at an `Array<...>`-shaped round-trip test errored with
//! `UnsupportedKind(TypeParameterInstantiation)` until this was added).
//!
//! [`GenJS::gen_type_parameter_list`] is `pub(crate)`, shared by both
//! kinds' dispatch arms via one method (juno's own single shared match arm
//! collapses identically). **This task does *not* port `TypeParameter`**
//! (`gen_js.rs:3042-3057`, each `TypeParameterDeclaration` list *element*'s
//! own kind, e.g. `T` or `+T: Bound = Default`) — that stays Task 11's,
//! explicitly named in its brief's "Produces" list alongside
//! `TypeParameterDeclaration` itself. A `TypeParameterDeclaration` that
//! actually has type parameters (as opposed to the untested, currently
//! unreachable-from-real-syntax `<>` empty case) therefore still reports
//! `UnsupportedKind(TypeParameter)` for each element until Task 11 lands;
//! `TypeParameterInstantiation` has no such gap, since its own list elements
//! are ordinary types, not `TypeParameter` nodes, and are fully covered by
//! this task's own kinds. Whoever implements Task 11 will find
//! `TypeParameterDeclaration`/`TypeParameterInstantiation` already present
//! in `dispatch.rs` under this comment — do not re-add them.
//!
//! `FunctionTypeAnnotation` itself does not reimplement its `(params)`
//! printing: it delegates to [`GenJS::visit_func_type_params`]
//! (`arms/func.rs`, ported early by Task 7 for exactly this reason — see
//! that module's doc comment, corrected below). This task is that helper's
//! first real caller.
//!
//! # Adaptations specific to this module
//!
//! **`TupleTypeAnnotation` has grown an `inexact: bool` field since juno was
//! frozen.** juno's version (`juno_ast/src/def.rs`) has only `types:
//! NodeList`; ours (`crates/ast/src/node.rs`) has `element_types: NodeList`
//! (the renamed field) plus `inexact: Cell<bool>`, matching the current C++
//! `ESTree.def` (`include/hermes/AST/ESTree.def:938-941`) and confirmed
//! against our own parser's `parseTupleTypeAnnotationFlow`
//! (`lib/Parser/JSParserImpl-flow.cpp:3683-3726`): a trailing bare `...`
//! before the closing `]` (with a comma first if the tuple already has
//! elements — `[a, b, ...]`, not `[a, b...]`) marks the tuple "inexact"
//! (there may be more elements than listed). This arm reprints exactly that
//! shape: `,`-separated `element_types`, then (only if `inexact`) a comma
//! (only if `element_types` was non-empty) followed by `...`, all inside the
//! brackets.
//!
//! **`TypeofTypeAnnotation` has grown a `type_arguments: Option<&Node>`
//! field since juno was frozen.** juno's version has only `argument`; ours
//! (matching current `ESTree.def:922-926`, `ESTREE_IGNORE_IF_EMPTY`) adds
//! Flow's `typeof x<T>` type-argument syntax, confirmed against our own
//! parser's `parseTypeofTypeAnnotationFlow`
//! (`lib/Parser/JSParserImpl-flow.cpp:3619-3675`, the `parseTypeArgsFlow`
//! call building a `TypeParameterInstantiation` — the same kind
//! `GenericTypeAnnotation` prints its own `<...>` through). This arm prints
//! it the same way:
//! `argument`, then `type_arguments` (via plain `gen_node`, no unwrapping)
//! when present.
//!
//! **`raw` on the three literal *type* annotations is load-bearing: it is
//! printed verbatim.** `StringLiteralTypeAnnotation`,
//! `NumberLiteralTypeAnnotation`, and `BigIntLiteralTypeAnnotation` each
//! carry both a decoded `value` and the verbatim source text in `raw`
//! (`ESTree.def:860-870`). Unlike a `NumericLiteral`'s `raw` — which the
//! dumper emits only under `-include-raw-ast-prop` and which every
//! round-trip harness therefore normalizes away — **these `raw` fields are
//! unconditional ESTree properties**: they appear in every `-dump-ast`
//! output (`crates/ast/src/dump.rs`'s generated field emitters; verified
//! with `ast-dump --parse-flow` on `type A = 0x10;`).
//!
//! So printing `value` instead of `raw` is a real round-trip corruption, not
//! a cosmetic choice. Measured, before this was fixed in Task 15:
//!
//! | source | regenerated | `raw` before → after |
//! |---|---|---|
//! | `type A = 0x10;` | `type A = 16;` | `"0x10"` → `"16"` |
//! | `type A = 1e3;` | `type A = 1000;` | `"1e3"` → `"1000"` |
//! | `type A = 1_0;` | `type A = 10;` | `"1_0"` → `"10"` |
//! | `type A = "foo";` | `type A = 'foo';` | `"\"foo\""` → `"'foo'"` |
//!
//! juno has the same defect in a milder form: its `NumberLiteral`
//! (`gen_js.rs:2187-2193`) and `BooleanLiteral` (`:2201-2207`) type arms
//! print `value`, and although its `StringLiteralTypeAnnotation` arm
//! (`:2176-2186`) does read `raw`, it reads only the *quote character* off
//! it and re-escapes `value`, so `type A = "aA";` still loses its
//! `raw`. An earlier revision of this crate went further in the wrong
//! direction — it printed the `Opt`-configured [`QuoteChar`](crate::QuoteChar)
//! and claimed in this comment that the choice "does not affect round-trip
//! correctness either way, `'a'` and `"a"` parse to the identical
//! `StringLiteralTypeAnnotation`". That claim was false, and the ported juno
//! case `test_roundtrip_flow("type A = \"foo\"")` is what falsified it.
//!
//! All three now print `raw` through `ctx.try_bytes_str` and
//! `write_utf8`, exactly as `BigIntLiteralTypeAnnotation` already did. The
//! cost is that `Opt::quote` no longer applies to a string literal *type*
//! (it still applies to every string literal *value*); that is the correct
//! trade, because a node that records its own source spelling is asking to
//! be reproduced with it.
//!
//! **`BooleanLiteralTypeAnnotation`'s `raw` stays unused, matching juno.**
//! It is the one of the four where `value` and `raw` cannot disagree: `raw`
//! can only ever be the token text `true` or `false`, which is character for
//! character what printing `value` produces. Confirmed by round trip
//! (`type A = true;` reparses to an identical dump, `raw` included).
//! Plan constraint 4 forbids `..`, so this arm names `raw: _` explicitly
//! instead — same substitution `arms/literal.rs`'s `gen_bigint_literal`
//! already made for `BigIntLiteral`'s own ignored fields elsewhere.
//!
//! **`BigIntLiteralTypeAnnotation`'s `raw` is printed verbatim, with no
//! juno-style missing-`n`-suffix bug to fix.** Unlike `BigIntLiteral::bigint`
//! (`arms/literal.rs`'s `gen_bigint_literal`, ported from
//! `gen_js.rs:842-848`) — whose value is the ESTree `bigint` property, the
//! literal's digits *without* the trailing `n`, so juno's bare
//! `self.write_utf8(ctx.str(*bigint))` there drops the suffix and is a real
//! round-trip bug this crate fixes — `BigIntLiteralTypeAnnotation::raw` is a
//! different property entirely: confirmed against our own parser
//! (`crates/parser/src/token.rs:199-204`, `get_bigint_literal_raw_value`'s
//! own doc comment: "the raw source text of a `bigint_literal` token,
//! including any radix prefix and the trailing `n`") and the C++ lexer's
//! matching `getBigIntLiteralRawValue()`/`getBigIntLiteral()` split
//! (`include/hermes/Parser/JSLexer.h:195-204`), `raw` already spells `123n`,
//! not `123`. juno's bare `self.write_utf8(ctx.str(*raw))` is therefore
//! correct as written; this arm ports it unchanged (beyond the standing
//! `try_bytes_str` substitution for our WTF-8 atoms — encoding rules, not a
//! correctness fix) rather than appending a second, duplicate `n`.
//!
//! **`Identifier`/label-shaped fields go through `ctx.try_bytes_str`, never
//! `ctx.str`/`gc.bytes()`/`bytes_str_lossy`.** `BigIntLiteralTypeAnnotation::raw`
//! is the one such field in this module (a `NodeLabel`, the same interned-atom
//! type as `NodeString` — `crates/ast/src/node_child.rs:12-16` — so the
//! standing encoding rule applies identically); every other kind here either
//! has no string-shaped field at all or already routes through
//! [`GenJS::print_escaped_string_literal`]/`hermes_support::json_emitter::number_to_string`.
//!
//! # A test-coverage gap this task does not fill, and why
//!
//! **`typeof x.y` (a *qualified* `typeof`) is not round-trip tested here.**
//! Its `argument` for a dotted path is a `QualifiedTypeofIdentifier`
//! (confirmed against our own parser's `parseTypeofTypeAnnotationFlow`,
//! `lib/Parser/JSParserImpl-flow.cpp:3636-3653` — a *different* kind from
//! `QualifiedTypeIdentifier`, this task's own, used for `GenericTypeAnnotation`'s
//! dotted `id`s like `A.B<T>`), and `QualifiedTypeofIdentifier` has no
//! dispatch arm: it is explicitly Task 12's ("the remaining type kinds").
//! Adding it here would duplicate work Task 12 is already scoped to do and
//! risk a duplicate `match` arm when it lands. This task's `typeof`
//! round-trip test uses the un-dotted `typeof x` instead — `argument` is
//! then a plain `Identifier`, already fully supported (Task 4) — which
//! still exercises `TypeofTypeAnnotation`'s own arm completely; only the
//! *argument's* kind differs.
//!
//! **UPDATE (Task 11): the round-trip gap below is closed.** Through Task
//! 10, no kind in this file could be round-trip tested through a full
//! `generate()` call — `Identifier::type_annotation` (`var x: T`'s `: T`)
//! and a `function` declaration/expression's own `returnType` are both
//! wrapped in a `TypeAnnotation` node (confirmed against
//! `crates/parser/src/js/flow/function_types.rs`'s
//! `parse_return_type_annotation_flow`, `wrapped_start: Some(..)`,
//! `crates/parser/src/js/functions.rs:181-184`), and `TypeAnnotation` had no
//! dispatch arm until Task 11 (`arms/flow_decl.rs`) landed it — so every real
//! top-level entry point into a bare Flow type used to error
//! `UnsupportedKind(TypeAnnotation)` through `generate()`. This task's own
//! `#[cfg(test)]` module therefore used to carry a hand-rolled unwrap/
//! re-embed workaround (`with_return_flow_type`/`round_trip_return_flow_type`)
//! duplicating what `tests/roundtrip.rs`'s ordinary `round_trip`/`gen`
//! helpers already do for every other task, purely because `generate()`
//! itself couldn't reach these kinds yet.
//!
//! Now that `TypeAnnotation` has an arm, every test that used that
//! workaround has been moved to `tests/roundtrip.rs` (its own "Task 10
//! migration" section) and drives the real `generate()` entry point like
//! everything else in that file, with `ParseFlags { parse_flow: true, .. }`.
//! The workaround functions and those tests have been deleted from this
//! module. Two tests remain here, both for a reason unrelated to
//! `TypeAnnotation`'s dispatch gap (see below): they hand-build a node
//! directly because no *grammar* produces that exact shape, not because
//! `generate()` couldn't reach it.
//!
//! **`InterfaceTypeAnnotation`'s two remaining hand-built-tree tests.** A
//! `function f(): T {}` fixture (now reachable end-to-end) still cannot
//! reach a *bodyless* `interface` type: every real parse of
//! `interface ... { ... }` requires a `{ ... }` body (our own parser's
//! `parseInterfaceTailFlow`, `lib/Parser/JSParserImpl-flow.cpp:2135-2153`,
//! unconditionally requires `l_brace`). `interface_type_annotation_with_no_extends_or_body_prints_bare_keyword`
//! (below) therefore still hand-builds the node, the same technique
//! `arms/literal.rs`'s `directive_literal_escapes_like_string_literal` test
//! uses for the identical reason. Likewise,
//! `interface_type_annotation_in_postfix_position_prints_without_redundant_parens`
//! (this module's last test) hand-builds an `InterfaceTypeAnnotation` as an
//! `ArrayTypeAnnotation`'s `element_type` purely to isolate the
//! `get_precedence` question ("does `print_child` add a redundant wrap") from
//! how the body was built — a real `interface {}[]` fixture would now also
//! work (Task 11 supplied `ObjectTypeAnnotation`/`InterfaceExtends`), but the
//! hand-built version stays since it already covers the question this test
//! asks with no extra machinery.

use hermes_ast::context::GCLock;
use hermes_ast::node::{
    ArrayTypeAnnotation, BigIntLiteralTypeAnnotation, BooleanLiteralTypeAnnotation,
    FunctionTypeAnnotation, FunctionTypeParam, GenericTypeAnnotation, IndexedAccessType,
    InterfaceTypeAnnotation, IntersectionTypeAnnotation, Node, NodeField, NullableTypeAnnotation,
    NumberLiteralTypeAnnotation, OptionalIndexedAccessType, QualifiedTypeIdentifier,
    StringLiteralTypeAnnotation, TupleTypeAnnotation, TypeofTypeAnnotation, UnionTypeAnnotation,
};
use hermes_ast::node_child::NodeList;
use hermes_ast::visitor::Path;

use crate::precedence::{ChildPos, ForceSpace};
use crate::{out, GenJS, GenJsError, Pretty};

impl<'s, 'w> GenJS<'s, 'w> {
    /// `ExistsTypeAnnotation`: the deprecated `*` ("exists") type.
    ///
    /// juno `gen_js.rs:2161-2163`. No fields besides `metadata`.
    pub(crate) fn gen_exists_type_annotation(&mut self) -> Result<(), GenJsError> {
        out!(self, "*");
        Ok(())
    }

    /// `EmptyTypeAnnotation`: `empty`.
    ///
    /// juno `gen_js.rs:2164-2166`. No fields besides `metadata`.
    pub(crate) fn gen_empty_type_annotation(&mut self) -> Result<(), GenJsError> {
        out!(self, "empty");
        Ok(())
    }

    /// `StringTypeAnnotation`: `string`.
    ///
    /// juno `gen_js.rs:2167-2169`. No fields besides `metadata`.
    pub(crate) fn gen_string_type_annotation(&mut self) -> Result<(), GenJsError> {
        out!(self, "string");
        Ok(())
    }

    /// `BigIntTypeAnnotation`: `bigint`.
    ///
    /// juno `gen_js.rs:2170-2172`. No fields besides `metadata`.
    pub(crate) fn gen_bigint_type_annotation(&mut self) -> Result<(), GenJsError> {
        out!(self, "bigint");
        Ok(())
    }

    /// `NumberTypeAnnotation`: `number`.
    ///
    /// juno `gen_js.rs:2173-2175`. No fields besides `metadata`.
    pub(crate) fn gen_number_type_annotation(&mut self) -> Result<(), GenJsError> {
        out!(self, "number");
        Ok(())
    }

    /// `StringLiteralTypeAnnotation`: a quoted, escaped string used as a
    /// type (e.g. the `'a'` in `type T = 'a' | 'b'`).
    ///
    /// juno `gen_js.rs:2176-2186`. Prints `raw` verbatim — see the module
    /// doc comment's "`raw` on the three literal *type* annotations is
    /// load-bearing" section.
    pub(crate) fn gen_string_literal_type_annotation(
        &mut self,
        ctx: &GCLock<'_, '_>,
        inner: &StringLiteralTypeAnnotation<'_>,
    ) -> Result<(), GenJsError> {
        let StringLiteralTypeAnnotation {
            metadata: _,
            value: _,
            raw,
        } = inner;
        let s = ctx
            .try_bytes_str(raw.get())
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        self.write_utf8(s);
        Ok(())
    }

    /// `NumberLiteralTypeAnnotation`: a number used as a type (e.g. the `42`
    /// in `type T = 42 | 43`).
    ///
    /// juno `gen_js.rs:2187-2193` prints `value` and ignores `raw`; this
    /// prints `raw` verbatim — see the module doc comment's "`raw` on the
    /// three literal *type* annotations is load-bearing" section.
    pub(crate) fn gen_number_literal_type_annotation(
        &mut self,
        ctx: &GCLock<'_, '_>,
        inner: &NumberLiteralTypeAnnotation<'_>,
    ) -> Result<(), GenJsError> {
        let NumberLiteralTypeAnnotation {
            metadata: _,
            value: _,
            raw,
        } = inner;
        let s = ctx
            .try_bytes_str(raw.get())
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        self.write_utf8(s);
        Ok(())
    }

    /// `BigIntLiteralTypeAnnotation`: a BigInt used as a type (e.g. the
    /// `123n` in `type T = 123n`).
    ///
    /// juno `gen_js.rs:2194-2197`. `raw` already includes the `n` suffix —
    /// see the module doc comment's "no juno-style missing-`n`-suffix bug"
    /// section — so, unlike `arms/literal.rs`'s `gen_bigint_literal`, no `n`
    /// is appended here.
    pub(crate) fn gen_bigint_literal_type_annotation(
        &mut self,
        ctx: &GCLock<'_, '_>,
        inner: &BigIntLiteralTypeAnnotation<'_>,
    ) -> Result<(), GenJsError> {
        let BigIntLiteralTypeAnnotation { metadata: _, raw } = inner;
        let s = ctx
            .try_bytes_str(raw.get())
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        self.write_utf8(s);
        Ok(())
    }

    /// `BooleanTypeAnnotation`: `boolean`.
    ///
    /// juno `gen_js.rs:2198-2200`. No fields besides `metadata`.
    pub(crate) fn gen_boolean_type_annotation(&mut self) -> Result<(), GenJsError> {
        out!(self, "boolean");
        Ok(())
    }

    /// `BooleanLiteralTypeAnnotation`: `true`/`false` used as a type.
    ///
    /// juno `gen_js.rs:2201-2207`. `raw` goes unused, matching juno — see
    /// the module doc comment.
    pub(crate) fn gen_boolean_literal_type_annotation(
        &mut self,
        inner: &BooleanLiteralTypeAnnotation<'_>,
    ) -> Result<(), GenJsError> {
        let BooleanLiteralTypeAnnotation {
            metadata: _,
            value,
            raw: _,
        } = inner;
        out!(self, "{}", if value.get() { "true" } else { "false" });
        Ok(())
    }

    /// `NullLiteralTypeAnnotation`: `null` used as a type.
    ///
    /// juno `gen_js.rs:2208-2210`. No fields besides `metadata`.
    pub(crate) fn gen_null_literal_type_annotation(&mut self) -> Result<(), GenJsError> {
        out!(self, "null");
        Ok(())
    }

    /// `SymbolTypeAnnotation`: `symbol`.
    ///
    /// juno `gen_js.rs:2211-2213`. No fields besides `metadata`.
    pub(crate) fn gen_symbol_type_annotation(&mut self) -> Result<(), GenJsError> {
        out!(self, "symbol");
        Ok(())
    }

    /// `AnyTypeAnnotation`: `any`.
    ///
    /// juno `gen_js.rs:2214-2216`. No fields besides `metadata`.
    pub(crate) fn gen_any_type_annotation(&mut self) -> Result<(), GenJsError> {
        out!(self, "any");
        Ok(())
    }

    /// `MixedTypeAnnotation`: `mixed`.
    ///
    /// juno `gen_js.rs:2217-2219`. No fields besides `metadata`.
    pub(crate) fn gen_mixed_type_annotation(&mut self) -> Result<(), GenJsError> {
        out!(self, "mixed");
        Ok(())
    }

    /// `VoidTypeAnnotation`: `void`.
    ///
    /// juno `gen_js.rs:2220-2222`. No fields besides `metadata`.
    pub(crate) fn gen_void_type_annotation(&mut self) -> Result<(), GenJsError> {
        out!(self, "void");
        Ok(())
    }

    /// `FunctionTypeAnnotation`: `<T>(this: This, a: A, ...rest: R) => Ret`.
    ///
    /// juno `gen_js.rs:2223-2266`. Delegates its `(params)` printing to
    /// [`GenJS::visit_func_type_params`] (`arms/func.rs`) rather than
    /// reimplementing juno's inline duplicate of that same logic — see the
    /// module doc comment's "tightly-coupled dependency" section.
    pub(crate) fn gen_function_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &FunctionTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let FunctionTypeAnnotation {
            metadata: _,
            params,
            this,
            return_type,
            rest,
            type_parameters,
        } = inner;
        self.visit_func_type_params(ctx, *params, *this, *rest, *type_parameters, node)?;
        if self.pretty() == Pretty::Yes {
            out!(self, " => ");
        } else {
            self.space_before_equals("=>");
            out!(self, "=>");
        }
        self.gen_node(
            ctx,
            return_type,
            Some(Path::new(node, NodeField::return_type)),
        )?;
        Ok(())
    }

    /// `FunctionTypeParam`: `name?: type`, or a bare `type` when unnamed
    /// (e.g. inside a `TupleTypeAnnotation`-adjacent rest position — in
    /// practice always named for an ordinary `FunctionTypeAnnotation` list
    /// element, but `name` is `Option` at the type level).
    ///
    /// juno `gen_js.rs:2283-2298`. This is the *general*-case printer for a
    /// `FunctionTypeParam` reached as an ordinary list element through
    /// `gen_node`; `FunctionTypeAnnotation`'s `this` parameter is handled by
    /// a separate, special-cased branch inside
    /// [`GenJS::visit_func_type_params`] instead (juno's own arm duplicates
    /// this same match once per caller — `gen_js.rs:2242-2255` inline here,
    /// `visit_func_type_params`'s own copy at `gen_js.rs:3417-3430` — rather
    /// than sharing it).
    pub(crate) fn gen_function_type_param<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &FunctionTypeParam<'gc>,
    ) -> Result<(), GenJsError> {
        let FunctionTypeParam {
            metadata: _,
            name,
            type_annotation,
            optional,
        } = inner;
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
        )?;
        Ok(())
    }

    /// `NullableTypeAnnotation`: `?type`, parenthesizing `type` when needed
    /// (e.g. `?(a | b)`, since `?a | b` would parse as `(?a) | b` instead —
    /// `NullableTypeAnnotation` is `UNARY` precedence, tighter than
    /// `UnionTypeAnnotation`'s `UNION_TYPE`/`IntersectionTypeAnnotation`'s
    /// `INTERSECTION_TYPE`, both ported by Task 3; see `precedence.rs:707-723`).
    ///
    /// juno `gen_js.rs:2299-2310`.
    pub(crate) fn gen_nullable_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &NullableTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let NullableTypeAnnotation {
            metadata: _,
            type_annotation,
        } = inner;
        out!(self, "?");
        self.print_child(
            ctx,
            Some(*type_annotation),
            Path::new(node, NodeField::type_annotation),
            ChildPos::Right,
        )?;
        Ok(())
    }

    /// `QualifiedTypeIdentifier`: `qualification.id` (e.g. the `A.B` in
    /// `A.B<T>`).
    ///
    /// juno `gen_js.rs:2311-2319`.
    pub(crate) fn gen_qualified_type_identifier<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &QualifiedTypeIdentifier<'gc>,
    ) -> Result<(), GenJsError> {
        let QualifiedTypeIdentifier {
            metadata: _,
            qualification,
            id,
        } = inner;
        self.gen_node(
            ctx,
            qualification,
            Some(Path::new(node, NodeField::qualification)),
        )?;
        out!(self, ".");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        Ok(())
    }

    /// `TypeofTypeAnnotation`: `typeof argument`, plus an optional
    /// `<type_arguments>` (Flow's `typeof x<T>`).
    ///
    /// juno `gen_js.rs:2320-2326`. `type_arguments` has no juno counterpart
    /// — see the module doc comment's "grown a `type_arguments` field"
    /// section.
    pub(crate) fn gen_typeof_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TypeofTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let TypeofTypeAnnotation {
            metadata: _,
            argument,
            type_arguments,
        } = inner;
        out!(self, "typeof ");
        self.gen_node(ctx, argument, Some(Path::new(node, NodeField::argument)))?;
        if let Some(type_arguments) = type_arguments {
            self.gen_node(
                ctx,
                type_arguments,
                Some(Path::new(node, NodeField::type_arguments)),
            )?;
        }
        Ok(())
    }

    /// `TupleTypeAnnotation`: `[a, b, ...c]`-shaped elements, plus an
    /// optional trailing bare `...` marking the tuple inexact.
    ///
    /// juno `gen_js.rs:2327-2336`. `element_types`/`inexact` have no juno
    /// counterpart (juno's field is plain `types`, with no inexact concept
    /// at all) — see the module doc comment's "grown an `inexact` field"
    /// section for the printed shape and its evidence.
    pub(crate) fn gen_tuple_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TupleTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let TupleTypeAnnotation {
            metadata: _,
            element_types,
            inexact,
        } = inner;
        out!(self, "[");
        for (i, ty) in element_types.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.gen_node(ctx, ty, Some(Path::new(node, NodeField::element_types)))?;
        }
        if inexact.get() {
            if !element_types.is_empty() {
                self.comma();
            }
            out!(self, "...");
        }
        out!(self, "]");
        Ok(())
    }

    /// `ArrayTypeAnnotation`: `element_type[]`, parenthesizing `element_type`
    /// when it is a lower-precedence type.
    ///
    /// juno `gen_js.rs:2337-2343`: a bare `element_type.visit(...)`, never
    /// `print_child`. **DEVIATION from juno — a correctness fix, not a
    /// transcription: this is a real round-trip corruption bug, found in
    /// task-10 review round 2, not merely the redundant-parens cosmetic gap
    /// documented on `GenericTypeAnnotation` and friends.** `(?a)[]`
    /// legally parses to `ArrayTypeAnnotation{element_type:
    /// NullableTypeAnnotation(a)}` — Flow's `( Type )` grouping returns the
    /// inner type unwrapped, with no wrapper node (see `precedence.rs`'s
    /// new `ArrayTypeAnnotation`/`IndexedAccessType`/
    /// `OptionalIndexedAccessType` `get_precedence` entry for the full
    /// parser trace) — so printing `element_type` bare loses the
    /// parenthesization: it comes back out as `?a[]`, which reparses as
    /// `NullableTypeAnnotation(ArrayTypeAnnotation(a))` — a different type,
    /// not a formatting difference. juno has the identical bug
    /// (`gen_js.rs:2337-2343`). Routed through `print_child` now, the same
    /// way `IndexedAccessType`/`OptionalIndexedAccessType` are below, so
    /// `need_parens` can catch this; regression test:
    /// `array_of_parenthesized_nullable_round_trips_preserving_structure`.
    ///
    /// juno `gen_js.rs:2337-2343`.
    pub(crate) fn gen_array_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ArrayTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let ArrayTypeAnnotation {
            metadata: _,
            element_type,
        } = inner;
        self.print_child(
            ctx,
            Some(*element_type),
            Path::new(node, NodeField::element_type),
            ChildPos::Left,
        )?;
        out!(self, "[]");
        Ok(())
    }

    /// `UnionTypeAnnotation`: `a | b | c`, parenthesizing any member whose
    /// own precedence is lower (e.g. a bare `FunctionTypeAnnotation` member,
    /// which `get_precedence` has no entry for and so falls to
    /// `ALWAYS_PAREN` — `precedence.rs`'s `need_parens` — printing
    /// `(() => void) | number`, not the invalid-looking-but-actually-just-
    /// wrong-without-parens `() => void | number`).
    ///
    /// juno `gen_js.rs:2344-2358`.
    pub(crate) fn gen_union_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &UnionTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let UnionTypeAnnotation { metadata: _, types } = inner;
        for (i, ty) in types.iter().enumerate() {
            if i > 0 {
                self.space(ForceSpace::No);
                out!(self, "|");
                self.space(ForceSpace::No);
            }
            self.print_child(
                ctx,
                Some(ty),
                Path::new(node, NodeField::types),
                ChildPos::Anywhere,
            )?;
        }
        Ok(())
    }

    /// `IntersectionTypeAnnotation`: `a & b & c`, parenthesizing any member
    /// whose own precedence is lower, the same way `UnionTypeAnnotation`
    /// does for `|`.
    ///
    /// juno `gen_js.rs:2359-2373`.
    pub(crate) fn gen_intersection_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &IntersectionTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let IntersectionTypeAnnotation { metadata: _, types } = inner;
        for (i, ty) in types.iter().enumerate() {
            if i > 0 {
                self.space(ForceSpace::No);
                out!(self, "&");
                self.space(ForceSpace::No);
            }
            self.print_child(
                ctx,
                Some(ty),
                Path::new(node, NodeField::types),
                ChildPos::Anywhere,
            )?;
        }
        Ok(())
    }

    /// `GenericTypeAnnotation`: `id`, or `id<type_parameters>` when
    /// instantiated (e.g. `Array<string>`).
    ///
    /// juno `gen_js.rs:2374-2387`.
    pub(crate) fn gen_generic_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &GenericTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let GenericTypeAnnotation {
            metadata: _,
            id,
            type_parameters,
        } = inner;
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        Ok(())
    }

    /// `IndexedAccessType`: `object_type[index_type]` (e.g. `A['b']`),
    /// parenthesizing `object_type` when it is a lower-precedence type.
    /// `index_type` is never parenthesized: brackets already delimit it, so
    /// it goes through plain `gen_node` (matching juno) regardless of its
    /// own precedence — the grammar reparses whatever it prints back to the
    /// identical type either way (`parseTypeAnnotationFlow()` inside the
    /// brackets, `lib/Parser/JSParserImpl-flow.cpp:3292`).
    ///
    /// **`object_type` is a real round-trip corruption bug, not the
    /// "cannot arise from real parsing" claim an earlier draft of this
    /// comment made — that claim was false and is corrected here (task-10
    /// review round 2).** The grammar does only ever *build* `object_type`
    /// from another postfix-tier type via left recursion
    /// (`parsePostfixTypeAnnotationFlow:3249-3301`) — but a literal
    /// `(LowerPrecedenceType)` grouping is one of the things postfix-tier
    /// parsing accepts: `parsePrimaryTypeAnnotationFlow`'s `l_paren` case
    /// delegates to `parseFunctionOrGroupTypeAnnotationFlow`, whose
    /// non-function branch returns the parenthesized inner type *unwrapped*
    /// (`type->incParens(); return type;`, `:4028-4030` — Flow's `( Type )`
    /// has no wrapper node at all). So `(?a)['b']` legally parses to
    /// `IndexedAccessType{object_type: NullableTypeAnnotation(a), index_type:
    /// 'b'}` — structurally identical to what `?a['b']` would parse to, if
    /// that were even the same grouping (it isn't: `?a['b']` actually
    /// parses as `NullableTypeAnnotation(IndexedAccessType(a, 'b'))`, a
    /// different tree entirely, since `?` recurses into
    /// `parsePrefixTypeAnnotationFlow` which itself consumes the whole
    /// postfix chain). Printing `object_type` bare therefore silently
    /// reparses into the wrong structure — not a panic, not invalid syntax,
    /// just a different program. juno has the identical bug
    /// (`gen_js.rs:2388-2397`, a bare `object_type.visit(...)`, never
    /// `print_child`). Routed through `print_child` now (see
    /// `precedence.rs`'s new `get_precedence` entry for these three postfix
    /// kinds); regression test:
    /// `indexed_access_of_parenthesized_nullable_round_trips_preserving_structure`.
    ///
    /// juno `gen_js.rs:2388-2397`.
    pub(crate) fn gen_indexed_access_type<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &IndexedAccessType<'gc>,
    ) -> Result<(), GenJsError> {
        let IndexedAccessType {
            metadata: _,
            object_type,
            index_type,
        } = inner;
        self.print_child(
            ctx,
            Some(*object_type),
            Path::new(node, NodeField::object_type),
            ChildPos::Left,
        )?;
        out!(self, "[");
        self.gen_node(
            ctx,
            index_type,
            Some(Path::new(node, NodeField::index_type)),
        )?;
        out!(self, "]");
        Ok(())
    }

    /// `OptionalIndexedAccessType`: `object_type?.[index_type]` (or
    /// `object_type[index_type]` when `optional` is false but the chain
    /// still contains an earlier `?.` — Flow's `seenOptionalIndexedAccess`
    /// propagation, `lib/Parser/JSParserImpl-flow.cpp:3268-3311`).
    /// `object_type` is parenthesized when it is a lower-precedence type,
    /// the same round-trip corruption fix as `IndexedAccessType`'s sibling
    /// arm above (see that arm's doc comment for the full trace and the
    /// grammar evidence — `(?a)?.['b']` legally parses to
    /// `OptionalIndexedAccessType{object_type: NullableTypeAnnotation(a),
    /// ...}` and needs the parens preserved on regeneration, or it silently
    /// reparses as `NullableTypeAnnotation(OptionalIndexedAccessType(a,
    /// ...))` instead). juno has the identical bug (`gen_js.rs:2398-2408`).
    /// `index_type` stays plain `gen_node`, also matching
    /// `IndexedAccessType`. Regression test:
    /// `optional_indexed_access_of_parenthesized_nullable_round_trips_preserving_structure`.
    ///
    /// juno `gen_js.rs:2398-2408`.
    pub(crate) fn gen_optional_indexed_access_type<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &OptionalIndexedAccessType<'gc>,
    ) -> Result<(), GenJsError> {
        let OptionalIndexedAccessType {
            metadata: _,
            object_type,
            index_type,
            optional,
        } = inner;
        self.print_child(
            ctx,
            Some(*object_type),
            Path::new(node, NodeField::object_type),
            ChildPos::Left,
        )?;
        out!(self, "{}[", if optional.get() { "?." } else { "" });
        self.gen_node(
            ctx,
            index_type,
            Some(Path::new(node, NodeField::index_type)),
        )?;
        out!(self, "]");
        Ok(())
    }

    /// `InterfaceTypeAnnotation`: `interface extends A, B { ... }` used as a
    /// value type (as opposed to an `InterfaceDeclaration` statement).
    ///
    /// juno `gen_js.rs:2409-2430`. See the module doc comment's
    /// "`InterfaceTypeAnnotation` is not round-trip tested" section for why
    /// this task's own tests cannot exercise the `extends`-non-empty/
    /// `body: Some` branches yet.
    pub(crate) fn gen_interface_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &InterfaceTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let InterfaceTypeAnnotation {
            metadata: _,
            extends,
            body,
        } = inner;
        out!(self, "interface");
        if !extends.is_empty() {
            out!(self, " extends ");
            for (i, extend) in extends.iter().enumerate() {
                if i > 0 {
                    self.comma();
                }
                self.gen_node(ctx, extend, Some(Path::new(node, NodeField::extends)))?;
            }
        } else {
            self.space(ForceSpace::No);
        }
        if let Some(body) = body {
            self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))?;
        }
        Ok(())
    }

    /// `<params>`, shared by `TypeParameterDeclaration` (`<T>` in a generic
    /// declaration) and `TypeParameterInstantiation` (`<string>` in
    /// `Array<string>`) — juno prints both through one shared match arm; see
    /// the module doc comment's "tightly-coupled dependency" section for why
    /// this task ports it early.
    ///
    /// juno `gen_js.rs:3026-3041`.
    pub(crate) fn gen_type_parameter_list<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        params: NodeList<'gc>,
    ) -> Result<(), GenJsError> {
        out!(self, "<");
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.gen_node(ctx, param, Some(Path::new(node, NodeField::params)))?;
        }
        out!(self, ">");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use hermes_ast::node::Program;
    use hermes_parser::{parse, ParseFlags};

    use super::*;
    use crate::{Opt, Pretty};

    /// Generate just `node` (not a whole program) and decode the result as a
    /// `String` — the same helper shape `arms/literal.rs`'s tests use, for
    /// the same reason: `InterfaceTypeAnnotation` coverage below cannot go
    /// through a full `generate()` call.
    fn gen_node_to_string<'gc>(gc: &GCLock<'static, '_>, node: &'gc Node<'gc>) -> String {
        // `Opt::default()`'s own default is `Pretty::Yes` (`gen.rs`), so
        // this matches the original `Opt::new()` call it replaces.
        gen_node_to_string_pretty(gc, node, Pretty::Yes)
    }

    /// [`gen_node_to_string`], parameterized on [`Pretty`] — every fixture
    /// in the "one Flow type through a `function` declaration" section below
    /// needs both modes (`for_each_pretty_mode`), unlike the two
    /// hand-built-tree tests above it, which only ever need `Opt`'s default.
    fn gen_node_to_string_pretty<'gc>(
        gc: &GCLock<'static, '_>,
        node: &'gc Node<'gc>,
        pretty: Pretty,
    ) -> String {
        let mut sink = Vec::new();
        {
            let mut gen_js = GenJS::for_test(
                &mut sink,
                Opt {
                    pretty,
                    ..Opt::default()
                },
            );
            gen_js.gen_node(gc, node, None).expect("node generates");
        }
        String::from_utf8(sink).expect("generator output is always valid UTF-8 (spec §5)")
    }

    /// `interface` alone (no `extends`, no `body`) round-trips through
    /// `gen_interface_type_annotation` directly. As the module doc comment's
    /// last section explains, no real Flow source parses to this shape (a
    /// body is always required), so this hand-builds the node the same way
    /// `arms/literal.rs`'s `directive_literal_escapes_like_string_literal`
    /// test hand-builds a bodyless `DirectiveLiteral`. This at least covers
    /// the `extends.is_empty()`/`body: None` branches; the non-empty
    /// branches are Task 11's to add fixtures for.
    #[test]
    fn interface_type_annotation_with_no_extends_or_body_prints_bare_keyword() {
        let mut parsed = parse("0;", ParseFlags::default()).expect("trivial source must parse");
        parsed.with_program(|gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("root is not a Program");
            };
            let stmt = body.iter().next().expect("has a statement");
            let range = stmt.metadata().range.get();
            let hand_built = gc.alloc(Node::InterfaceTypeAnnotation(InterfaceTypeAnnotation::new(
                hermes_ast::node_child::NodeMetadata::new(range),
                NodeList::empty(),
                None,
            )));
            let js = gen_node_to_string(gc, hand_built);
            assert_eq!(js, "interface ");
        });
    }

    /// Pins that [`GenJS::gen_string_literal_type_annotation`] prints `raw`
    /// verbatim rather than re-quoting `value` with [`GenJS::quote`] — the
    /// module doc comment's "`raw` on the three literal *type* annotations
    /// is load-bearing" section.
    ///
    /// The hand-built node's `raw` deliberately disagrees with `value` in
    /// two independent ways: a different quote character than `Opt::new()`'s
    /// default ([`QuoteChar::Single`](crate::QuoteChar), `gen.rs`), and a
    /// `\u0062` escape whose decoded form is the plain `b` in `value`. An
    /// implementation that re-quoted `value` would print `'ab'`; one that
    /// merely borrowed `raw`'s quote character (juno's behavior,
    /// `gen_js.rs:2176-2186`) would print `"ab"`. Only printing `raw`
    /// verbatim yields the expected text, so this test separates all three.
    ///
    /// Parses a *value*-level `StringLiteral` (not a type) purely to reuse
    /// its `metadata` for the hand-built node, the same workaround
    /// `arms/literal.rs`'s `directive_literal_escapes_like_string_literal`
    /// test uses.
    #[test]
    fn string_literal_type_annotation_prints_raw_verbatim() {
        let mut parsed =
            parse(r#"var s = "a";"#, ParseFlags::default()).expect("source must parse");
        parsed.with_program(|gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("root is not a Program");
            };
            let stmt = body.iter().next().expect("has a statement");
            let range = stmt.metadata().range.get();
            let value = gc.atom_bytes(&b"ab"[..]);
            let raw = gc.atom_bytes(&b"\"a\\u0062\""[..]);
            let hand_built = gc.alloc(Node::StringLiteralTypeAnnotation(
                StringLiteralTypeAnnotation::new(
                    hermes_ast::node_child::NodeMetadata::new(range),
                    value,
                    raw,
                ),
            ));
            let js = gen_node_to_string(gc, hand_built);
            assert_eq!(js, r#""a\u0062""#);
        });
    }

    /// `InterfaceTypeAnnotation` as the base of an `ArrayTypeAnnotation`.
    /// Demonstrates `InterfaceTypeAnnotation` needed (and now has) the same
    /// `PRIMARY` fix, using the same hand-built-tree technique
    /// `interface_type_annotation_with_no_extends_or_body_prints_bare_keyword`
    /// above already uses. See the module doc comment's "two remaining
    /// hand-built-tree tests" section for why this stays hand-built even
    /// though a real `interface {}[]` fixture is reachable now (Task 11): the
    /// `get_precedence` question this test asks — does `print_child` add a
    /// redundant wrap — doesn't depend on how the body was built, only on the
    /// kind itself.
    #[test]
    fn interface_type_annotation_in_postfix_position_prints_without_redundant_parens() {
        let mut parsed = parse("0;", ParseFlags::default()).expect("trivial source must parse");
        parsed.with_program(|gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("root is not a Program");
            };
            let stmt = body.iter().next().expect("has a statement");
            let range = stmt.metadata().range.get();
            let interface = gc.alloc(Node::InterfaceTypeAnnotation(InterfaceTypeAnnotation::new(
                hermes_ast::node_child::NodeMetadata::new(range),
                NodeList::empty(),
                None,
            )));
            let array = gc.alloc(Node::ArrayTypeAnnotation(ArrayTypeAnnotation::new(
                hermes_ast::node_child::NodeMetadata::new(range),
                interface,
            )));
            let js = gen_node_to_string_pretty(gc, array, Pretty::No);
            assert_eq!(js, "interface[]");
        });
    }
}
