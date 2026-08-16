/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Flow declarations (`type`, `opaque type`, `interface`, `declare ...`),
//! object types (`{ ... }`) and their five member kinds, and `enum`.
//!
//! Ported from juno `gen_js.rs:2431-3195` (the big shared `match` arm group)
//! plus `gen_js.rs:3454-3524` (`visit_interface`/`visit_enum_body`). This is
//! the plan's Task 11. `TypeParameterDeclaration`/`TypeParameterInstantiation`
//! (`gen_js.rs:3026-3041`) were already ported by Task 10
//! (`arms/flow_type.rs`) — see that module's doc comment — because
//! `GenericTypeAnnotation`/`FunctionTypeAnnotation` needed them; this task
//! does not re-add them. `visit_func_type_params` (`gen_js.rs:3401-3452`) was
//! ported early by Task 7 (`arms/func.rs`); this task is its second caller
//! (Task 10's `FunctionTypeAnnotation` was the first).
//!
//! # `Node::ClassImplements`, not named in this task's brief
//!
//! juno's `InterfaceExtends`/`ClassImplements` share one match arm
//! (`gen_js.rs:2747-2762`, identical field sets: `id`, `type_parameters`).
//! The brief's own "Produces" list names only `InterfaceExtends`, but
//! `ClassImplements` sits in the same cited line range and is exactly what
//! `DeclareClass::implements` (this task's own kind) holds — without it, any
//! `declare class ... implements ...` errors `UnsupportedKind(ClassImplements)`.
//! Ported here too, via the same shared helper juno's shared arm collapses to.
//!
//! # `TASK 11 TODO` obligation from Task 10's review — done
//!
//! Task 10's Flow-type round-trip tests lived inside `arms/flow_type.rs`'s own
//! `#[cfg(test)]` module, using a hand-rolled unwrap/re-embed workaround,
//! because no real Flow type could reach `generate()`'s public entry point
//! until `Node::TypeAnnotation` (this task's own kind) had a dispatch arm.
//! Now that it does, those tests have been moved verbatim (minus the
//! unwrap/re-embed plumbing, which is no longer needed) into
//! `tests/roundtrip.rs`, driven through the crate's ordinary `generate()`-based
//! `round_trip`/`gen` helpers with `ParseFlags { parse_flow: true, .. }`. The
//! workaround functions and the `#[cfg(test)]` module that housed them have
//! been deleted from `arms/flow_type.rs`.
//!
//! # Adaptations: fields our AST grew since juno was frozen
//!
//! **`OpaqueType`/`DeclareOpaqueType` grew `lower_bound`/`upper_bound`
//! fields.** juno's frozen versions have only `supertype` (`opaque type T:
//! Supertype = Impl`). Confirmed against our own parser
//! (`crates/parser/src/js/flow/declarations.rs`'s `parse_type_alias_flow`,
//! C++ `lib/Parser/JSParserImpl-flow.cpp:2018-2039`) and the C++
//! `ESTree.def:999-1006`/`1020-1027`: a newer `opaque type T super Lower
//! extends Upper = Impl` bound syntax (`test/Parser/flow/type-alias.js:97`:
//! `opaque type Counter super empty extends Box<T> = Container<T>;`), with
//! the legacy `: Supertype` form only reachable when *neither* bound was
//! given (the parser makes this structurally exclusive — see
//! `parse_type_alias_flow`'s `if lower_bound.is_none() && upper_bound.is_none()
//! ...`). [`GenJS::gen_opaque_type_bounds`] prints exactly that: ` super
//! Lower`, ` extends Upper`, or (only when both are absent) the legacy `:
//! Supertype`, shared by both kinds since their bound fields are identical.
//!
//! **`Variance` grew a `"writeonly"` spelling.** juno's arm
//! (`gen_js.rs:3012-3025`) only handles `"plus"`/`"minus"`/`"readonly"`,
//! `unimplemented!()`-ing on anything else. Confirmed against our own parser
//! (`crates/parser/src/js/flow/object_types.rs:254-267`,
//! `crates/parser/src/js/flow/types.rs:1178-1191`): `writeonly` fields
//! (`{ writeonly x: T }`) are a real, reachable Flow feature, gated by the
//! same `can_follow_variance_keyword_flow` lookahead as `readonly`. Handled
//! here as its own case (`"writeonly "`, mirroring `"readonly "`'s trailing
//! space) rather than falling into an error.
//!
//! **`Variance` grew `in`/`out` spellings.** Found by the Tier 1 corpus gate
//! (Task 15) on `parser/tests/parser_corpus_flow/type_params.js`, which
//! contains `type F<in T> = T;` and `type G<out T> = T;`: generation failed
//! outright with `UnknownOperator { kind: "Variance", spelling: "in" }`.
//! Flow adopted TypeScript's `in`/`out` spellings for type-parameter
//! variance, and our parser stores the *token spelling* in `Variance::kind`
//! for these (`crates/parser/src/js/flow/params.rs:111-137`, C++
//! `JSParserImpl-flow.cpp:4760-4781` — `tok_->getResWordOrIdentifier()`,
//! not the `plusIdent_`/`minusIdent_` used for `+`/`-`). So the two extra
//! labels are `"in"` and `"out"`, printed with a trailing space like
//! `readonly `/`writeonly ` because a name always follows. Note the parser
//! only builds a `Variance` for them when a name *does* follow: `type H<in>
//! = X;` (same corpus file) makes `in` the type parameter's NAME with no
//! variance node, so this arm is not reached there.
//!
//! **`TypeParameter` grew a `const` field (`r#const`).** juno's frozen
//! version (`juno_ast/src/def.rs:722-727`) has `name`, `bound`, `variance`,
//! `default`, `usesExtendsBound` — no `const` at all, so its arm
//! (`gen_js.rs:3043-3065`) never prints it (and cannot: the field doesn't
//! exist there). Confirmed against our own parser
//! (`crates/parser/src/js/flow/params.rs:82-87`, which consumes a leading
//! `const` keyword *before* the variance sigil) and `test/Parser/flow/
//! function-typeparams.js:105,141` (`<const T>`, `<const +T>`): dropping this
//! field would silently turn a `const` type parameter into a non-`const` one
//! on regeneration — a real round-trip corruption, not merely cosmetic,
//! since `const` type parameters have distinct Flow subtyping behavior.
//! Printed as `"const "` before the variance sigil, matching the parser's own
//! order.
//!
//! **`uses_extends_bound` IS preserved — a deviation from juno, forced by
//! the corpus gate.** juno's arm (`gen_js.rs:3043-3065`) has this field (it
//! is not a post-freeze addition) and ignores it (`uses_extends_bound: _`),
//! always printing the bound as `name: Bound` regardless of whether the
//! source spelled `name: Bound` or `name extends Bound`. This crate followed
//! juno through Task 12, on the reasoning that both spellings build the
//! *identical* `bound` field (`params.rs:158-185`) and that
//! `uses_extends_bound` is only a source-fidelity marker. **That reasoning
//! was wrong**, and Task 15's Tier 1 corpus gate proved it on
//! `parser/tests/parser_corpus_flow/type_params.js`
//! (`type I<T extends U> = T;`): the field is not merely a marker, it is
//! *emitted in the ESTree output* (`ESTree.def:1160-1161`,
//! `crates/ast/src/node.rs:16850`), so rewriting `extends` as `:` flips
//! `usesExtendsBound` from `true` to `false` and the regenerated program
//! dumps a **different AST**. `ESTREE_IGNORE_IF_EMPTY` hides it only when it
//! is `false`, which is exactly the direction the rewrite moves it — the
//! reason the loss looked invisible. Printed here as `" extends "` with
//! mandatory spaces on both sides (a keyword, unlike `:`, cannot abut its
//! neighbours in `Pretty::No`).
//!
//! **`EnumDeclaration`'s `EnumBigIntBody`/`EnumBigIntMember` are explicitly
//! out of scope for this task.** They are real kinds our parser produces
//! (`crates/parser/src/js/flow/declarations.rs`'s `EnumKind::BigInt` arm) with
//! no juno counterpart, but the plan's own Task 12 ("Step 5") names them
//! explicitly as its own — this task's brief says only "`EnumDeclaration` and
//! its bodies/members" without enumerating, and the two BigInt kinds are
//! deliberately left for Task 12 rather than assumed here. An enum with a
//! BigInt member (`enum E { A = 1n }`) still reports `UnsupportedKind` until
//! Task 12 lands.
//!
//! **`ObjectTypeMappedTypeProperty` is explicitly out of scope for this
//! task**, for the identical reason: it is one of `ObjectTypeAnnotation`'s
//! `properties`-list member kinds (`crates/parser/src/js/flow/
//! object_types.rs:763-862`, Flow's mapped-type `[K in T]: V` syntax) with no
//! juno counterpart and no line range in this task's citation — and the plan
//! names it as Task 12's ("Step 5") by name. The brief's "five member kinds"
//! (`ObjectTypeProperty`, `ObjectTypeSpreadProperty`, `ObjectTypeInternalSlot`,
//! `ObjectTypeCallProperty`, `ObjectTypeIndexer`) is exhaustive for this task
//! on purpose; a mapped-type member in `properties` still reports
//! `UnsupportedKind` until Task 12 lands.
//!
//! # Deviations from juno: real round-trip/output bugs, not transcribed
//!
//! **`ObjectTypeProperty` silently drops `get`/`set` on regeneration — a real
//! round-trip corruption, fixed here.** juno's arm (`gen_js.rs:2832-2891`)
//! destructures `kind` only to immediately discard it (`..` after `variance`)
//! and picks its print shape purely from `method`: `method == true` prints a
//! function signature (`key<T>(params): R`), `method == false` prints `key:
//! value` for *any* `value`, including when `value` is itself a
//! `FunctionTypeAnnotation`. Confirmed against our own parser
//! (`crates/parser/src/js/flow/object_types.rs:609-616,713-758`,
//! `parseGetOrSetTypePropertyFlow`): a getter/setter (`{ get foo(): T }`) is
//! parsed with `method: false`, `kind: "get"`/`"set"`, and `value` set to the
//! *same* `FunctionTypeAnnotation` shape a `method: true` property would use.
//! juno's logic therefore prints `{ get foo(): T }` back out as `{foo:() =>
//! T}` — which reparses to `kind: "init"`, `method: false`, a
//! `FunctionTypeAnnotation`-valued plain property — a structurally different
//! tree (a getter, which Flow treats as a read-only accessor, silently
//! becomes an ordinary callable-typed property) from the exact same "prints a
//! child without consulting a field that changes its shape" family the task
//! brief calls out. [`GenJS::gen_object_type_property`] classifies `kind`
//! first (via the new `ObjectTypePropertyKind`, alongside `arms/func.rs`'s
//! `MethodDefinitionKind` precedent) and prints `get `/`set ` + the
//! `(params): R` signature for the accessor cases, only falling to the
//! `method`/plain-`value` branching for `kind == "init"`. Regression test:
//! `object_type_getter_and_setter_round_trip_preserving_kind`.
//!
//! **`DeclareVariable` silently drops both `declare` and the `var`/`let`/
//! `const` keyword when printed with no parent — a real bug, fixed here.**
//! juno's arm (`gen_js.rs:2679-2691`) wraps *both* the `declare ` prefix
//! decision *and* the unconditional `"{} "` kind-keyword print inside a
//! single `if let Some(path) = path { ... }` — so when `path` is `None` (the
//! node is the root the generator was invoked on, or an isolated subtree),
//! neither ever prints, and only the identifier (with its own `: T` type
//! annotation, from `Identifier`'s ordinary arm) comes out. `declare var x:
//! number;` regenerates as `x: number;` — not merely different-looking, but
//! not even the same *kind* of statement (a plain typed-identifier fragment,
//! not a declaration). Every sibling `Declare*` arm in this same juno range
//! (`DeclareOpaqueType`, `DeclareClass`, `DeclareFunction`) correctly treats
//! `path.is_none()` as "print `declare `, this is not inside a `declare
//! export`" — `DeclareVariable` is the one outlier that treats `path.is_none()`
//! as "print nothing at all". [`GenJS::declare_prefix_needed`] applies the
//! same rule uniformly to all four `Declare*` arms; `kind`'s `"{} "` print is
//! now unconditional (it is a required, non-optional field — there is no
//! `declare var`-without-`var`). Regression test:
//! `declare_variable_with_no_parent_still_prints_declare_and_kind`.
//!
//! # A shared helper juno duplicates four times
//!
//! juno repeats the identical `FunctionTypeAnnotation`-as-signature print
//! (`visit_func_type_params` call, then `: return_type`) at four separate
//! call sites: `ObjectTypeProperty`'s `method`/`get`/`set` branches,
//! `ObjectTypeInternalSlot`'s `method` branch, `ObjectTypeCallProperty`
//! (always), and `DeclareFunction` (`gen_js.rs:2223-2266` inline copy in
//! `FunctionTypeAnnotation` aside, the four *this task* owns are at
//! `:2846-2865`, `:2914-2933`, `:2957-2971`, `:2637-2653`). Each site also
//! repeats the identical `unimplemented!("Malformed AST: Need to handle
//! error")` fallback for a non-`FunctionTypeAnnotation` `value`. Factored
//! into one private helper, [`GenJS::gen_methodish_type_signature`], whose
//! fallback is `GenJsError::UnsupportedKind` — the same `unreachable!()` →
//! `GenJsError` substitution `arms/func.rs`'s `MethodDefinition` arm already
//! made for the identical shape (spec §4). This is a structural
//! simplification only (deduplicating four verbatim copies), not a behavior
//! change; see the plan's File Structure note on this crate's deliberate
//! divergence from juno's single giant `match`.

use hermes_ast::context::GCLock;
use hermes_ast::node::{
    ClassImplements, DeclareClass, DeclareExportAllDeclaration, DeclareExportDeclaration,
    DeclareFunction, DeclareInterface, DeclareModule, DeclareModuleExports, DeclareOpaqueType,
    DeclareTypeAlias, DeclareVariable, DeclaredPredicate, EnumBooleanBody, EnumBooleanMember,
    EnumDeclaration, EnumDefaultedMember, EnumNumberBody, EnumNumberMember, EnumStringBody,
    EnumStringMember, EnumSymbolBody, FunctionTypeAnnotation, Identifier, InferredPredicate,
    InterfaceDeclaration, InterfaceExtends, Node, NodeField, ObjectTypeAnnotation,
    ObjectTypeCallProperty, ObjectTypeIndexer, ObjectTypeInternalSlot, ObjectTypeProperty,
    ObjectTypeSpreadProperty, OpaqueType, TypeAlias, TypeAnnotation, TypeCastExpression,
    TypeParameter, Variance,
};
use hermes_ast::node_child::{NodeLabel, NodeList};
use hermes_ast::visitor::Path;

use crate::precedence::{ChildPos, ForceSpace};
use crate::{out, GenJS, GenJsError, Pretty};

// ---------------------------------------------------------------------------
// `ObjectTypePropertyKind`/`VarianceKind`, this module's own operator-shaped
// classifiers — same rationale and pattern as `arms/func.rs`'s
// `MethodDefinitionKind` (lives here rather than `precedence.rs` because
// nothing in `get_precedence`/`need_parens` ever needs to classify these).
// ---------------------------------------------------------------------------

/// `ObjectTypeProperty::kind`, classified from its raw spelling.
///
/// Spellings confirmed against our own parser
/// (`crates/parser/src/js/flow/object_types.rs:670,705,755`:
/// `self.lexer.get_identifier(b"init")` for plain/method properties,
/// `kind: &[u8] = if is_getter { b"get" } else { b"set" }` for accessors).
/// See the module doc comment's "`ObjectTypeProperty` silently drops
/// `get`/`set`" section for why this classification exists at all (juno
/// ignores `kind` entirely).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum ObjectTypePropertyKind {
    /// `init`: an ordinary (non-accessor) property or method.
    Init,
    /// `get`: a getter.
    Get,
    /// `set`: a setter.
    Set,
}

impl ObjectTypePropertyKind {
    /// Classify `label`, the raw contents of an `ObjectTypeProperty`'s `kind`
    /// field.
    ///
    /// # Errors
    /// `Err(GenJsError::UnknownOperator { .. })` if `label`'s spelling is
    /// none of the 3 above — the same reuse rationale `arms/func.rs`'s
    /// `MethodDefinitionKind::from_label` documents.
    fn from_label(gc: &GCLock<'_, '_>, label: NodeLabel) -> Result<Self, GenJsError> {
        Ok(match gc.bytes_str_lossy(label) {
            "init" => Self::Init,
            "get" => Self::Get,
            "set" => Self::Set,
            other => {
                return Err(GenJsError::UnknownOperator {
                    kind: "ObjectTypeProperty",
                    spelling: other.to_string(),
                })
            }
        })
    }
}

/// `Variance::kind`, classified from its raw spelling.
///
/// Spellings confirmed against our own parser (`params.rs:98-107`:
/// `b"plus"`/`b"minus"`; `object_types.rs:254-267`,`types.rs:1178-1191`: the
/// literal `"readonly"`/`"writeonly"` token spelling via
/// `self.lexer.token().get_identifier()`). See the module doc comment's
/// "`Variance` grew a `writeonly` spelling" section.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum VarianceKind {
    /// `+`: covariant.
    Plus,
    /// `-`: contravariant.
    Minus,
    /// `readonly`: read-only (object-type properties/indexers only).
    Readonly,
    /// `writeonly`: write-only (object-type properties/indexers only).
    Writeonly,
    /// `in`: contravariant, TypeScript-style spelling (type parameters only).
    In,
    /// `out`: covariant, TypeScript-style spelling (type parameters only).
    Out,
}

impl VarianceKind {
    /// Classify `label`, the raw contents of a `Variance`'s `kind` field.
    ///
    /// # Errors
    /// `Err(GenJsError::UnknownOperator { .. })` if `label`'s spelling is
    /// none of the 4 above.
    fn from_label(gc: &GCLock<'_, '_>, label: NodeLabel) -> Result<Self, GenJsError> {
        Ok(match gc.bytes_str_lossy(label) {
            "plus" => Self::Plus,
            "minus" => Self::Minus,
            "readonly" => Self::Readonly,
            "writeonly" => Self::Writeonly,
            "in" => Self::In,
            "out" => Self::Out,
            other => {
                return Err(GenJsError::UnknownOperator {
                    kind: "Variance",
                    spelling: other.to_string(),
                })
            }
        })
    }
}

/// Whether a `Declare*` node reached via `path` (the `Option<Path>` its own
/// dispatch call received) must print its own leading `declare ` keyword:
/// true unless `path`'s parent is a `DeclareExportDeclaration`, which already
/// contributed `declare export ` (printing `declare ` again would double it:
/// `declare export declare class ...`). juno's three call sites
/// (`DeclareOpaqueType` `gen_js.rs:2513-2517`, `DeclareClass` `:2529-2536`,
/// `DeclareFunction` `:2618-2621`) each write out this same condition, in two
/// slightly different but logically identical shapes; `DeclareVariable`'s own
/// site (`:2679-2683`) gets it wrong — see the module doc comment's
/// "`DeclareVariable` silently drops" section — so this helper is not a
/// verbatim port of any one of the three, but their shared, corrected logic,
/// applied uniformly to all four `Declare*` arms in this file.
// `pub(crate)`, not private: Task 12's `arms/newer.rs` reuses this for
// `DeclareComponent`/`DeclareHook`/`DeclareEnum`, which are reachable inside
// `declare export` exactly like this file's own four `Declare*` kinds — see
// that module's doc comment.
pub(crate) fn declare_prefix_needed(path: Option<Path<'_>>) -> bool {
    path.is_none_or(|p| !matches!(p.parent, Node::DeclareExportDeclaration(_)))
}

impl<'s, 'w> GenJS<'s, 'w> {
    /// Print a `FunctionTypeAnnotation`-valued object-type member's
    /// `(params): ReturnType` signature. See the module doc comment's "A
    /// shared helper juno duplicates four times" section.
    fn gen_methodish_type_signature<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        value: &'gc Node<'gc>,
    ) -> Result<(), GenJsError> {
        let Node::FunctionTypeAnnotation(FunctionTypeAnnotation {
            metadata: _,
            params,
            this,
            return_type,
            rest,
            type_parameters,
        }) = value
        else {
            return Err(GenJsError::UnsupportedKind(value.kind()));
        };
        self.visit_func_type_params(ctx, *params, *this, *rest, *type_parameters, node)?;
        out!(self, ":");
        self.space(ForceSpace::No);
        self.gen_node(
            ctx,
            return_type,
            Some(Path::new(node, NodeField::return_type)),
        )
    }

    /// Shared printing logic for `TypeAlias`/`DeclareTypeAlias` — identical
    /// field sets, differing only in the leading keyword.
    fn gen_type_alias_like<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        id: &'gc Node<'gc>,
        type_parameters: Option<&'gc Node<'gc>>,
        right: &'gc Node<'gc>,
        declare: bool,
    ) -> Result<(), GenJsError> {
        out!(self, "{}", if declare { "declare type " } else { "type " });
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        if self.pretty() == Pretty::Yes {
            out!(self, " = ");
        } else {
            self.space_before_equals("=");
            out!(self, "=");
        }
        self.gen_node(ctx, right, Some(Path::new(node, NodeField::right)))
    }

    /// `TypeAlias`: `type Id<T> = Right;`.
    ///
    /// juno `gen_js.rs:2431-2461` (shared arm with `DeclareTypeAlias`).
    pub(crate) fn gen_type_alias<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TypeAlias<'gc>,
    ) -> Result<(), GenJsError> {
        let TypeAlias {
            metadata: _,
            id,
            type_parameters,
            right,
        } = inner;
        self.gen_type_alias_like(ctx, node, id, *type_parameters, right, false)
    }

    /// `DeclareTypeAlias`: `declare type Id<T> = Right;`.
    ///
    /// juno `gen_js.rs:2431-2461` (shared arm with `TypeAlias`).
    pub(crate) fn gen_declare_type_alias<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclareTypeAlias<'gc>,
    ) -> Result<(), GenJsError> {
        let DeclareTypeAlias {
            metadata: _,
            id,
            type_parameters,
            right,
        } = inner;
        self.gen_type_alias_like(ctx, node, id, *type_parameters, right, true)
    }

    /// Print an `OpaqueType`/`DeclareOpaqueType`'s bound clause: the current
    /// `super Lower extends Upper` bound syntax, or (only when neither bound
    /// is present) the legacy `: Supertype` syntax — structurally exclusive
    /// by construction (see the module doc comment's "`OpaqueType`/
    /// `DeclareOpaqueType` grew `lower_bound`/`upper_bound`" section). Shared
    /// because both kinds have identical bound fields.
    fn gen_opaque_type_bounds<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        lower_bound: Option<&'gc Node<'gc>>,
        upper_bound: Option<&'gc Node<'gc>>,
        supertype: Option<&'gc Node<'gc>>,
    ) -> Result<(), GenJsError> {
        if let Some(lower_bound) = lower_bound {
            out!(self, " super ");
            self.gen_node(
                ctx,
                lower_bound,
                Some(Path::new(node, NodeField::lower_bound)),
            )?;
        }
        if let Some(upper_bound) = upper_bound {
            out!(self, " extends ");
            self.gen_node(
                ctx,
                upper_bound,
                Some(Path::new(node, NodeField::upper_bound)),
            )?;
        }
        if lower_bound.is_none() && upper_bound.is_none() {
            if let Some(supertype) = supertype {
                out!(self, ":");
                self.space(ForceSpace::No);
                self.gen_node(ctx, supertype, Some(Path::new(node, NodeField::supertype)))?;
            }
        }
        Ok(())
    }

    /// `OpaqueType`: `opaque type Id<T> [super L] [extends U] = Impl;` (or
    /// the legacy `opaque type Id<T>: Supertype = Impl;`).
    ///
    /// juno `gen_js.rs:2463-2489`. See the module doc comment's "grew
    /// `lower_bound`/`upper_bound`" section for the bound clause, absent from
    /// juno's frozen version.
    pub(crate) fn gen_opaque_type<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &OpaqueType<'gc>,
    ) -> Result<(), GenJsError> {
        let OpaqueType {
            metadata: _,
            id,
            type_parameters,
            impltype,
            lower_bound,
            upper_bound,
            supertype,
        } = inner;
        out!(self, "opaque type ");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        self.gen_opaque_type_bounds(ctx, node, *lower_bound, *upper_bound, *supertype)?;
        if self.pretty() == Pretty::Yes {
            out!(self, " = ");
        } else {
            self.space_before_equals("=");
            out!(self, "=");
        }
        self.gen_node(ctx, impltype, Some(Path::new(node, NodeField::impltype)))
    }

    /// Shared printing logic for `InterfaceDeclaration`/`DeclareInterface` —
    /// `decl` is the leading keyword phrase (`"interface"`/`"declare
    /// interface"`).
    ///
    /// juno `gen_js.rs:3454-3483` (`visit_interface`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn visit_interface<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        decl: &str,
        id: &'gc Node<'gc>,
        type_parameters: Option<&'gc Node<'gc>>,
        extends: NodeList<'gc>,
        body: &'gc Node<'gc>,
        node: &'gc Node<'gc>,
    ) -> Result<(), GenJsError> {
        out!(self, "{} ", decl);
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        self.space(ForceSpace::No);
        if !extends.is_empty() {
            out!(self, " extends ");
            for (i, extend) in extends.iter().enumerate() {
                if i > 0 {
                    self.comma();
                }
                self.gen_node(ctx, extend, Some(Path::new(node, NodeField::extends)))?;
            }
            self.space(ForceSpace::No);
        }
        self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))
    }

    /// `InterfaceDeclaration`: `interface Id<T> extends A, B { ... }`.
    ///
    /// juno `gen_js.rs:2491-2517` (dispatches to `visit_interface`).
    pub(crate) fn gen_interface_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &InterfaceDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let InterfaceDeclaration {
            metadata: _,
            id,
            type_parameters,
            extends,
            body,
        } = inner;
        self.visit_interface(ctx, "interface", id, *type_parameters, *extends, body, node)
    }

    /// `DeclareInterface`: `declare interface Id<T> extends A, B { ... }`.
    ///
    /// juno `gen_js.rs:2491-2517` (dispatches to `visit_interface`).
    pub(crate) fn gen_declare_interface<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclareInterface<'gc>,
    ) -> Result<(), GenJsError> {
        let DeclareInterface {
            metadata: _,
            id,
            type_parameters,
            extends,
            body,
        } = inner;
        self.visit_interface(
            ctx,
            "declare interface",
            id,
            *type_parameters,
            *extends,
            body,
            node,
        )
    }

    /// `DeclareOpaqueType`: `[declare] opaque type Id<T> [super L] [extends
    /// U] [= Impl];` — `impltype` is optional here (unlike `OpaqueType`,
    /// where it is required): a plain `declare opaque type T;` has none.
    ///
    /// juno `gen_js.rs:2519-2553`. See the module doc comment's "grew
    /// `lower_bound`/`upper_bound`" section for the bound clause.
    pub(crate) fn gen_declare_opaque_type<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclareOpaqueType<'gc>,
        path: Option<Path<'gc>>,
    ) -> Result<(), GenJsError> {
        let DeclareOpaqueType {
            metadata: _,
            id,
            type_parameters,
            impltype,
            lower_bound,
            upper_bound,
            supertype,
        } = inner;
        if declare_prefix_needed(path) {
            out!(self, "declare ");
        }
        out!(self, "opaque type ");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        self.gen_opaque_type_bounds(ctx, node, *lower_bound, *upper_bound, *supertype)?;
        if let Some(impltype) = impltype {
            if self.pretty() == Pretty::Yes {
                out!(self, " = ");
            } else {
                self.space_before_equals("=");
                out!(self, "=");
            }
            self.gen_node(ctx, impltype, Some(Path::new(node, NodeField::impltype)))?;
        }
        Ok(())
    }

    /// `DeclareClass`: `[declare] class Id<T> [extends E] [mixins M] [implements
    /// I] { ... }`.
    ///
    /// juno `gen_js.rs:2555-2611`.
    pub(crate) fn gen_declare_class<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclareClass<'gc>,
        path: Option<Path<'gc>>,
    ) -> Result<(), GenJsError> {
        let DeclareClass {
            metadata: _,
            id,
            type_parameters,
            extends,
            implements,
            mixins,
            body,
        } = inner;
        if declare_prefix_needed(path) {
            out!(self, "declare ");
        }
        out!(self, "class ");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        if !extends.is_empty() {
            out!(self, " extends ");
            for (i, extend) in extends.iter().enumerate() {
                if i > 0 {
                    self.comma();
                }
                self.gen_node(ctx, extend, Some(Path::new(node, NodeField::extends)))?;
            }
        }
        if !mixins.is_empty() {
            out!(self, " mixins ");
            for (i, mixin) in mixins.iter().enumerate() {
                if i > 0 {
                    self.comma();
                }
                self.gen_node(ctx, mixin, Some(Path::new(node, NodeField::mixins)))?;
            }
        }
        if !implements.is_empty() {
            out!(self, " implements ");
            for (i, implement) in implements.iter().enumerate() {
                if i > 0 {
                    self.comma();
                }
                self.gen_node(ctx, implement, Some(Path::new(node, NodeField::implements)))?;
            }
        }
        self.space(ForceSpace::No);
        self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))
    }

    /// `DeclareFunction`: `[declare] function Id<T>(params): R [%checks(...)];`
    /// — the signature is smuggled through `id`'s own `Identifier
    /// { type_annotation: TypeAnnotation(FunctionTypeAnnotation) }`, so this
    /// deep-matches into it rather than printing `id` through the ordinary
    /// `Identifier` arm (which would print `: FunctionType` instead of
    /// `(params): R`).
    ///
    /// juno `gen_js.rs:2612-2662`. juno's `unimplemented!("Malformed AST:
    /// Need to handle error")` fallbacks (both the outer `id` match and the
    /// inner `type_annotation` match) become `GenJsError::UnsupportedKind`,
    /// the standing `unreachable!()` → `GenJsError` substitution (spec §4).
    pub(crate) fn gen_declare_function<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclareFunction<'gc>,
        path: Option<Path<'gc>>,
    ) -> Result<(), GenJsError> {
        let DeclareFunction {
            metadata: _,
            id,
            predicate,
        } = inner;
        if declare_prefix_needed(path) {
            out!(self, "declare function ");
        } else {
            out!(self, "function ");
        }
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
        let name_str = ctx
            .try_bytes_str(name.get())
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        self.write_utf8(name_str);
        let Some(annot) = type_annotation else {
            return Err(GenJsError::UnsupportedKind(id.kind()));
        };
        let Node::TypeAnnotation(TypeAnnotation {
            metadata: _,
            type_annotation: fta,
        }) = annot
        else {
            return Err(GenJsError::UnsupportedKind(annot.kind()));
        };
        self.gen_methodish_type_signature(ctx, node, fta)?;
        if let Some(predicate) = predicate {
            self.space(ForceSpace::No);
            self.gen_node(ctx, predicate, Some(Path::new(node, NodeField::predicate)))?;
        }
        Ok(())
    }

    /// `DeclareVariable`: `[declare] var|let|const Id: T;`.
    ///
    /// juno `gen_js.rs:2679-2691`. See the module doc comment's
    /// "`DeclareVariable` silently drops" section for the fix: `declare`'s
    /// presence now uses [`declare_prefix_needed`] (matching the other three
    /// `Declare*` arms) and the `var`/`let`/`const` keyword now always
    /// prints, rather than both being gated on `path.is_some()`.
    pub(crate) fn gen_declare_variable<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclareVariable<'gc>,
        path: Option<Path<'gc>>,
    ) -> Result<(), GenJsError> {
        let DeclareVariable { metadata: _, id, kind: _ } = inner;
        if declare_prefix_needed(path) {
            out!(self, "declare ");
        }
        out!(self, "{} ", inner.kind_str(ctx));
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))
    }

    /// `DeclareExportDeclaration`: `declare export [default] Decl;` or
    /// `declare export { specs } [from Source];`.
    ///
    /// juno `gen_js.rs:2692-2719`.
    pub(crate) fn gen_declare_export_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclareExportDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let DeclareExportDeclaration {
            metadata: _,
            declaration,
            specifiers,
            source,
            default,
        } = inner;
        out!(self, "declare export ");
        if default.get() {
            out!(self, "default ");
        }
        if let Some(declaration) = declaration {
            self.gen_node(
                ctx,
                declaration,
                Some(Path::new(node, NodeField::declaration)),
            )?;
        } else {
            out!(self, "{{");
            for (i, spec) in specifiers.iter().enumerate() {
                if i > 0 {
                    self.comma();
                }
                self.gen_node(ctx, spec, Some(Path::new(node, NodeField::specifiers)))?;
            }
            out!(self, "}}");
            if let Some(source) = source {
                out!(self, " from ");
                self.gen_node(ctx, source, Some(Path::new(node, NodeField::source)))?;
            }
        }
        Ok(())
    }

    /// `DeclareExportAllDeclaration`: `declare export * from Source;`.
    ///
    /// juno `gen_js.rs:2720-2726`.
    pub(crate) fn gen_declare_export_all_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclareExportAllDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let DeclareExportAllDeclaration { metadata: _, source } = inner;
        out!(self, "declare export * from ");
        self.gen_node(ctx, source, Some(Path::new(node, NodeField::source)))
    }

    /// `DeclareModule`: `declare module Id { Body }`.
    ///
    /// juno `gen_js.rs:2727-2737`.
    pub(crate) fn gen_declare_module<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclareModule<'gc>,
    ) -> Result<(), GenJsError> {
        let DeclareModule { metadata: _, id, body } = inner;
        out!(self, "declare module ");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        self.space(ForceSpace::No);
        self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))
    }

    /// `DeclareModuleExports`: `declare module.exports: T;`.
    ///
    /// juno `gen_js.rs:2738-2746`.
    pub(crate) fn gen_declare_module_exports<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclareModuleExports<'gc>,
    ) -> Result<(), GenJsError> {
        let DeclareModuleExports {
            metadata: _,
            type_annotation,
        } = inner;
        out!(self, "declare module.exports:");
        self.space(ForceSpace::No);
        self.gen_node(
            ctx,
            type_annotation,
            Some(Path::new(node, NodeField::type_annotation)),
        )
    }

    /// Shared printing logic for `InterfaceExtends`/`ClassImplements` —
    /// identical field sets (`id`, `type_parameters`); see the module doc
    /// comment's "`Node::ClassImplements`" section for why the latter is
    /// ported here despite not being named in the brief.
    fn gen_interface_extends_like<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        id: &'gc Node<'gc>,
        type_parameters: Option<&'gc Node<'gc>>,
    ) -> Result<(), GenJsError> {
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

    /// `InterfaceExtends`: `Id<T>` (one entry of an `extends`/`mixins`
    /// clause).
    ///
    /// juno `gen_js.rs:2747-2762` (shared arm with `ClassImplements`).
    pub(crate) fn gen_interface_extends<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &InterfaceExtends<'gc>,
    ) -> Result<(), GenJsError> {
        let InterfaceExtends {
            metadata: _,
            id,
            type_parameters,
        } = inner;
        self.gen_interface_extends_like(ctx, node, id, *type_parameters)
    }

    /// `ClassImplements`: `Id<T>` (one entry of an `implements` clause).
    ///
    /// juno `gen_js.rs:2747-2762` (shared arm with `InterfaceExtends`).
    pub(crate) fn gen_class_implements<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ClassImplements<'gc>,
    ) -> Result<(), GenJsError> {
        let ClassImplements {
            metadata: _,
            id,
            type_parameters,
        } = inner;
        self.gen_interface_extends_like(ctx, node, id, *type_parameters)
    }

    /// `TypeAnnotation`: a transparent wrapper — prints its inner type only.
    ///
    /// juno `gen_js.rs:2767-2772`.
    pub(crate) fn gen_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let TypeAnnotation {
            metadata: _,
            type_annotation,
        } = inner;
        self.gen_node(
            ctx,
            type_annotation,
            Some(Path::new(node, NodeField::type_annotation)),
        )
    }

    /// `ObjectTypeAnnotation`: `{ props; indexers; calls; slots [...] }` (or
    /// `{| ... |}` when `exact`).
    ///
    /// juno `gen_js.rs:2773-2831`.
    pub(crate) fn gen_object_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ObjectTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let ObjectTypeAnnotation {
            metadata: _,
            properties,
            indexers,
            call_properties,
            internal_slots,
            inexact,
            exact,
        } = inner;
        out!(self, "{}", if exact.get() { "{|" } else { "{" });
        self.inc_indent();
        self.newline();

        let mut need_comma = false;

        for prop in properties.iter() {
            if need_comma {
                self.comma();
            }
            self.gen_node(ctx, prop, Some(Path::new(node, NodeField::properties)))?;
            self.newline();
            need_comma = true;
        }
        for prop in indexers.iter() {
            if need_comma {
                self.comma();
            }
            self.gen_node(ctx, prop, Some(Path::new(node, NodeField::indexers)))?;
            self.newline();
            need_comma = true;
        }
        for prop in call_properties.iter() {
            if need_comma {
                self.comma();
            }
            self.gen_node(ctx, prop, Some(Path::new(node, NodeField::call_properties)))?;
            self.newline();
            need_comma = true;
        }
        for prop in internal_slots.iter() {
            if need_comma {
                self.comma();
            }
            self.gen_node(ctx, prop, Some(Path::new(node, NodeField::internal_slots)))?;
            self.newline();
            need_comma = true;
        }

        if inexact.get() {
            if need_comma {
                self.comma();
            }
            out!(self, "...");
        }

        self.dec_indent();
        self.newline();
        out!(self, "{}", if exact.get() { "|}" } else { "}" });
        Ok(())
    }

    /// `ObjectTypeProperty`: `[variance] [static] [proto] key[?]: value`, a
    /// method-shaped `[static] key<T>(params): R`, or (see the module doc
    /// comment's "`ObjectTypeProperty` silently drops `get`/`set`" section)
    /// an accessor-shaped `[static] get|set key(params): R`.
    ///
    /// juno `gen_js.rs:2832-2891`. **DEVIATION from juno — a real round-trip
    /// fix, not a transcription**; see the module doc comment.
    pub(crate) fn gen_object_type_property<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ObjectTypeProperty<'gc>,
    ) -> Result<(), GenJsError> {
        let ObjectTypeProperty {
            metadata: _,
            key,
            value,
            method,
            optional,
            r#static,
            proto,
            variance,
            kind,
        } = inner;
        // `proto`/`static` FIRST, then the variance sigil.
        //
        // **DEVIATION from juno — a correctness fix found by the Tier 2
        // sweep** (`test/Parser/flow/proto.js`, `static-property.js`). juno
        // prints the variance first (`gen_js.rs:2952-2961`), which for
        // `declare class B { proto +x: T }` emits `+proto x: T` — a hard
        // reparse failure (`':' or '?' expected in property type
        // annotation`), not a silent divergence. `parseTypePropertyFlow`'s
        // caller reads the modifiers in exactly one order:
        // `proto`, then `static` (only `if (!proto)`), then the `+`/`-` or
        // `readonly`/`writeonly` sigil (`lib/Parser/JSParserImpl-flow.cpp:4178-4205`).
        // `gen_object_type_indexer` already prints them in that order; this
        // arm was the one that did not.
        if r#static.get() {
            out!(self, "static ");
        }
        if proto.get() {
            out!(self, "proto ");
        }
        if let Some(variance) = variance {
            self.gen_node(ctx, variance, Some(Path::new(node, NodeField::variance)))?;
        }
        let prop_kind = ObjectTypePropertyKind::from_label(ctx, kind.get())?;
        match prop_kind {
            ObjectTypePropertyKind::Get | ObjectTypePropertyKind::Set => {
                let is_get = prop_kind == ObjectTypePropertyKind::Get;
                out!(self, "{} ", if is_get { "get" } else { "set" });
                self.gen_node(ctx, key, Some(Path::new(node, NodeField::key)))?;
                self.gen_methodish_type_signature(ctx, node, value)?;
            }
            ObjectTypePropertyKind::Init => {
                self.gen_node(ctx, key, Some(Path::new(node, NodeField::key)))?;
                if optional.get() {
                    out!(self, "?");
                }
                if method.get() {
                    self.gen_methodish_type_signature(ctx, node, value)?;
                } else {
                    out!(self, ":");
                    self.space(ForceSpace::No);
                    self.gen_node(ctx, value, Some(Path::new(node, NodeField::value)))?;
                }
            }
        }
        Ok(())
    }

    /// `ObjectTypeSpreadProperty`: `...argument`.
    ///
    /// juno `gen_js.rs:2892-2898`.
    pub(crate) fn gen_object_type_spread_property<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ObjectTypeSpreadProperty<'gc>,
    ) -> Result<(), GenJsError> {
        let ObjectTypeSpreadProperty { metadata: _, argument } = inner;
        out!(self, "...");
        self.gen_node(ctx, argument, Some(Path::new(node, NodeField::argument)))
    }

    /// `ObjectTypeInternalSlot`: `[static] [[id]][?]: value` or `[static]
    /// [[id]]<T>(params): R` (when `method`).
    ///
    /// juno `gen_js.rs:2899-2951`.
    pub(crate) fn gen_object_type_internal_slot<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ObjectTypeInternalSlot<'gc>,
    ) -> Result<(), GenJsError> {
        let ObjectTypeInternalSlot {
            metadata: _,
            id,
            value,
            optional,
            r#static,
            method,
        } = inner;
        if r#static.get() {
            out!(self, "static ");
        }
        out!(self, "[[");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        out!(self, "]]");
        if optional.get() {
            out!(self, "?");
        }
        if method.get() {
            self.gen_methodish_type_signature(ctx, node, value)?;
        } else {
            out!(self, ":");
            self.space(ForceSpace::No);
            self.gen_node(ctx, value, Some(Path::new(node, NodeField::value)))?;
        }
        Ok(())
    }

    /// `ObjectTypeCallProperty`: `[static] (params): R`.
    ///
    /// juno `gen_js.rs:2952-2985`.
    pub(crate) fn gen_object_type_call_property<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ObjectTypeCallProperty<'gc>,
    ) -> Result<(), GenJsError> {
        let ObjectTypeCallProperty { metadata: _, value, r#static } = inner;
        if r#static.get() {
            out!(self, "static ");
        }
        self.gen_methodish_type_signature(ctx, node, value)
    }

    /// `ObjectTypeIndexer`: `[static] [variance] [id:] [key]: value`.
    ///
    /// juno `gen_js.rs:2986-3011`.
    pub(crate) fn gen_object_type_indexer<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ObjectTypeIndexer<'gc>,
    ) -> Result<(), GenJsError> {
        let ObjectTypeIndexer {
            metadata: _,
            id,
            key,
            value,
            r#static,
            variance,
        } = inner;
        if r#static.get() {
            out!(self, "static ");
        }
        if let Some(variance) = variance {
            self.gen_node(ctx, variance, Some(Path::new(node, NodeField::variance)))?;
        }
        out!(self, "[");
        if let Some(id) = id {
            self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
            out!(self, ":");
            self.space(ForceSpace::No);
        }
        self.gen_node(ctx, key, Some(Path::new(node, NodeField::key)))?;
        out!(self, "]");
        out!(self, ":");
        self.space(ForceSpace::No);
        self.gen_node(ctx, value, Some(Path::new(node, NodeField::value)))
    }

    /// `Variance`: `+`, `-`, `readonly `, `writeonly `, `in `, or `out `.
    ///
    /// juno `gen_js.rs:3012-3025`. See the module doc comment's "grew a
    /// `writeonly` spelling" section for the `writeonly` case, and its
    /// "grew `in`/`out` variance spellings" section for those two — all
    /// three are absent from juno's frozen version.
    pub(crate) fn gen_variance<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        inner: &Variance<'gc>,
    ) -> Result<(), GenJsError> {
        let Variance { metadata: _, kind } = inner;
        out!(
            self,
            "{}",
            match VarianceKind::from_label(ctx, kind.get())? {
                VarianceKind::Plus => "+",
                VarianceKind::Minus => "-",
                VarianceKind::Readonly => "readonly ",
                VarianceKind::Writeonly => "writeonly ",
                VarianceKind::In => "in ",
                VarianceKind::Out => "out ",
            }
        );
        Ok(())
    }

    /// `TypeParameter`: `[const] [variance] name[: bound| extends bound]
    /// [= default]` (one element of a `TypeParameterDeclaration`).
    ///
    /// juno `gen_js.rs:3043-3065`. See the module doc comment's "grew a
    /// `const` field" section for the `const` prefix, absent from juno's
    /// frozen version, and "`uses_extends_bound` IS preserved" for why
    /// `bound`'s original `:`-vs-`extends` spelling is round-tripped here
    /// where juno drops it.
    pub(crate) fn gen_type_parameter<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TypeParameter<'gc>,
    ) -> Result<(), GenJsError> {
        let TypeParameter {
            metadata: _,
            name,
            r#const,
            bound,
            variance,
            default,
            uses_extends_bound,
        } = inner;
        if r#const.get() {
            out!(self, "const ");
        }
        if let Some(variance) = variance {
            self.gen_node(ctx, variance, Some(Path::new(node, NodeField::variance)))?;
        }
        let name_str = ctx
            .try_bytes_str(name.get())
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        self.write_utf8(name_str);
        if let Some(bound) = bound {
            // `extends` is a keyword, so its surrounding spaces are required
            // even in `Pretty::No`; `:` takes an optional one.
            if uses_extends_bound.get() {
                out!(self, " extends ");
            } else {
                out!(self, ":");
                self.space(ForceSpace::No);
            }
            self.gen_node(ctx, bound, Some(Path::new(node, NodeField::bound)))?;
        }
        if let Some(default) = default {
            self.space_before_equals("=");
            out!(self, "=");
            self.space(ForceSpace::No);
            self.gen_node(ctx, default, Some(Path::new(node, NodeField::default)))?;
        }
        Ok(())
    }

    /// `TypeCastExpression`: `(expression: type_annotation)` — the
    /// parentheses are required syntax, not optional pretty-printing.
    ///
    /// juno `gen_js.rs:3066-3088`.
    pub(crate) fn gen_type_cast_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TypeCastExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let TypeCastExpression {
            metadata: _,
            expression,
            type_annotation,
        } = inner;
        out!(self, "(");
        self.print_child(
            ctx,
            Some(*expression),
            Path::new(node, NodeField::expression),
            ChildPos::Left,
        )?;
        out!(self, ":");
        self.space(ForceSpace::No);
        self.print_child(
            ctx,
            Some(*type_annotation),
            Path::new(node, NodeField::type_annotation),
            ChildPos::Right,
        )?;
        out!(self, ")");
        Ok(())
    }

    /// `InferredPredicate`: `%checks`.
    ///
    /// juno `gen_js.rs:3089-3091`. No fields besides `metadata`.
    pub(crate) fn gen_inferred_predicate(&mut self, _inner: &InferredPredicate<'_>) -> Result<(), GenJsError> {
        out!(self, "%checks");
        Ok(())
    }

    /// `DeclaredPredicate`: `%checks(value)`.
    ///
    /// juno `gen_js.rs:3092-3097`.
    pub(crate) fn gen_declared_predicate<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DeclaredPredicate<'gc>,
    ) -> Result<(), GenJsError> {
        let DeclaredPredicate { metadata: _, value } = inner;
        out!(self, "%checks(");
        self.gen_node(ctx, value, Some(Path::new(node, NodeField::value)))?;
        out!(self, ")");
        Ok(())
    }

    /// `EnumDeclaration`: `enum Id Body`.
    ///
    /// juno `gen_js.rs:3098-3106`.
    pub(crate) fn gen_enum_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &EnumDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let EnumDeclaration { metadata: _, id, body } = inner;
        out!(self, "enum ");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))
    }

    /// Generate the body of a Flow enum with element kind `kind` (e.g.
    /// `"string"`).
    ///
    /// juno `gen_js.rs:3484-3523` (`visit_enum_body`).
    pub(crate) fn visit_enum_body<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        kind: &str,
        members: NodeList<'gc>,
        explicit_type: bool,
        has_unknown_members: bool,
        node: &'gc Node<'gc>,
    ) -> Result<(), GenJsError> {
        if explicit_type {
            out!(self, " of {}", kind);
        }
        self.space(ForceSpace::No);
        out!(self, "{{");
        self.inc_indent();
        self.newline();

        for (i, member) in members.iter().enumerate() {
            if i > 0 {
                self.comma();
                self.newline();
            }
            self.gen_node(ctx, member, Some(Path::new(node, NodeField::members)))?;
        }

        if has_unknown_members {
            if !members.is_empty() {
                self.comma();
                self.newline();
            }
            out!(self, "...");
        }

        self.dec_indent();
        self.newline();
        out!(self, "}}");
        Ok(())
    }

    /// `EnumStringBody`: an enum body of `string`-valued members.
    ///
    /// juno `gen_js.rs:3107-3121` (dispatches to `visit_enum_body`).
    pub(crate) fn gen_enum_string_body<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &EnumStringBody<'gc>,
    ) -> Result<(), GenJsError> {
        let EnumStringBody {
            metadata: _,
            members,
            explicit_type,
            has_unknown_members,
        } = inner;
        self.visit_enum_body(
            ctx,
            "string",
            *members,
            explicit_type.get(),
            has_unknown_members.get(),
            node,
        )
    }

    /// `EnumNumberBody`: an enum body of `number`-valued members.
    ///
    /// juno `gen_js.rs:3122-3136` (dispatches to `visit_enum_body`).
    pub(crate) fn gen_enum_number_body<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &EnumNumberBody<'gc>,
    ) -> Result<(), GenJsError> {
        let EnumNumberBody {
            metadata: _,
            members,
            explicit_type,
            has_unknown_members,
        } = inner;
        self.visit_enum_body(
            ctx,
            "number",
            *members,
            explicit_type.get(),
            has_unknown_members.get(),
            node,
        )
    }

    /// `EnumBooleanBody`: an enum body of `boolean`-valued members.
    ///
    /// juno `gen_js.rs:3137-3151` (dispatches to `visit_enum_body`).
    pub(crate) fn gen_enum_boolean_body<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &EnumBooleanBody<'gc>,
    ) -> Result<(), GenJsError> {
        let EnumBooleanBody {
            metadata: _,
            members,
            explicit_type,
            has_unknown_members,
        } = inner;
        self.visit_enum_body(
            ctx,
            "boolean",
            *members,
            explicit_type.get(),
            has_unknown_members.get(),
            node,
        )
    }

    /// `EnumSymbolBody`: an enum body of `symbol` members (defaulted only —
    /// `symbol` enums have no initializers, so `explicit_type` is always
    /// `true`: a `symbol` enum can never omit `of symbol`).
    ///
    /// juno `gen_js.rs:3152-3158` (dispatches to `visit_enum_body`).
    pub(crate) fn gen_enum_symbol_body<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &EnumSymbolBody<'gc>,
    ) -> Result<(), GenJsError> {
        let EnumSymbolBody {
            metadata: _,
            members,
            has_unknown_members,
        } = inner;
        self.visit_enum_body(ctx, "symbol", *members, true, has_unknown_members.get(), node)
    }

    /// `EnumDefaultedMember`: a bare `Id` (no initializer).
    ///
    /// juno `gen_js.rs:3159-3161`.
    pub(crate) fn gen_enum_defaulted_member<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &EnumDefaultedMember<'gc>,
    ) -> Result<(), GenJsError> {
        let EnumDefaultedMember { metadata: _, id } = inner;
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))
    }

    /// Shared printing logic for `EnumStringMember`/`EnumNumberMember`/
    /// `EnumBooleanMember` — identical shape, `Id = init`.
    ///
    /// `pub(crate)`, not private: Task 12's `arms/newer.rs` reuses this for
    /// `EnumBigIntMember`, the fifth member of this same family, deliberately
    /// left for that task (see this file's module doc comment).
    pub(crate) fn gen_enum_member_with_init<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        id: &'gc Node<'gc>,
        init: &'gc Node<'gc>,
    ) -> Result<(), GenJsError> {
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        let eq = match self.pretty() {
            Pretty::Yes => " = ",
            Pretty::No => "=",
        };
        self.space_before_equals(eq);
        out!(
            self,
            "{}",
            match self.pretty() {
                Pretty::Yes => " = ",
                Pretty::No => "=",
            }
        );
        self.gen_node(ctx, init, Some(Path::new(node, NodeField::init)))
    }

    /// `EnumStringMember`: `Id = "string"`.
    ///
    /// juno `gen_js.rs:3162-3181` (shared arm with `EnumNumberMember`/
    /// `EnumBooleanMember`).
    pub(crate) fn gen_enum_string_member<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &EnumStringMember<'gc>,
    ) -> Result<(), GenJsError> {
        let EnumStringMember { metadata: _, id, init } = inner;
        self.gen_enum_member_with_init(ctx, node, id, init)
    }

    /// `EnumNumberMember`: `Id = 42`.
    ///
    /// juno `gen_js.rs:3162-3181` (shared arm with `EnumStringMember`/
    /// `EnumBooleanMember`).
    pub(crate) fn gen_enum_number_member<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &EnumNumberMember<'gc>,
    ) -> Result<(), GenJsError> {
        let EnumNumberMember { metadata: _, id, init } = inner;
        self.gen_enum_member_with_init(ctx, node, id, init)
    }

    /// `EnumBooleanMember`: `Id = true`.
    ///
    /// juno `gen_js.rs:3162-3181` (shared arm with `EnumStringMember`/
    /// `EnumNumberMember`).
    pub(crate) fn gen_enum_boolean_member<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &EnumBooleanMember<'gc>,
    ) -> Result<(), GenJsError> {
        let EnumBooleanMember { metadata: _, id, init } = inner;
        self.gen_enum_member_with_init(ctx, node, id, init)
    }
}
