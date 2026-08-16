/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Functions, classes, methods, and properties: `FunctionExpression`,
//! `FunctionDeclaration`, `ArrowFunctionExpression`, `ClassExpression`,
//! `ClassDeclaration`, `ClassBody`, `ClassProperty`, `ClassPrivateProperty`,
//! `MethodDefinition`, plus the `visit_func_type_params` helper.
//!
//! Ported from juno `gen_js.rs:374-520` (`FunctionExpression`/
//! `FunctionDeclaration`/`ArrowFunctionExpression`), `gen_js.rs:1553-1795`
//! (`ClassExpression`/`ClassDeclaration`/`ClassBody`/`ClassProperty`/
//! `ClassPrivateProperty`/`MethodDefinition`), and `gen_js.rs:3401-3452`
//! (`visit_func_type_params`). This is the plan's Task 7.
//! `visit_func_params_body` (`gen_js.rs:3365-3399`), also listed in the
//! task brief's "Produces", already exists (`arms/expr.rs`'s
//! `GenJS::visit_func_params_body`) — Task 5 needed it early for `Property`;
//! see that module's doc comment for why. Nothing here re-implements it.
//!
//! `visit_func_type_params` was dead code when this task landed it, pending
//! a later task's Flow `FunctionTypeAnnotation` arm (juno's own only
//! caller of the identical logic, `gen_js.rs:2223-2266`, is Flow-type
//! printing — Task 10's territory, not Task 9's as an earlier draft of this
//! comment said). Task 10's `arms/flow_type.rs`'s `GenJS::gen_function_type_annotation`
//! is now that caller, so this is no longer dead code and no longer carries
//! `#[allow(dead_code)]`.
//!
//! # A juno correctness bug found and fixed here, not transcribed
//!
//! **`ClassPrivateProperty`'s `declare` modifier printed `"static "` a
//! second time instead of `"declare "`.** juno's arm (`gen_js.rs:1691-1717`):
//!
//! ```text
//! if *is_static {
//!     out!(self, "static ");
//! }
//! if *declare {
//!     out!(self, "static ");   // <- should be "declare "
//! }
//! ```
//!
//! Two independent pieces of evidence confirm this is a copy-paste bug, not
//! deliberate: (1) the sibling `ClassProperty` arm three cases above gets it
//! right (`if *declare { out!(self, "declare "); } if *is_static { out!(self,
//! "static "); }` — juno `gen_js.rs:1639-1645`); (2) our own parser
//! (`crates/parser/src/js/classes.rs:606-618`) only recognizes `declare` as a
//! modifier when it is immediately followed by `static`/an identifier/`+`/
//! `-`/a private name — i.e. `declare` always precedes `static` in the
//! source grammar, confirming `declare`'s keyword is `"declare "`, not a
//! second `"static "`. Reprinting `declare #x;` as `static #x;` (or
//! `declare static #x;` as `static static #x;`) is not cosmetic: it silently
//! turns a per-instance private field into a per-class one on reparse — a
//! genuine round-trip correctness bug, not just odd-looking output. Fixed
//! here to `out!(self, "declare ")`, and reordered to `declare` before
//! `static` (matching both the parser's own source order and
//! `ClassProperty`'s already-correct arm) rather than juno's `static` before
//! (buggy) `declare`. `tests/roundtrip.rs`'s
//! `class_private_property_declare_modifier_prints_declare_not_static` is
//! the regression test; see task-7-report.md for the "revert the fix, watch
//! it fail" transcript.
//!
//! # Adaptations specific to this module
//!
//! **`ArrowFunctionExpression`'s single-parameter shortcut cannot pattern-match
//! `optional: false` the way juno's struct-literal pattern does.** juno's
//! `Identifier` has a plain `bool` `optional` field, so
//! `Node::Identifier(Identifier { type_annotation: None, optional: false, ..
//! })` is one pattern. Ours is `Cell<bool>` (`crates/ast/src/node.rs`'s
//! `Identifier::optional`), which cannot appear as a `false` literal in a
//! struct pattern; this arm instead matches `type_annotation: None` in the
//! pattern (a real `Option`, still literal-matchable) and checks
//! `!optional.get()` as a separate guard. Same substitution
//! `arms/expr.rs`'s `UpdateExpression`/`BinaryExpression` arms already made
//! for every other `Cell<bool>` field via `.get()` — nothing new in kind,
//! just the first time it interacts with a struct *pattern* rather than a
//! plain field read.
//!
//! **The single-parameter shortcut locates its one parameter without
//! `NodeList::len()`/`::head()`.** Neither exists on our `NodeList`
//! (`crates/ast/src/node_child.rs`'s linked-list `NodeList` exposes only
//! `is_empty()`/`iter()`); this arm instead pulls the first two items from
//! `params.iter()` and checks the shape `(Some(p), None)` — "exactly one
//! element" without a length query.
//!
//! **`ClassDeclaration`/`ClassExpression`'s decorator-vs-no-decorator
//! branch collapses to one path.** juno's arm (`gen_js.rs:1572-1577`)
//! chooses between `out_token!(self, node, "class")` (no decorators) and a
//! decorator-printing loop followed by a bare `out!(self, "class")` (has
//! decorators). Per the plan's Adaptation Rules, `out_token!` always
//! collapses to plain `out!` here (the sourcemap segment it recorded is
//! dropped, spec §6) — once that substitution is made, both branches print
//! the byte-identical `"class"`, and the "no decorators" branch's `for`
//! loop over an empty `NodeList` is already a no-op. The `if
//! !decorators.is_empty()` guard is therefore dead after the adaptation
//! rule is applied, not a judgment call about output; this arm drops it and
//! keeps the loop unconditional.
//!
//! **`ClassProperty`/`ClassPrivateProperty`/`MethodDefinition`'s
//! `decorators: NodeList` field has no juno counterpart at all.** juno's
//! `ClassProperty`/`ClassPrivateProperty`/`MethodDefinition`
//! (`juno_ast/src/def.rs:322-349`) predate the class-member decorators
//! proposal entirely — only `ClassDeclaration`/`ClassExpression` carry
//! `decorators` there. Ours adds the field to all three member kinds
//! (`crates/ast/src/node.rs`), so there is no juno line range to port for
//! it. This arm mirrors `ClassDeclaration`/`ClassExpression`'s own
//! decorator loop (print each, `force_newline` after) for consistency, run
//! before the rest of the member's own printing. Each decorator element is
//! a `Decorator` node (`crates/ast/src/node.rs:3374`, `expression` field),
//! which has no dispatch arm until Task 12 (`arms/newer.rs` — the plan's
//! own inventory puts `Decorator` among "the 53 ES/Flow kinds juno lacks");
//! until then a decorated member reports `UnsupportedKind(Decorator)`
//! rather than printing wrong-but-quiet output — the same posture every
//! other not-yet-ported kind takes through the temporary catch-all. No
//! required test exercises this path.
//!
//! **`ts_modifiers: Option<&Node>` cannot stay `ts_modifiers: None` in the
//! struct pattern.** juno's `ClassProperty`/`ClassPrivateProperty` arms
//! (`gen_js.rs:1631-1638`, `1683-1690`) match the whole node with a literal
//! `ts_modifiers: None` field, falling through to its (TS-only, still
//! unimplemented in juno) sibling case when `Some`. These arms destructure
//! `ts_modifiers` as a normal binding instead.
//!
//! Task 7 landed that binding with an `is_some() => UnsupportedKind` bail,
//! on the understanding that TS modifier printing belonged to the TS arms
//! task. Task 13 replaced it with real printing, because the bail turned out
//! to reject every TypeScript class **with a property field**, not just the
//! modifier-carrying ones: `crates/parser/src/js/classes.rs` builds a
//! `TSModifiers` node unconditionally under `-parse-ts`, so even
//! `class C { x = 1; }` arrives with
//! `Some(TSModifiers { accessibility: null, readonly: false })`. Method-only
//! and empty TS classes were never affected — the bail lived only in these
//! two arms (review round 1 finding M-1, which measured
//! `class C {}`/`class C { m() {} }` generating fine under the old bail). The
//! two halves are printed either side of `static` — the parser accepts the
//! modifiers only in the order accessibility, `static`, `readonly` — via
//! [`GenJS::print_ts_modifiers_accessibility`] and
//! [`GenJS::print_ts_modifiers_readonly`] (`arms/ts.rs`); see that module's
//! doc comment for why one-unit printing is wrong. [`ts_modifiers_of`] does
//! the unwrap.
//!
//! **`MethodDefinition`'s call into `visit_func_params_body` passes the
//! `FunctionExpression` node, not the `MethodDefinition` node — a deviation
//! from juno, matching `Property`'s own call instead.** juno's two callers
//! disagree with each other: `Property`'s arm (`gen_js.rs:1508-1521`) passes
//! `*value` (the `FunctionExpression` itself) as `visit_func_params_body`'s
//! trailing `node` parameter (used only to build each printed child's
//! `Path`), while `MethodDefinition`'s arm (`gen_js.rs:1787-1794`) passes
//! `node` (the enclosing `MethodDefinition`). Structurally `Property` is
//! right: `params`/`body`/`return_type`/`predicate` are fields of the
//! `FunctionExpression`, not of whatever wraps it, so `Path::parent` should
//! read as the `FunctionExpression` either way — a `MethodDefinition` has no
//! `params` field of its own for `NodeField::params` to plausibly name.
//! Nothing in today's `need_parens` branches on `path.parent` being
//! `MethodDefinition` vs `FunctionExpression` for these fields (checked
//! against every branch in `precedence.rs`), so this is currently
//! behaviorally inert either way — but there is no reason to carry forward
//! the less-accurate of two disagreeing juno call sites when the more
//! accurate one is sitting right there, already committed, one task over.
//! This arm follows `Property`'s.

use hermes_ast::context::GCLock;
use hermes_ast::node::{
    ArrowFunctionExpression, ClassBody, ClassDeclaration, ClassExpression, ClassPrivateProperty,
    ClassProperty, FunctionDeclaration, FunctionExpression, FunctionTypeParam, Identifier,
    MethodDefinition, Node, NodeField, TSModifiers,
};
use hermes_ast::node_child::{NodeLabel, NodeList};
use hermes_ast::visitor::Path;

use crate::precedence::{ChildPos, ForceSpace};
use crate::{out, GenJS, GenJsError, Pretty};

// ---------------------------------------------------------------------------
// `MethodDefinitionKind`, this module's own operator-shaped classifier —
// same rationale as `arms/expr.rs`'s `PropertyKind` (lives here rather than
// `precedence.rs` because nothing in `get_precedence`/`need_parens` ever
// needs to classify a `MethodDefinition`'s `kind`).
// ---------------------------------------------------------------------------

/// `MethodDefinition::kind`, classified from its raw spelling.
///
/// Variant set and spellings from juno `juno_ast/src/node_enums.rs:111-117`
/// (`define_str_enum!(MethodDefinitionKind, ..., (Method, "method"),
/// (Constructor, "constructor"), (Get, "get"), (Set, "set"))`); confirmed
/// against the bundled parser's own atoms (`crates/parser/src/js/classes.rs`,
/// the `kind` local around line 1271: `atom_bytes(b"method"/b"constructor"/
/// b"get"/b"set")`).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum MethodDefinitionKind {
    /// `method`: an ordinary method.
    Method,
    /// `constructor`: the class constructor (spelled via the key, not this
    /// keyword — see [`GenJS::gen_method_definition`]).
    Constructor,
    /// `get`: a getter.
    Get,
    /// `set`: a setter.
    Set,
}

impl MethodDefinitionKind {
    /// Classify `label`, the raw contents of a `MethodDefinition`'s `kind`
    /// field.
    ///
    /// # Errors
    /// `Err(GenJsError::UnknownOperator { .. })` if `label`'s spelling is
    /// none of the 4 above. Reuses `GenJsError::UnknownOperator` for the same
    /// reason `arms/expr.rs`'s `PropertyKind::from_label` does: the failure
    /// mode (an enum-shaped field holding an out-of-set spelling, from a
    /// hand-built or JSON-deserialized tree per spec §4) is identical, and a
    /// `kind: "MethodDefinition"` payload names it precisely enough.
    fn from_label(gc: &GCLock<'_, '_>, label: NodeLabel) -> Result<Self, GenJsError> {
        Ok(match gc.bytes_str_lossy(label) {
            "method" => Self::Method,
            "constructor" => Self::Constructor,
            "get" => Self::Get,
            "set" => Self::Set,
            other => {
                return Err(GenJsError::UnknownOperator {
                    kind: "MethodDefinition",
                    spelling: other.to_string(),
                })
            }
        })
    }
}

impl<'s, 'w> GenJS<'s, 'w> {
    /// Shared printing logic for `FunctionExpression` and
    /// `FunctionDeclaration` — juno matches both kinds in one arm
    /// (`gen_js.rs:376-421`) since their field sets are identical; this
    /// crate dispatches one method per kind (`dispatch.rs`'s module doc
    /// comment), so [`GenJS::gen_function_expression`]/
    /// [`GenJS::gen_function_declaration`] are thin wrappers around this.
    ///
    /// juno `gen_js.rs:376-421`.
    #[allow(clippy::too_many_arguments)]
    fn gen_function_like<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        id: Option<&'gc Node<'gc>>,
        params: NodeList<'gc>,
        body: &'gc Node<'gc>,
        type_parameters: Option<&'gc Node<'gc>>,
        return_type: Option<&'gc Node<'gc>>,
        predicate: Option<&'gc Node<'gc>>,
        generator: bool,
        is_async: bool,
    ) -> Result<(), GenJsError> {
        if is_async {
            out!(self, "async function");
        } else {
            out!(self, "function");
        }
        if generator {
            out!(self, "*");
            if id.is_some() {
                self.space(ForceSpace::No);
            }
        } else if id.is_some() {
            self.space(ForceSpace::Yes);
        }
        if let Some(id) = id {
            self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        }
        self.visit_func_params_body(
            ctx,
            params,
            type_parameters,
            return_type,
            predicate,
            body,
            node,
        )
    }

    /// `FunctionExpression`: `[async] function[*] [id](params) [: return_type]
    /// body`.
    ///
    /// juno `gen_js.rs:376-421` (shared with `FunctionDeclaration`, see
    /// [`GenJS::gen_function_like`]).
    pub(crate) fn gen_function_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &FunctionExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let FunctionExpression {
            metadata: _,
            id,
            params,
            body,
            type_parameters,
            return_type,
            predicate,
            generator,
            r#async,
            // Sema decorations (grandfathered onto the AST node itself, per
            // `ast-annotation-principle`): not read by this generator.
            scope: _,
            sem_info: _,
            strictness: _,
            is_method_definition: _,
            decorations: _,
        } = inner;
        self.gen_function_like(
            ctx,
            node,
            *id,
            *params,
            body,
            *type_parameters,
            *return_type,
            *predicate,
            generator.get(),
            r#async.get(),
        )
    }

    /// `FunctionDeclaration`: same shape as `FunctionExpression`; see
    /// [`GenJS::gen_function_like`].
    ///
    /// juno `gen_js.rs:376-421`.
    pub(crate) fn gen_function_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &FunctionDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let FunctionDeclaration {
            metadata: _,
            id,
            params,
            body,
            type_parameters,
            return_type,
            predicate,
            generator,
            r#async,
            scope: _,
            sem_info: _,
            strictness: _,
            is_method_definition: _,
            decorations: _,
        } = inner;
        self.gen_function_like(
            ctx,
            node,
            *id,
            *params,
            body,
            *type_parameters,
            *return_type,
            *predicate,
            generator.get(),
            r#async.get(),
        )
    }

    /// `ArrowFunctionExpression`: `[async] (params) [: return_type] =>
    /// body`, with a single-identifier-parameter shortcut that omits the
    /// parens.
    ///
    /// juno `gen_js.rs:423-519`. See the module doc comment for the two
    /// adaptations (`Cell<bool>` vs a `false` struct-pattern literal; no
    /// `NodeList::len()`/`::head()`) the single-parameter shortcut needs.
    pub(crate) fn gen_arrow_function_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ArrowFunctionExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let ArrowFunctionExpression {
            metadata: _,
            params,
            body,
            type_parameters,
            return_type,
            predicate,
            expression,
            r#async,
            scope: _,
            sem_info: _,
            strictness: _,
            is_method_definition: _,
            decorations: _,
        } = inner;

        let mut need_sep = false;
        if r#async.get() {
            out!(self, "async");
            if self.force_async_arrow_space() || self.pretty() == Pretty::Yes {
                // Force a space to work with certain transforms that match on
                // `async` followed by whitespace to detect async functions.
                self.space(ForceSpace::Yes);
            } else {
                need_sep = true;
            }
        }
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
            need_sep = false;
        }

        // Single parameter without type info doesn't need parens. But only
        // in expression mode, otherwise it is ugly. "Exactly one element" is
        // located by peeking two items off the iterator rather than a
        // `NodeList::len()`/`::head()` this crate's `NodeList` doesn't have
        // (module doc comment).
        let mut params_iter = params.iter();
        let sole_param = match (params_iter.next(), params_iter.next()) {
            (Some(p), None) => Some(p),
            _ => None,
        };
        let sole_param_is_plain_identifier = matches!(
            sole_param,
            Some(Node::Identifier(Identifier {
                type_annotation: None,
                optional,
                ..
            })) if !optional.get()
        );
        let single_param_no_parens = type_parameters.is_none()
            && return_type.is_none()
            && predicate.is_none()
            && sole_param_is_plain_identifier
            && (expression.get() || self.pretty() == Pretty::No);
        if single_param_no_parens {
            if need_sep {
                out!(self, " ");
            }
            self.gen_node(
                ctx,
                sole_param.expect("single_param_no_parens implies sole_param is Some"),
                Some(Path::new(node, NodeField::params)),
            )?;
        } else {
            out!(self, "(");
            for (i, param) in params.iter().enumerate() {
                if i > 0 {
                    self.comma();
                }
                self.gen_node(ctx, param, Some(Path::new(node, NodeField::params)))?;
            }
            out!(self, ")");
        }
        if return_type.is_some() || predicate.is_some() {
            out!(self, ":");
        }
        if let Some(return_type) = return_type {
            self.space(ForceSpace::No);
            self.print_child(
                ctx,
                Some(*return_type),
                Path::new(node, NodeField::return_type),
                ChildPos::Anywhere,
            )?;
        }
        if let Some(predicate) = predicate {
            self.space(ForceSpace::Yes);
            self.gen_node(ctx, predicate, Some(Path::new(node, NodeField::predicate)))?;
        }
        self.space(ForceSpace::No);
        self.space_before_equals("=>");
        out!(self, "=>");
        self.space(ForceSpace::No);
        match body {
            Node::BlockStatement(_) => {
                self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))?;
            }
            _ => {
                self.print_child(
                    ctx,
                    Some(*body),
                    Path::new(node, NodeField::body),
                    ChildPos::Right,
                )?;
            }
        }
        Ok(())
    }

    /// Shared printing logic for `ClassExpression` and `ClassDeclaration` —
    /// juno matches both kinds in one arm (`gen_js.rs:1553-1611`) since
    /// their field sets are identical; see [`GenJS::gen_function_like`]'s
    /// doc comment for why this crate splits it into two thin wrappers
    /// instead.
    ///
    /// juno `gen_js.rs:1553-1611`. See the module doc comment for why the
    /// decorator-vs-no-decorator branch collapses to one unconditional loop
    /// here.
    #[allow(clippy::too_many_arguments)]
    fn gen_class_like<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        id: Option<&'gc Node<'gc>>,
        type_parameters: Option<&'gc Node<'gc>>,
        super_class: Option<&'gc Node<'gc>>,
        super_type_arguments: Option<&'gc Node<'gc>>,
        implements: NodeList<'gc>,
        decorators: NodeList<'gc>,
        body: &'gc Node<'gc>,
    ) -> Result<(), GenJsError> {
        for decorator in decorators.iter() {
            self.gen_node(ctx, decorator, Some(Path::new(node, NodeField::decorators)))?;
            self.force_newline();
        }
        out!(self, "class");
        if let Some(id) = id {
            self.space(ForceSpace::Yes);
            self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        }
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        if let Some(super_class) = super_class {
            out!(self, " extends ");
            // `print_child`, not `gen_node` (Task 12 review round 5): the
            // heritage slot is `ClassHeritage : extends
            // LeftHandSideExpression`, a strictly narrower tier than the
            // full expression grammar (`crates/parser/src/js/classes.rs:437-438`
            // calls `parse_left_hand_side_expression`, not
            // `parse_assignment_expression`), so a looser expression reaches
            // this field only through explicit source parens. Printed bare,
            // `class C extends (a = b) {}` and its siblings emit source that
            // fails to reparse, and `class C extends (R {p: 1}) {}` corrupts
            // silently. The threshold lives in `need_parens`'s dedicated
            // `super_class` branch (`precedence.rs`), which is where the
            // full grammar evidence is recorded; `ChildPos::Anywhere`
            // because that branch's decision does not depend on position.
            self.print_child(
                ctx,
                Some(super_class),
                Path::new(node, NodeField::super_class),
                ChildPos::Anywhere,
            )?;
        }
        if let Some(super_type_arguments) = super_type_arguments {
            self.gen_node(
                ctx,
                super_type_arguments,
                Some(Path::new(node, NodeField::super_type_arguments)),
            )?;
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

    /// `ClassExpression`: see [`GenJS::gen_class_like`].
    ///
    /// juno `gen_js.rs:1553-1611`.
    pub(crate) fn gen_class_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ClassExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let ClassExpression {
            metadata: _,
            id,
            type_parameters,
            super_class,
            super_type_arguments,
            implements,
            decorators,
            body,
            scope: _,
            implicit_ctor_function_info: _,
            instance_elements_init_function_info: _,
            static_elements_init_function_info: _,
        } = inner;
        self.gen_class_like(
            ctx,
            node,
            *id,
            *type_parameters,
            *super_class,
            *super_type_arguments,
            *implements,
            *decorators,
            body,
        )
    }

    /// `ClassDeclaration`: see [`GenJS::gen_class_like`].
    ///
    /// juno `gen_js.rs:1553-1611`.
    pub(crate) fn gen_class_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ClassDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let ClassDeclaration {
            metadata: _,
            id,
            type_parameters,
            super_class,
            super_type_arguments,
            implements,
            decorators,
            body,
            scope: _,
            implicit_ctor_function_info: _,
            instance_elements_init_function_info: _,
            static_elements_init_function_info: _,
        } = inner;
        self.gen_class_like(
            ctx,
            node,
            *id,
            *type_parameters,
            *super_class,
            *super_type_arguments,
            *implements,
            *decorators,
            body,
        )
    }

    /// `ClassBody`: `{ member0\n member1\n ... }`, or `{}` when empty.
    ///
    /// juno `gen_js.rs:1613-1630`. Ported as-is, including juno's slightly
    /// unusual indent bookkeeping: the closing `}` is printed *before*
    /// `dec_indent`/the trailing `newline`, so — in pretty mode — it lands
    /// at the body's own (inner) indent column rather than aligned with the
    /// opening `class`/`{`. Confirmed against the sibling C++ generator
    /// (`lib/AST2JS/AST2JS.cpp`'s `ClassBodyNode` visitor), which does the
    /// more conventional `dec_indent`-then-`newline`-then-`}`. This is a
    /// cosmetic pretty-mode formatting difference, not a round-trip hazard
    /// (the byte sequence is still syntactically valid either way, and
    /// spec §7 makes round-trip correctness, not output formatting, this
    /// crate's bar), so it is transcribed rather than "fixed" to match C++.
    pub(crate) fn gen_class_body<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ClassBody<'gc>,
    ) -> Result<(), GenJsError> {
        let ClassBody { metadata: _, body } = inner;
        if body.is_empty() {
            out!(self, "{{}}");
        } else {
            out!(self, "{{");
            self.inc_indent();
            self.newline();
            for prop in body.iter() {
                self.gen_node(ctx, prop, Some(Path::new(node, NodeField::body)))?;
                self.newline();
            }
            out!(self, "}}");
            self.dec_indent();
            self.newline();
        }
        Ok(())
    }

    /// `ClassProperty`: `[declare] [static] [variance] [computed key] [?]
    /// [: type_annotation] [= value];`.
    ///
    /// juno `gen_js.rs:1631-1668` (guarded there on `ts_modifiers: None`;
    /// see the module doc comment for why this arm checks `.is_some()`
    /// instead, and for the new `decorators` field juno's version has no
    /// counterpart for).
    pub(crate) fn gen_class_property<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ClassProperty<'gc>,
    ) -> Result<(), GenJsError> {
        let ClassProperty {
            metadata: _,
            key,
            value,
            computed,
            r#static,
            decorators,
            declare,
            optional,
            variance,
            type_annotation,
            ts_modifiers,
        } = inner;
        let ts_modifiers = ts_modifiers_of(*ts_modifiers)?;
        for decorator in decorators.iter() {
            self.gen_node(ctx, decorator, Some(Path::new(node, NodeField::decorators)))?;
            self.force_newline();
        }
        if declare.get() {
            out!(self, "declare ");
        }
        if let Some(m) = ts_modifiers {
            self.print_ts_modifiers_accessibility(ctx, m.accessibility.get());
        }
        if r#static.get() {
            out!(self, "static ");
        }
        if let Some(m) = ts_modifiers {
            self.print_ts_modifiers_readonly(m.readonly.get());
        }
        if let Some(variance) = variance {
            self.gen_node(ctx, variance, Some(Path::new(node, NodeField::variance)))?;
        }
        if computed.get() {
            out!(self, "[");
        }
        self.gen_node(ctx, key, Some(Path::new(node, NodeField::key)))?;
        if computed.get() {
            out!(self, "]");
        }
        if optional.get() {
            out!(self, "?");
        }
        if let Some(type_annotation) = type_annotation {
            out!(self, ":");
            self.space(ForceSpace::No);
            self.gen_node(
                ctx,
                type_annotation,
                Some(Path::new(node, NodeField::type_annotation)),
            )?;
        }
        if let Some(value) = value {
            self.space(ForceSpace::No);
            self.space_before_equals("=");
            out!(self, "=");
            self.space(ForceSpace::No);
            self.gen_node(ctx, value, Some(Path::new(node, NodeField::value)))?;
        }
        out!(self, ";");
        Ok(())
    }

    /// `ClassPrivateProperty`: `[declare] [static] [variance] #key [?]
    /// [: type_annotation] [= value];`.
    ///
    /// juno `gen_js.rs:1669-1717`. See the module doc comment's "juno
    /// correctness bug found and fixed here" section: this arm prints
    /// `declare` as `"declare "` (juno prints `"static "` a second time)
    /// and orders it before `static`, not after.
    ///
    /// `key` is a bare `Identifier`, not a `PrivateName`-wrapped one — our
    /// parser strips the `#` into the identifier itself for this kind
    /// (`crates/parser/src/js/classes.rs:1044-1046`'s comment: "The inner
    /// Identifier holds the private name (#-stripped)"), unlike
    /// `MethodDefinition`'s private key, which the parser wraps in a real
    /// `PrivateName` (`classes.rs:1289-1296`) — unrelated to `# `s
    /// printed by [`GenJS::gen_method_definition`] letting the wrapped
    /// `PrivateName` print its own `#` (`arms/literal.rs`'s
    /// `gen_private_name`). This arm's explicit `out!(self, "#")` before the
    /// key matches juno exactly, because juno's `key`/parser split is the
    /// same.
    pub(crate) fn gen_class_private_property<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ClassPrivateProperty<'gc>,
    ) -> Result<(), GenJsError> {
        let ClassPrivateProperty {
            metadata: _,
            key,
            value,
            r#static,
            decorators,
            declare,
            optional,
            variance,
            type_annotation,
            ts_modifiers,
        } = inner;
        let ts_modifiers = ts_modifiers_of(*ts_modifiers)?;
        for decorator in decorators.iter() {
            self.gen_node(ctx, decorator, Some(Path::new(node, NodeField::decorators)))?;
            self.force_newline();
        }
        if let Some(variance) = variance {
            self.gen_node(ctx, variance, Some(Path::new(node, NodeField::variance)))?;
        }
        // Bug fix (module doc comment): `declare` before `static`, printed
        // as `"declare "`, not a second `"static "`.
        if declare.get() {
            out!(self, "declare ");
        }
        // A private name can never carry an accessibility modifier — the
        // parser rejects the combination outright ("An accessibility
        // modifier cannot be used with a private identifier",
        // `crates/parser/src/js/classes.rs`) and stores
        // `INVALID_ATOM_BYTES` here — but this prints the field rather than
        // assuming, so a hand-built node does not silently lose it.
        if let Some(m) = ts_modifiers {
            self.print_ts_modifiers_accessibility(ctx, m.accessibility.get());
        }
        if r#static.get() {
            out!(self, "static ");
        }
        if let Some(m) = ts_modifiers {
            self.print_ts_modifiers_readonly(m.readonly.get());
        }
        out!(self, "#");
        self.gen_node(ctx, key, Some(Path::new(node, NodeField::key)))?;
        if optional.get() {
            out!(self, "?");
        }
        if let Some(type_annotation) = type_annotation {
            out!(self, ":");
            self.space(ForceSpace::No);
            self.gen_node(
                ctx,
                type_annotation,
                Some(Path::new(node, NodeField::type_annotation)),
            )?;
        }
        self.space(ForceSpace::No);
        if let Some(value) = value {
            self.space_before_equals("=");
            out!(self, "=");
            self.space(ForceSpace::No);
            self.gen_node(ctx, value, Some(Path::new(node, NodeField::value)))?;
        }
        out!(self, ";");
        Ok(())
    }

    /// `MethodDefinition`: `[static] [async] [*] [get|set ]key(params) [:
    /// return_type] body`. `kind == Constructor` prints no extra keyword —
    /// the `key` (always the literal `constructor`) already says it.
    ///
    /// juno `gen_js.rs:1718-1795`. See the module doc comment for the
    /// `visit_func_params_body` trailing-`node`-argument deviation (follows
    /// `Property`'s call, not juno's own `MethodDefinition` call) and for
    /// the new `decorators` field juno's version has no counterpart for.
    /// juno's `Node::FunctionExpression(...) => (...) ` / `_ =>
    /// unreachable!("Invalid method value")` (`gen_js.rs:1727-1751`) becomes
    /// `_ => Err(GenJsError::UnsupportedKind(...))`, the same
    /// `unreachable!()` → `GenJsError` substitution `arms/expr.rs`'s
    /// `Property` arm already made for the identical shape (spec §4).
    pub(crate) fn gen_method_definition<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MethodDefinition<'gc>,
    ) -> Result<(), GenJsError> {
        let MethodDefinition {
            metadata: _,
            key,
            value,
            kind,
            computed,
            r#static,
            decorators,
        } = inner;
        let kind = MethodDefinitionKind::from_label(ctx, kind.get())?;
        let (is_async, generator, params, body, return_type, predicate, type_parameters) =
            match value {
                Node::FunctionExpression(FunctionExpression {
                    metadata: _,
                    // Name is handled by the property key.
                    id: _,
                    generator,
                    r#async,
                    params,
                    body,
                    return_type,
                    predicate,
                    type_parameters,
                    scope: _,
                    sem_info: _,
                    strictness: _,
                    is_method_definition: _,
                    decorations: _,
                }) => (
                    r#async.get(),
                    generator.get(),
                    *params,
                    *body,
                    *return_type,
                    *predicate,
                    *type_parameters,
                ),
                _ => return Err(GenJsError::UnsupportedKind(value.kind())),
            };

        for decorator in decorators.iter() {
            self.gen_node(ctx, decorator, Some(Path::new(node, NodeField::decorators)))?;
            self.force_newline();
        }
        if r#static.get() {
            out!(self, "static ");
        }
        if is_async {
            out!(self, "async ");
        }
        if generator {
            out!(self, "*");
        }
        match kind {
            MethodDefinitionKind::Method | MethodDefinitionKind::Constructor => {}
            MethodDefinitionKind::Get => out!(self, "get "),
            MethodDefinitionKind::Set => out!(self, "set "),
        }
        if computed.get() {
            out!(self, "[");
        }
        self.gen_node(ctx, key, Some(Path::new(node, NodeField::key)))?;
        if computed.get() {
            out!(self, "]");
        }
        // Deviation from juno's own call here: pass `*value` (the
        // `FunctionExpression`), not `node` (the `MethodDefinition`) — see
        // the module doc comment's last section.
        self.visit_func_params_body(
            ctx,
            params,
            type_parameters,
            return_type,
            predicate,
            body,
            value,
        )
    }

    /// Print a Flow function type's `(params)` — used by
    /// `FunctionTypeAnnotation` (`arms/flow_type.rs`'s
    /// `GenJS::gen_function_type_annotation`, Task 10), with an optional
    /// `this:` parameter and `...rest`.
    ///
    /// juno `gen_js.rs:3401-3452`. Listed under this task's own "Produces"
    /// in the brief, so it was ported here (Task 7) rather than left for a
    /// forward reference in Task 10.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn visit_func_type_params<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        params: NodeList<'gc>,
        this: Option<&'gc Node<'gc>>,
        rest: Option<&'gc Node<'gc>>,
        type_parameters: Option<&'gc Node<'gc>>,
        node: &'gc Node<'gc>,
    ) -> Result<(), GenJsError> {
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        out!(self, "(");
        let mut need_comma = false;
        if let Some(this) = this {
            match this {
                Node::FunctionTypeParam(FunctionTypeParam {
                    metadata: _,
                    type_annotation,
                    name: _,
                    optional: _,
                }) => {
                    out!(self, "this:");
                    self.space(ForceSpace::No);
                    self.gen_node(
                        ctx,
                        type_annotation,
                        Some(Path::new(node, NodeField::type_annotation)),
                    )?;
                }
                _ => return self.unsupported_kind(this),
            }
            need_comma = true;
        }
        for param in params.iter() {
            if need_comma {
                self.comma();
            }
            self.gen_node(ctx, param, Some(Path::new(node, NodeField::param)))?;
            need_comma = true;
        }
        if let Some(rest) = rest {
            if need_comma {
                self.comma();
            }
            out!(self, "...");
            self.gen_node(ctx, rest, Some(Path::new(node, NodeField::rest)))?;
        }
        out!(self, ")");
        Ok(())
    }
}

/// Unwrap a class member's `ts_modifiers` field into the `TSModifiers` it
/// must hold.
///
/// Task 13. Under `-parse-ts` this field is **always** `Some`:
/// `crates/parser/src/js/classes.rs` builds a `TSModifiers` at both the
/// `ClassPrivateProperty` and the `ClassProperty` construction site whenever
/// `parse_ts()` is set — a plain `class C { x = 1; }` gets
/// `accessibility: null, readonly: false`. Under Flow or plain JS it is
/// always `None`. Tasks 7-12 bailed out with `UnsupportedKind` on `Some`,
/// which made every TypeScript class *with a property field* ungeneratable;
/// the two arms above now print the two halves either side of `static` (see
/// `arms/ts.rs`'s module doc comment for why they cannot be printed as one
/// unit).
///
/// A `Some` holding something other than a `TSModifiers` can only come from
/// a hand-built tree. It is reported as `UnsupportedKind` naming **the
/// offending child's** kind, not the enclosing class member's — review round
/// 1 finding M-5: the original spelling reported `ClassProperty`/
/// `ClassPrivateProperty`, kinds that are perfectly supported, pointing a
/// reader at the wrong node. Every other `UnsupportedKind` site in the crate
/// names the node that actually could not be printed.
fn ts_modifiers_of<'gc>(
    ts_modifiers: Option<&'gc Node<'gc>>,
) -> Result<Option<&'gc TSModifiers<'gc>>, GenJsError> {
    match ts_modifiers {
        None => Ok(None),
        Some(Node::TSModifiers(m)) => Ok(Some(m)),
        Some(other) => Err(GenJsError::UnsupportedKind(other.kind())),
    }
}
