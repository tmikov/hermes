/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! S4a T3: the four ES-module declaration visits. A further `impl<'bt, 'sc,
//! 'sm, 'ad> SemanticResolver<'bt, 'sc, 'sm, 'ad>` block, split out of
//! `resolver/mod.rs` the same way `identifiers.rs` (S1 T4),
//! `declarations.rs` (S1 T5), `expressions.rs` (S1 T6), `functions.rs`
//! (S1 T7), `statements.rs` (S2 T1), `classes.rs` (S2 T4) and `calls.rs`
//! (S2 T6) were — see `identifiers.rs`'s module doc for why a child module
//! sees `mod.rs`'s private fields and helpers.
//!
//! Ports `SemanticResolver::visit(ImportDeclarationNode *)`
//! (SemanticResolver.cpp:874-890),
//! `visit(ExportNamedDeclarationNode *)` (cpp:1524-1531),
//! `visit(ExportDefaultDeclarationNode *)` (cpp:1533-1561) and
//! `visit(ExportAllDeclarationNode *)` (cpp:1563-1568).
//!
//! ## No CommonJS-module support in this port
//!
//! All four visits are gated on `astContext_.getUseCJSModules()` — the
//! `-commonjs` driver flag. This port does not implement `-commonjs`
//! anywhere (neither the flag, nor `Context::useCJSModules`, nor the
//! `$SHBuiltin` module protocol `calls.rs:310-341` still panics on), so
//! `getUseCJSModules()` is CONSTANTLY FALSE here and every `!useCJSModules`
//! sub-condition folds away to `true`. The Rust gates are therefore
//! `self.compile()` alone (or nothing at all, for the import visit — see
//! below), each marked `// S4b: && !use_cjs_modules` at the site so that
//! whoever adds CJS support has the exact condition to restore.
//!
//! ## One bug-for-bug quirk, preserved and flagged
//!
//! 1. **The import error is NOT `compile_`-gated** (cpp:876-879), while all
//!    three export errors ARE (cpp:1525, 1534, 1564). So under the
//!    `compile = false` entry point (`resolveASTForParser`,
//!    `resolve::resolve_ast_for_parser`) an `import` still errors and an
//!    `export` does not — an asymmetry with no stated rationale in the C++.
//!    Pinned from BOTH sides by the parser-entry corpus:
//!    `module-imports.js` (errors under `compile = false`) and
//!    `compile-false-basics.js` (does not).
//!
//! Two further quirks this port used to pin have since been FIXED upstream
//! and the fixes mirrored here: rewrite #4 dropping the `async` flag
//! (`6b59daf0d`) and `ExportAllDeclaration` spelling its error "CommonJS
//! module mode" while the other two say plain "module mode" (`f90a83146`).
//!
//! ## Rewrite #4: `export default function () {}` (spec §3.4)
//!
//! `visit(ExportDefaultDeclarationNode *)` mutates its own child in place
//! (`node->_declaration = funcExpr;`, cpp:1557) and only then runs
//! `visitESTreeChildren` over the mutated node. Structural fields are
//! immutable in this port, so — exactly like rewrite #1 (the arrow's
//! expression body, `functions.rs`) and rewrite #3 (`$SHBuiltin.prop(...)`,
//! `calls.rs`) — the rewrite instead FUNCTIONALLY REBUILDS both nodes
//! through the generated `builder`: a fresh `FunctionExpression` carrying
//! the declaration's seven structural children, then a fresh
//! `ExportDefaultDeclaration` pointing at it. The children walk then runs on
//! the REBUILT export node, which is what C++'s runs on too, and the visit
//! reports `Changed` even when that walk itself reports `Unchanged`
//! (`export default function () {}` — nothing below the rewrite changes),
//! the same tail `calls::visit_call_expression` needs for rewrite #3.
//!
//! No decorate-before-recurse exception here (`resolver/mod.rs`'s module
//! doc): the visit writes no `Cell` of its own. `strictness` (cpp:1553) is a
//! `Cell` COPY off the declaration made while building the replacement,
//! before anything recurses — and it is `Strictness::NotSet` at that point
//! either way, since `visitFunctionLike` is what eventually sets it, on the
//! rebuilt node.
//!
//! `-commonjs` is where this rewrite is actually observable in a dump (it is
//! what makes `test/AST/es6/export-default-function.js`'s
//! `-dump-transformed-ast -commonjs` print `"type": "FunctionExpression"`),
//! so **S4b owns the `-commonjs` corpus pinning of this rewrite**. Without
//! `-commonjs` the enclosing `export` is an error and `hermesc` never dumps;
//! what the S4a corpus can pin is the parser-entry side
//! (`compile-false-basics.js` — where `compile_` is false, so the rewrite
//! deliberately does NOT fire and the dump shows a `FunctionDeclaration`)
//! plus the unit test
//! `export_default_anonymous_function_is_rewritten_to_an_expression` in
//! `tests/resolver.rs`, which reaches into the returned tree.
//!
//! ## Backref fixup: `FunctionInfo::imports` (spec §3.4 (a))
//!
//! `visit(ImportDeclarationNode *)` pushes the node onto
//! `curFunctionInfo()->imports` (cpp:887) before descending into it. This is
//! the second of the two sema records that keep a `NodeRc` to an INTERIOR
//! node (`LexicalScope::hoisted_functions` is the first, discharged by
//! `functions::visit_function_declaration`), and it takes the same fixup for
//! the same reason: the children walk can rebuild the `ImportDeclaration`,
//! stranding the recorded `NodeRc` on a node no longer in the returned tree.
//! `visit_import_declaration` therefore remembers WHICH `FunctionInfo`'s
//! list it pushed into and at WHICH index (a `Vec::push`, so `len() - 1`;
//! nothing ever removes an entry) and patches that slot when — and only
//! when — the walk returns `Changed`. With that, `resolver/mod.rs`'s
//! §3.4 (a) obligation is fully discharged for both records.
//!
//! The differential cannot catch a missed fixup: `imports` is dump-blind
//! everywhere (`SemContextDumper.cpp` never mentions it, and neither does
//! this port's `dump_context.rs`) — it exists for `ESTreeIRGen`'s CommonJS
//! module lowering, which S4b owns. The unit tests
//! `import_declarations_are_recorded_on_the_function_info` and
//! `import_backref_is_untouched_without_a_rebuild` in `tests/resolver.rs`
//! are what pin it, by comparing the recorded nodes' identity against the
//! `ImportDeclaration`s in the tree the resolver returned.
//!
//! The `Changed` branch of the fixup is DEFENSIVE, not currently
//! exercisable: an `ImportDeclaration`'s children are specifiers (whose own
//! children are `Identifier` leaves), a `StringLiteral` source and
//! `ImportAttribute`s (literal key/value pairs) — nothing this resolver
//! rewrites or folds, so its walk can only report `Unchanged` today. It is
//! written anyway because the obligation is structural rather than
//! shape-specific, and because the alternative (omitting it and relying on
//! a shape argument that a later phase can invalidate silently) is exactly
//! the failure mode §3.4 (a) exists to prevent. What the first test above
//! DOES pin is the neighboring, real case: an ancestor rebuild (a fold
//! elsewhere in the `Program`) leaves the recorded `NodeRc`s pointing at
//! nodes that are still in the returned tree.

use hermes_ast::context::{GCLock, NodeRc};
use hermes_ast::node::{builder, FunctionExpression, Node};
use hermes_ast::visitor::TransformResult;

use super::functions::copy_location_from;
use super::SemanticResolver;

impl<'bt, 'sc, 'sm, 'ad> SemanticResolver<'bt, 'sc, 'sm, 'ad> {
    // ---- visit(ImportDeclarationNode *) --------------------------------

    /// Port of `SemanticResolver::visit(ESTree::ImportDeclarationNode
    /// *importDecl)` (SemanticResolver.cpp:874-890), plus this port's
    /// `FunctionInfo::imports` backref fixup — see the module doc.
    ///
    /// The declared names themselves are NOT introduced here: like C++,
    /// this port hoists them through `DeclCollector::visit(
    /// ImportDeclarationNode *)` (DeclCollector.cpp:124-127 /
    /// `decl_collector.rs`), whose `addToCur` puts the whole declaration in
    /// the enclosing scope's list; `extractIdentsFromDecl`'s
    /// `ImportDeclaration` arm (cpp:2364-2377 /
    /// `declarations::extract_idents_from_decl`) then maps each specifier's
    /// `_local` to a `DeclKind::Import` decl when the scope is processed.
    /// Both sides of that were ported in S1; this visit is what first makes
    /// them corpus-reachable — verified against the C++ arm above
    /// (`ImportSpecifier`/`ImportDefaultSpecifier`/`ImportNamespaceSpecifier`
    /// `→ _local`, everything else skipped, `Decl::Kind::Import` returned)
    /// and pinned by `sema_corpus_parser/module-imports.js`, whose dump
    /// shows `Decl %d.N '<local>' Import` for all three specifier shapes.
    pub(super) fn visit_import_declaration<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let import_decl = node
            .as_import_declaration()
            .expect("visit_import_declaration: not an ImportDeclaration");

        // Like variable declarations, imported names must be hoisted.
        //
        //   if (!astContext_.getUseCJSModules()) { sm_.error(...); }
        //
        // Deliberately NOT `compile_`-gated, unlike all three export visits
        // below — see the module doc's quirk 1. With no CJS support the
        // condition is constantly true, so nothing guards this at all.
        // S4b: `if !self.use_cjs_modules() { ... }`.
        self.sm.error_range(
            node.range(),
            "'import' statement requires module mode",
        );

        if self.compile() && !import_decl.attributes.is_empty() {
            self.sm.error_range(
                node.range(),
                "import assertions are not supported",
            );
        }

        // curFunctionInfo()->imports.push_back(importDecl);
        let info = self.cur_function_info();
        let imports = &mut self.sem_ctx.function_mut(info).imports;
        imports.push(NodeRc::from_node(gc, node));
        let import_idx = imports.len() - 1;

        // visitESTreeChildren(*this, importDecl);
        let result = node.visit_children_mut(gc, self);

        // Backref fixup (spec §3.4 (a)): the node recorded above is stale
        // exactly when the walk rebuilt it. See the module doc.
        if let TransformResult::Changed(new_node) = &result {
            self.sem_ctx.function_mut(info).imports[import_idx] =
                NodeRc::from_node(gc, new_node);
        }
        result
    }

    // ---- visit(ExportNamedDeclarationNode *) ----------------------------

    /// Port of `SemanticResolver::visit(ESTree::ExportNamedDeclarationNode
    /// *node)` (SemanticResolver.cpp:1524-1531).
    pub(super) fn visit_export_named_declaration<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        // if (compile_ && !astContext_.getUseCJSModules())
        // S4b: `&& !self.use_cjs_modules()`.
        if self.compile() {
            self.sm.error_range(
                node.range(),
                "'export' statement requires module mode",
            );
        }

        // visitESTreeChildren(*this, node);
        node.visit_children_mut(gc, self)
    }

    // ---- visit(ExportDefaultDeclarationNode *) --------------------------

    /// Port of `SemanticResolver::visit(ESTree::ExportDefaultDeclarationNode
    /// *node)` (SemanticResolver.cpp:1533-1561), **rewrite #4** included —
    /// see the module doc for why the rewrite allocates new nodes instead of
    /// mutating this one.
    pub(super) fn visit_export_default_declaration<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let export = node.as_export_default_declaration().expect(
            "visit_export_default_declaration: not an ExportDefaultDeclaration",
        );

        // if (compile_ && !astContext_.getUseCJSModules())
        // S4b: `&& !self.use_cjs_modules()`.
        if self.compile() {
            self.sm.error_range(
                node.range(),
                "'export' statement requires module mode",
            );
        }

        // `rewritten_node` is C++'s `node` after the mutation at cpp:1557:
        // the children walk below reads the declaration off it, exactly as
        // C++'s `visitESTreeChildren(*this, node)` does.
        let mut rewritten = false;
        let mut rewritten_node = node;
        // dyn_cast<FunctionDeclarationNode>(node->_declaration)
        if let Node::FunctionDeclaration(func_decl) = export.declaration {
            if self.compile() && func_decl.id.is_none() {
                // If the default function declaration has no name, then
                // change it to a FunctionExpression node for cleaner IRGen.
                let func_expr = gc.alloc(Node::FunctionExpression(
                    FunctionExpression::new(
                        // funcExpr->copyLocationFrom(funcDecl) (cpp:1554),
                        // hoisted into the constructor — see
                        // `copy_location_from`'s doc.
                        copy_location_from(export.declaration),
                        func_decl.id,
                        // std::move(funcDecl->_params): the C++ move empties
                        // the declaration's list, which is safe there only
                        // because the declaration is being discarded. Here
                        // the list is shared structurally instead — same
                        // resulting tree, and the discarded declaration is
                        // simply unreachable.
                        func_decl.params,
                        func_decl.body,
                        func_decl.type_parameters,
                        func_decl.return_type,
                        func_decl.predicate,
                        func_decl.generator.get(),
                        // `funcDecl->_async` (cpp:1552). This used to be a
                        // literal `false`, so an anonymous `export default
                        // async function () {}` lost its async flag on the
                        // rewritten node; fixed upstream in `6b59daf0d`.
                        func_decl.r#async.get(),
                    ),
                ));
                // funcExpr->strictness = funcDecl->strictness;
                func_expr
                    .as_function_expression()
                    .expect("just allocated a FunctionExpression")
                    .strictness
                    .set(func_decl.strictness.get());

                // node->_declaration = funcExpr;
                let mut b =
                    builder::ExportDefaultDeclaration::from_node(export);
                b.declaration(func_expr);
                rewritten_node = b.build_forced(gc);
                rewritten = true;
            }
        }

        // visitESTreeChildren(*this, node);
        let result = rewritten_node.visit_children_mut(gc, self);
        if rewritten {
            // The rewrite alone makes this `Changed`, even when the walk
            // over the rebuilt node reported `Unchanged` (`export default
            // function () {}` has nothing below it that could change) —
            // same tail as rewrite #3's, `calls::visit_call_expression`.
            return TransformResult::Changed(match result {
                TransformResult::Changed(v) => v,
                TransformResult::Unchanged => rewritten_node,
                other => unreachable!(
                    "the resolver never removes or expands a child: {other:?}"
                ),
            });
        }
        result
    }

    // ---- visit(ExportAllDeclarationNode *) ------------------------------

    /// Port of `SemanticResolver::visit(ESTree::ExportAllDeclarationNode
    /// *node)` (SemanticResolver.cpp:1563-1568).
    pub(super) fn visit_export_all_declaration<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        // if (compile_ && !astContext_.getUseCJSModules())
        // S4b: `&& !self.use_cjs_modules()`.
        if self.compile() {
            // This message used to say "CommonJS module mode" while the two
            // export visits above — same gate, same condition, same phrasing
            // otherwise — said plain "module mode"; unified upstream in
            // `f90a83146`. `module-export-plain.js` exercises all three
            // messages in one file.
            self.sm.error_range(
                node.range(),
                "'export' statement requires module mode",
            );
        }

        // visitESTreeChildren(*this, node);
        node.visit_children_mut(gc, self)
    }
}
