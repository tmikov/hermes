/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! `import`/`export` declarations: `ImportDeclaration`, `ImportSpecifier`,
//! `ImportDefaultSpecifier`, `ImportNamespaceSpecifier`, `ImportAttribute`,
//! `ExportNamedDeclaration`, `ExportSpecifier`, `ExportNamespaceSpecifier`,
//! `ExportDefaultDeclaration`, `ExportAllDeclaration`.
//!
//! Ported from juno `gen_js.rs:1796-1941`. This is the plan's Task 8.
//!
//! # Adaptations specific to this module
//!
//! **`import_kind`/`export_kind` print through `ctx.try_bytes_str`, not a
//! typed enum.** juno's fields are a typed `ImportKind`/`ExportKind` enum
//! (`"value"`/`"type"`/`"typeof"`), read back with `.as_str()`; ours are a
//! raw `Cell<NodeLabel>` atom, the same substitution `arms/stmt.rs`'s
//! `gen_variable_declaration` already made for `VariableDeclaration::kind`
//! (see that module's doc comment for why: the value set is not provably
//! fixed at the type level, so there is nothing to classify into an enum —
//! only to print, or not print, verbatim). [`GenJS::gen_import_export_kind_prefix`]
//! is the one shared helper, used by all four fields that carry this kind of
//! label (`ImportDeclaration`/`ImportSpecifier`'s `import_kind`,
//! `ExportNamedDeclaration`/`ExportAllDeclaration`'s `export_kind`).
//!
//! **`ImportDeclaration`'s `assertions: Option<NodeList>` is `attributes:
//! NodeList`, not `Option`.** Ours has no "the whole clause is absent"
//! `Option` layer to check — an absent `with { ... }` clause and an empty
//! one both parse to an empty `NodeList` (`crates/parser/src/js/modules.rs`'s
//! `parse_import_declaration`, which always constructs `NodeList::from_iter`
//! over whatever `parse_with_clause` collected, defaulting to
//! `Vec::new()`), so this arm checks `!attributes.is_empty()` instead of
//! matching `Some`.
//!
//! **DEVIATION from juno — a correctness fix, not a transcription: the
//! import-attributes keyword is `with`, not `assert`.** juno prints
//! `" assert {{"` (`gen_js.rs:1832`, the older, now-superseded
//! [TC39 import-assertions proposal](https://github.com/tc39/proposal-import-assertions)
//! spelling). Our parser (`crates/parser/src/js/modules.rs`'s
//! `parse_with_clause`, gated on `TokenKind::rw_with`) only recognizes the
//! current [import-attributes proposal](https://github.com/tc39/proposal-import-attributes)'s
//! `with` keyword — confirmed against the lexer's own reserved-word table
//! (`crates/parser/src/token_kinds.rs`: `rw_with` exists, mapped from
//! `b"with"`; no `assert`-spelled token exists anywhere in it). Printing
//! `assert` would produce source our own parser rejects outright (`with` is
//! the *only* spelling `parse_import_declaration`/`parse_with_clause` ever
//! check for), a live round-trip break for every attributed import — not a
//! stylistic choice. This arm prints `" with {{"` instead. The task brief's
//! own required test (an import attribute, `with { type: "json" }`) is
//! exactly this case.
//!
//! **DEVIATION from juno — a correctness fix, not a transcription:
//! `ExportNamedDeclaration`'s `export * as ns from 'm'` form must not be
//! wrapped in `{ ... }`.** juno's arm (`gen_js.rs:1878-1905`) has exactly
//! one printing path for `declaration: None`: wrap `specifiers` in `{`/`}`
//! unconditionally, regardless of what kind of node each specifier is. But
//! `specifiers` can hold a *single* `ExportNamespaceSpecifier` instead of an
//! `ExportSpecifier` list — ECMA-262's `ExportDeclaration : export * as
//! ModuleExportName from ModuleSpecifier ;` is an entirely separate grammar
//! production from `export ExportClause FromClause ;`, and our parser
//! builds exactly this shape for it (`crates/parser/src/js/modules.rs`'s
//! `parse_export_declaration`, the `export_as` branch: a one-element
//! `NodeList` holding an `ExportNamespaceSpecifier`, `declaration: None`).
//! `ExportNamespaceSpecifier` itself prints as `* as ident`
//! ([`GenJS::gen_export_namespace_specifier`]) — juno's own unconditional
//! wrap around that would emit `export {* as ns} from 'm';`, which is not
//! merely wrong output but a straight syntax error (`*` cannot appear
//! inside an `ExportsList`), so this is a live round-trip break, not a
//! cosmetic one. This arm special-cases "exactly one specifier and it's an
//! `ExportNamespaceSpecifier`" to print it bare, with no surrounding braces
//! and no `export_kind` prefix (the `export * as ns from 'm'` production has
//! no kind slot in the grammar at all, and our parser's `export_as` branch
//! always writes the `"value"` label there anyway — `modules.rs`'s
//! `export_as` branch), falling back to juno's brace-wrapped loop for every
//! other shape (an ordinary `ExportSpecifier` list, empty or not). The task
//! brief's own required test (`export * as ns from`) is exactly this case.
//!
//! **`ExportDefaultDeclaration`'s `declaration` now prints through
//! `print_child`, not a bare `gen_node` call — a correctness fix living
//! mostly in `precedence.rs`, referenced here.** juno's arm
//! (`gen_js.rs:1922-1928`) is a bare `declaration.visit(...)`, and juno's own
//! `need_parens` (`gen_js.rs:3685-3822`) has no `ExportDefaultDeclaration`
//! branch at all — so a `declaration` that is a bare `FunctionExpression` or
//! `ClassExpression` (from source like `export default (function () {})`)
//! prints unparenthesized as `export default function(){}`, which reparses
//! under ECMA-262's own `ExportDeclaration : export default [lookahead ∉
//! {function, async function, class}] AssignmentExpression ;` production as
//! a `FunctionDeclaration`, not a `FunctionExpression` — a genuine node-kind
//! flip on reparse, not merely a formatting difference. See
//! `precedence.rs`'s new `ExportDefaultDeclaration` branch of `need_parens`
//! for the fix and the full accounting; this arm's only change is routing
//! through `print_child` (`ChildPos::Anywhere`, matching
//! `arms/stmt.rs`'s `gen_expression_statement`'s identical-shaped call) so
//! that branch actually runs. The task brief's own required test
//! (`export default function(){}` vs `export default (function(){})`) is
//! exactly this case.
//!
//! **`ImportSpecifier`/`ExportSpecifier` always print `local as
//! imported`/`local as exported`, even when the two names are identical.**
//! Ported verbatim from juno (`gen_js.rs:1847-1859`, `1906-1914`), which
//! does the same — not a bug: `import {a as a} from 'm'` and `import {a}
//! from 'm'` are semantically identical, so this is at most non-minimal
//! output, not a round-trip hazard (spec §7 makes round-trip correctness,
//! not minimal output, this crate's bar).

use hermes_ast::context::GCLock;
use hermes_ast::node::{
    ExportAllDeclaration, ExportDefaultDeclaration, ExportNamedDeclaration,
    ExportNamespaceSpecifier, ExportSpecifier, ImportAttribute, ImportDeclaration,
    ImportDefaultSpecifier, ImportNamespaceSpecifier, ImportSpecifier, Node, NodeField,
};
use hermes_ast::node_child::NodeLabel;
use hermes_ast::visitor::Path;

use crate::precedence::{ChildPos, ForceSpace};
use crate::{out, GenJS, GenJsError};

impl<'s, 'w> GenJS<'s, 'w> {
    /// Print `kind`'s spelling followed by a space, unless it is the default
    /// `"value"` spelling — shared by `ImportDeclaration`/`ImportSpecifier`'s
    /// `import_kind` and `ExportNamedDeclaration`/`ExportAllDeclaration`'s
    /// `export_kind`. See the module doc comment's first adaptation for why
    /// this reads the raw label rather than classifying into an enum.
    fn gen_import_export_kind_prefix(
        &mut self,
        ctx: &GCLock<'_, '_>,
        kind: NodeLabel,
    ) -> Result<(), GenJsError> {
        let spelling = ctx
            .try_bytes_str(kind)
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        if spelling != "value" {
            out!(self, "{} ", spelling);
        }
        Ok(())
    }

    /// `ImportDeclaration`: `import [kind ]specifiers from source[ with {
    /// attributes }]`, or just `import source[ with { attributes }]` when
    /// there are no specifiers at all (`import 'x';`).
    ///
    /// juno `gen_js.rs:1796-1846`. See the module doc comment for the
    /// `assertions`→`attributes` field-shape adaptation and the `assert`→
    /// `with` keyword fix.
    pub(crate) fn gen_import_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ImportDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let ImportDeclaration {
            metadata: _,
            specifiers,
            source,
            attributes,
            import_kind,
        } = inner;
        out!(self, "import ");
        self.gen_import_export_kind_prefix(ctx, import_kind.get())?;
        let mut has_named_specs = false;
        for (i, spec) in specifiers.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            if matches!(spec, Node::ImportSpecifier(_)) && !has_named_specs {
                has_named_specs = true;
                out!(self, "{{");
            }
            self.gen_node(ctx, spec, Some(Path::new(node, NodeField::specifiers)))?;
        }
        if !specifiers.is_empty() {
            if has_named_specs {
                out!(self, "}}");
                self.space(ForceSpace::No);
            } else {
                out!(self, " ");
            }
            out!(self, "from ");
        }
        self.gen_node(ctx, source, Some(Path::new(node, NodeField::source)))?;
        if !attributes.is_empty() {
            out!(self, " with {{");
            for (i, attribute) in attributes.iter().enumerate() {
                if i > 0 {
                    self.comma();
                }
                self.gen_node(ctx, attribute, Some(Path::new(node, NodeField::attributes)))?;
            }
            out!(self, "}}");
        }
        Ok(())
    }

    /// `ImportSpecifier`: `[kind ]imported as local`.
    ///
    /// juno `gen_js.rs:1847-1859`.
    pub(crate) fn gen_import_specifier<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ImportSpecifier<'gc>,
    ) -> Result<(), GenJsError> {
        let ImportSpecifier {
            metadata: _,
            imported,
            local,
            import_kind,
        } = inner;
        self.gen_import_export_kind_prefix(ctx, import_kind.get())?;
        self.gen_node(ctx, imported, Some(Path::new(node, NodeField::imported)))?;
        out!(self, " as ");
        self.gen_node(ctx, local, Some(Path::new(node, NodeField::local)))
    }

    /// `ImportDefaultSpecifier`: just `local`.
    ///
    /// juno `gen_js.rs:1860-1862`.
    pub(crate) fn gen_import_default_specifier<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ImportDefaultSpecifier<'gc>,
    ) -> Result<(), GenJsError> {
        let ImportDefaultSpecifier { metadata: _, local } = inner;
        self.gen_node(ctx, local, Some(Path::new(node, NodeField::local)))
    }

    /// `ImportNamespaceSpecifier`: `* as local`.
    ///
    /// juno `gen_js.rs:1863-1866`.
    pub(crate) fn gen_import_namespace_specifier<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ImportNamespaceSpecifier<'gc>,
    ) -> Result<(), GenJsError> {
        let ImportNamespaceSpecifier { metadata: _, local } = inner;
        out!(self, "* as ");
        self.gen_node(ctx, local, Some(Path::new(node, NodeField::local)))
    }

    /// `ImportAttribute`: `key: value`, one entry of an `import ... with {
    /// ... }` clause.
    ///
    /// juno `gen_js.rs:1867-1877`.
    pub(crate) fn gen_import_attribute<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ImportAttribute<'gc>,
    ) -> Result<(), GenJsError> {
        let ImportAttribute {
            metadata: _,
            key,
            value,
        } = inner;
        self.gen_node(ctx, key, Some(Path::new(node, NodeField::key)))?;
        out!(self, ":");
        self.space(ForceSpace::No);
        self.gen_node(ctx, value, Some(Path::new(node, NodeField::value)))
    }

    /// `ExportNamedDeclaration`: `export declaration`, `export * as ns from
    /// source` (bare, no braces), or `export [kind ]{specifiers}[ from
    /// source]`.
    ///
    /// juno `gen_js.rs:1878-1905`. See the module doc comment for the
    /// `export * as ns from` bug fix — the reason this arm branches on
    /// `specifiers`' shape instead of always wrapping it in `{ ... }`.
    pub(crate) fn gen_export_named_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ExportNamedDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let ExportNamedDeclaration {
            metadata: _,
            declaration,
            specifiers,
            source,
            export_kind,
        } = inner;
        out!(self, "export ");
        if let Some(declaration) = declaration {
            return self.gen_node(ctx, declaration, Some(Path::new(node, NodeField::declaration)));
        }
        let mut iter = specifiers.iter();
        let first = iter.next();
        let second = iter.next();
        match (first, second) {
            (Some(spec @ Node::ExportNamespaceSpecifier(_)), None) => {
                // `export * as ns from 'm'` — bare, no braces, no kind
                // prefix (module doc comment's bug-fix section).
                self.gen_node(ctx, spec, Some(Path::new(node, NodeField::specifiers)))?;
            }
            _ => {
                self.gen_import_export_kind_prefix(ctx, export_kind.get())?;
                out!(self, "{{");
                for (i, spec) in specifiers.iter().enumerate() {
                    if i > 0 {
                        self.comma();
                    }
                    self.gen_node(ctx, spec, Some(Path::new(node, NodeField::specifiers)))?;
                }
                out!(self, "}}");
            }
        }
        if let Some(source) = source {
            out!(self, " from ");
            self.gen_node(ctx, source, Some(Path::new(node, NodeField::source)))?;
        }
        Ok(())
    }

    /// `ExportSpecifier`: `local as exported`.
    ///
    /// juno `gen_js.rs:1906-1914`.
    pub(crate) fn gen_export_specifier<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ExportSpecifier<'gc>,
    ) -> Result<(), GenJsError> {
        let ExportSpecifier {
            metadata: _,
            exported,
            local,
        } = inner;
        self.gen_node(ctx, local, Some(Path::new(node, NodeField::local)))?;
        out!(self, " as ");
        self.gen_node(ctx, exported, Some(Path::new(node, NodeField::exported)))
    }

    /// `ExportNamespaceSpecifier`: `* as exported`.
    ///
    /// juno `gen_js.rs:1915-1921`.
    pub(crate) fn gen_export_namespace_specifier<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ExportNamespaceSpecifier<'gc>,
    ) -> Result<(), GenJsError> {
        let ExportNamespaceSpecifier {
            metadata: _,
            exported,
        } = inner;
        out!(self, "* as ");
        self.gen_node(ctx, exported, Some(Path::new(node, NodeField::exported)))
    }

    /// `ExportDefaultDeclaration`: `export default declaration`.
    ///
    /// juno `gen_js.rs:1922-1928`. See the module doc comment for why
    /// `declaration` now routes through `print_child` — the
    /// `precedence.rs`-side fix that actually adds the parens.
    pub(crate) fn gen_export_default_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ExportDefaultDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let ExportDefaultDeclaration {
            metadata: _,
            declaration,
        } = inner;
        out!(self, "export default ");
        self.print_child(
            ctx,
            Some(*declaration),
            Path::new(node, NodeField::declaration),
            ChildPos::Anywhere,
        )
    }

    /// `ExportAllDeclaration`: `export [kind ]* from source`.
    ///
    /// juno `gen_js.rs:1929-1941`.
    pub(crate) fn gen_export_all_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ExportAllDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let ExportAllDeclaration {
            metadata: _,
            source,
            export_kind,
        } = inner;
        out!(self, "export ");
        self.gen_import_export_kind_prefix(ctx, export_kind.get())?;
        out!(self, "* from ");
        self.gen_node(ctx, source, Some(Path::new(node, NodeField::source)))
    }
}
