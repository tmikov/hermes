/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! S2 T6: `visit(CallExpressionNode *)` (SemanticResolver.cpp:1127-1219) —
//! the three "call specials" the C++ resolver folds into one override, plus
//! the static helper one of them needs. A sibling `impl<'bt, 'sc, 'sm, 'ad>
//! SemanticResolver<'bt, 'sc, 'sm, 'ad>` block, split into its own file for
//! the same reason `identifiers.rs`/`declarations.rs`/`expressions.rs`/
//! `statements.rs`/`classes.rs` were (see `identifiers.rs`'s module doc for
//! why a child module sees `mod.rs`'s private fields): `expressions.rs` is
//! already 1.4k lines, and this visit is a self-contained unit with its own
//! rewrite.
//!
//! Ports `SemanticResolver::visit(ESTree::CallExpressionNode *node)`
//! (cpp:1127-1219) and `SemanticResolver::registerLocalEval`
//! (cpp:2849-2857, declared `static` at SemanticResolver.h:501-503).
//!
//! The visit does three unrelated things in sequence, and `visit_node`
//! dispatches `CallExpression` — and ONLY `CallExpression` — here:
//!
//! 1. **Direct `eval()` detection** (cpp:1128-1161). A call whose callee is
//!    the bare identifier `eval` is a *direct* eval. Whether it "looks like"
//!    the real global `eval` is decided by the binding: unbound, or bound to
//!    a global-scope `UndeclaredGlobalProperty`/`GlobalProperty`, means yes.
//!    With `eval` enabled (the `Context` default) a `DirectEval` warning is
//!    emitted for that case and [`register_local_eval`] runs
//!    UNCONDITIONALLY — i.e. also for a *shadowed* `eval`, where `isEval` was
//!    false and no warning was produced. That asymmetry is C++'s, ported as
//!    written; `LexicalScope::localEval`'s own comment (SemContext.h:252-254)
//!    describes the flag as "true if this scope or any descendent scopes have
//!    a local eval call", i.e. a conservative marker, but what consumes it is
//!    outside sema and outside this port. With `eval` disabled the warning
//!    becomes `EvalDisabled` and no scope is marked.
//! 2. **Rewrite #3: `$SHBuiltin.prop(...)` → `SHBuiltin` node**
//!    (cpp:1163-1207). See the section below.
//! 3. **The `super()` check** (cpp:1209-1216): a `super()` call is only
//!    legal when the nearest non-arrow enclosing function is a *derived*
//!    class constructor.
//!
//! ## What is NOT dispatched here
//!
//! `OptionalCallExpression` and `NewExpression` have **no**
//! `SemanticResolver::visit` overload. Verified against the inventory in
//! SemanticResolver.h:200-304: the only call-family overload is `void
//! visit(ESTree::CallExpressionNode *node)` (SemanticResolver.h:269), and
//! `OptionalCallExpressionNode` does not derive from `CallExpressionNode` —
//! ESTree.def:304-319 makes both of them children of the
//! `CallExpressionLike` GROUP (`ESTREE_FIRST(CallExpressionLike, Base)`),
//! i.e. siblings — so that overload is not even viable for it and C++
//! overload resolution picks the catch-all `void visit(ESTree::Node *node) {
//! visitESTreeChildren(*this, node); }` (SemanticResolver.h:191-193). Both
//! kinds therefore belong in `visit_node`'s override-free generic arm;
//! `NewExpression` has been there since S2 T2 and `OptionalCallExpression`
//! joins it here.
//!
//! The visible consequence, and the reason this is worth stating: `eval?.()`
//! and `new eval()` produce **no** direct-eval warning and mark no scope
//! (pinned by `tests/sema_corpus/eval-direct.js`), and `$SHBuiltin.foo?.(1)`
//! is **not** rewritten — so its `$SHBuiltin` identifier survives and becomes
//! an `invalid use of $SHBuiltin` error (pinned by
//! `tests/sema_corpus/error-shbuiltin.js`). Both are C++'s behavior, and
//! `calls-shapes.js` pins the rest of the two kinds' plain resolution.
//!
//! ## Rewrite #3: `$SHBuiltin.prop(...)` (spec §3.4)
//!
//! C++ mutates the callee in place:
//!
//! ```text
//! auto *shBuiltin = new (astContext_) ESTree::SHBuiltinNode();
//! shBuiltin->copyLocationFrom(methodCallee->_object);
//! methodCallee->_object = shBuiltin;
//! ```
//!
//! `methodCallee` IS `node->_callee`, so after that assignment the same
//! `CallExpressionNode` the visit was handed has a `MemberExpression` callee
//! whose `_object` is an `SHBuiltinNode`, and `visitESTreeChildren(*this,
//! node)` at cpp:1218 walks *that*. Structural fields are immutable here, so
//! the port rebuilds instead: a new `MemberExpression` (through the
//! generated builder, so `computed` — a `Cell` decoration — is carried
//! over) and then a new `CallExpression` around it. Everything after the
//! rewrite point — the property-name branches, the `super()` check and the
//! children walk — runs on the REBUILT node, exactly as C++'s run on the
//! mutated one, and the visit returns `Changed`.
//!
//! Per `resolver/mod.rs`'s "decorate before recursing", note that neither
//! rebuilt node carries any decoration this visit writes: `CallExpression`
//! has no `Cell` fields at all and the `MemberExpression`'s only one
//! (`computed`) is copied by `from_node`. The trap that bit rewrite #1 (a
//! decoration written on the discarded node) therefore cannot arise here —
//! but the rebuild order still matters for a different reason, see below.
//!
//! Three details of the C++ that are load-bearing:
//!
//! - **`resolveIdentifier(ident, false)` runs BEFORE the kind test** and is
//!   what makes the rewrite possible: for an unshadowed `$SHBuiltin` it
//!   resolves the identifier to the ambient `UndeclaredGlobalProperty` decl
//!   (libhermes declares `var $SHBuiltin;`, so it is normally already
//!   bound — and if it were not, this call would CREATE it, which is
//!   observable in the dump's decl numbering). The decl it returns is what
//!   the rewrite is gated on: only `UndeclaredGlobalProperty` rewrites, so
//!   `let $SHBuiltin = {}; $SHBuiltin.foo(1)` does not.
//! - **The identifier it resolves is then THROWN AWAY** in the rewriting
//!   case, so `setExpressionDecl`'s entry for it is stranded (in C++ too —
//!   the node is unlinked from the tree while the side table keeps its
//!   entry). Harmless: every consumer, `-dump-sema` included, reaches
//!   expression decls by walking the tree, never by iterating that table.
//! - **The rewrite does NOT suppress the `invalid use of $SHBuiltin` error**
//!   in `visit(IdentifierNode *, Node *)` (cpp:310-314) — it *avoids* it, by
//!   replacing the identifier before the children walk can reach it. In the
//!   shadowed case the identifier survives, the walk reaches it and the
//!   error fires (pinned by `tests/sema_corpus/error-shbuiltin.js`, which
//!   also pins that `resolveIdentifier` being called twice on it — once
//!   here, once from the identifier visit — reports the error exactly ONCE,
//!   because the second call hits the decl cache).
//!
//! The three module-related property names (`moduleFactory`, `export`,
//! `import`, cpp:1183-1204) are the `$SHBuiltin` CommonJS-module protocol
//! and belong to **S4** (the module visits). They are ported as loud
//! phase-tagged panics rather than approximated. One subtlety recorded here
//! for whoever lands S4: the `export` branch (cpp:1192-1201) visits the
//! children FIRST and only then calls `visitModuleExport(node)`, because the
//! exported name must already be resolved — in this port "visit the
//! children" means `visit_children_mut`, which may hand back a *rebuilt*
//! call node, and it is that rebuilt node `visitModuleExport` must be given
//! (and returned as `Changed`). The `moduleFactory` branch (cpp:1183-1191)
//! and the `export` branch both `return` without falling through to
//! cpp:1218, while the `import` branch (cpp:1202-1204) DOES fall through.

use ast::context::GCLock;
use ast::node::{builder, Node, SHBuiltin};
use ast::visitor::TransformResult;
use support::diag::{Subsystem, Warning};

use crate::ids::ScopeId;
use crate::sem_context::{Atom, ConstructorKind, DeclKind, SemContext};

use super::functions::copy_location_from;
use super::SemanticResolver;

/// Mark \p scope and every one of its ancestor scopes as users of local
/// `eval()`. Port of `SemanticResolver::registerLocalEval`
/// (SemanticResolver.cpp:2849-2857).
///
/// C++ declares this `static` (SemanticResolver.h:503) — it touches no
/// resolver state — so it is a free function here rather than a method,
/// taking the `SemContext` the `LexicalScope` records live in. C++'s
/// `LexicalScope *scope` is nullable in principle; `Option<ScopeId>` ports
/// that directly, and the single call site (`curScope_`) is exactly as
/// nullable.
pub(super) fn register_local_eval(
    sem_ctx: &mut SemContext,
    scope: Option<ScopeId>,
) {
    let mut cur_scope = scope;
    while let Some(s) = cur_scope {
        sem_ctx.scope_mut(s).local_eval = true;

        // This can also set a `canRename` flag on the identifier,
        // which we haven't implemented yet.

        cur_scope = sem_ctx.scope(s).parent_scope;
    }
}

/// The `_name` of a `$SHBuiltin.<prop>` member expression's `_property`,
/// when that property is an identifier at all. Port of
/// `llvh::dyn_cast<ESTree::IdentifierNode>(methodCallee->_property)`
/// (cpp:1171-1172, as of upstream `07efab88d`).
///
/// C++ used to `cast` here, so the enclosing `if (auto *propIdent = ...)` was
/// always taken and a non-`Identifier` `_property` was an assertion failure —
/// which `$SHBuiltin.#x()` inside a class declaring `#x` really did trigger
/// (the `_property` is a `PrivateName`; a non-computed member expression's
/// property can be either). Upstream `07efab88d` ("Fix crash on
/// `$SHBuiltin.#privateName()`") turned it into a `dyn_cast` whose result also
/// gates the whole rewrite, so a private property leaves `$SHBuiltin` alone and
/// the identifier is reported as an `invalid use of $SHBuiltin` when it is
/// visited, like any other non-builtin use. This port mirrors that.
fn sh_builtin_property_name(node: &Node) -> Option<Atom> {
    match node {
        Node::Identifier(id) => Some(id.name.get()),
        _ => None,
    }
}

impl SemanticResolver<'_, '_, '_, '_> {
    /// Port of `SemanticResolver::visit(ESTree::CallExpressionNode *node)`
    /// (SemanticResolver.cpp:1127-1219). See the module doc for the three
    /// things it does, for rewrite #3's mechanics and for why
    /// `OptionalCallExpression`/`NewExpression` are not routed here.
    pub(super) fn visit_call_expression<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let call = node
            .as_call_expression()
            .expect("visit_call_expression: not a CallExpression");

        // Check for a direct call to local `eval()`.
        if let Node::Identifier(identifier) = call.callee {
            if identifier.name.get() == self.kw().ident_eval {
                // Check to see whether it looks like attempting to call the
                // actual global eval and generate a warning.
                let is_eval;
                if let Some(binding) =
                    self.binding_table.find(&identifier.name.get())
                {
                    let decl = binding.decl;
                    let global_scope = self.sem_ctx.get_global_scope();
                    is_eval = self.sem_ctx.decl(decl).scope
                        == Some(global_scope)
                        && matches!(
                            self.sem_ctx.decl(decl).kind,
                            DeclKind::UndeclaredGlobalProperty
                                | DeclKind::GlobalProperty
                        );
                } else {
                    is_eval = true;
                }

                // Register the local eval, but only if eval is enabled.
                //
                // C++ reads `astContext_.getEnableEval()`; this port has no
                // `astContext_` field (see `mod.rs`'s module doc) and
                // `GCLock::ctx().enable_eval()` is the same flag.
                if gc.ctx().enable_eval() {
                    if is_eval {
                        self.sm.warning_range(
                            Warning::DirectEval,
                            call.callee.range(),
                            "Direct call to eval(), but lexical scope is not \
                             supported.",
                            Subsystem::Unspecified,
                        );
                    }
                    register_local_eval(self.sem_ctx, self.cur_scope);
                } else if is_eval {
                    self.sm.warning_range(
                        Warning::EvalDisabled,
                        call.callee.range(),
                        "eval() is disabled at runtime",
                        Subsystem::Unspecified,
                    );
                }
            }
        }

        // Check for $SHBuiltin, and transform the node if necessary to
        // SHBuiltinNode. This allows typechecker/IRGen to simply match on
        // SHBuiltinNode.
        //
        // `rewritten` stands for C++'s in-place `methodCallee->_object =
        // shBuiltin` (which also mutates `node`, since `methodCallee` IS
        // `node->_callee`): `None` means the tree was left alone. See the
        // module doc's "Rewrite #3".
        let mut rewritten: Option<&'gc Node<'gc>> = None;
        if let Node::MemberExpression(method_callee) = call.callee {
            // Note that the property of a non-computed member expression can
            // also be a PrivateNameNode, not just an identifier. A private
            // property is never a builtin access, so in that case $SHBuiltin
            // is left alone and is reported as an invalid use when the
            // identifier itself is visited.
            let prop_ident = sh_builtin_property_name(method_callee.property);
            if let Node::Identifier(ident) = method_callee.object {
                if ident.name.get() == self.kw().ident_sh_builtin
                    && !method_callee.computed.get()
                    && prop_ident.is_some()
                {
                    // `resolveIdentifier` never returns null (cpp:1981-2045
                    // ends in an unconditional `return decl;` after creating
                    // an ambient global), which is why this port's
                    // `resolve_identifier` returns a plain `DeclId` and C++'s
                    // `decl &&` has no counterpart below.
                    let obj = method_callee.object;
                    let decl = self.resolve_identifier(gc, obj, false);
                    if self.sem_ctx.decl(decl).kind
                        == DeclKind::UndeclaredGlobalProperty
                    {
                        let sh_builtin =
                            gc.alloc(Node::SHBuiltin(SHBuiltin::new(
                                copy_location_from(method_callee.object),
                            )));
                        // methodCallee->_object = shBuiltin;
                        let mut mb =
                            builder::MemberExpression::from_node(method_callee);
                        mb.object(sh_builtin);
                        // ... and, because `methodCallee` is `node->_callee`,
                        // the same assignment updates `node` too.
                        let mut cb = builder::CallExpression::from_node(call);
                        cb.callee(mb.build_forced(gc));
                        rewritten = Some(cb.build_forced(gc));
                    }
                    // C++ writes the `dyn_cast` result into the SAME
                    // condition (`&& propIdent`) and then dereferences it
                    // here; edition 2021 has no let-chains, so the test above
                    // is `is_some()` and the value is taken back out here.
                    let Some(prop_name) = prop_ident else {
                        unreachable!("gated on `prop_ident.is_some()` above")
                    };
                    if prop_name == self.kw().ident_module_factory {
                        // This visits its children explicitly (with a module
                        // context set), so we return after it.
                        // The require optimization should only be validated
                        // when we are actually attempting to parse this code
                        // for compilation. Before that point, the SH builtin
                        // call may still be incomplete.
                        //   if (compile_)
                        //     visitModuleFactory(node);
                        //   return;
                        //
                        // The panic is unconditional even though the C++ call
                        // is guarded by `compile_`. That is a deliberate,
                        // spec-sanctioned deviation (parent spec §1: "the
                        // `$SHBuiltin` branches ... keep their loud
                        // phase-tagged panics" through S4a), NOT an argument
                        // that the guard is dead — `compile` is `false` on
                        // one of this port's two entries,
                        // `resolve::resolve_ast_for_parser` (resolve.rs:97),
                        // and the parser oracle pair reaches this branch. An
                        // earlier version of this comment claimed `compile`
                        // was `true` on every entry; S4a T2 made that false.
                        //
                        // **For S4b**: the capstone review probed hermesc's
                        // `compile = false` behavior for all three branches,
                        // because it is not obvious from the code and is
                        // observable through the parser pair
                        // (`sema-parser-dump` vs `sema-dump --parser-entry`):
                        //
                        // - `moduleFactory`: exit 0 with a full dump, and the
                        //   children are NOT walked — the `return` at
                        //   cpp:1191 is OUTSIDE the `if (compile_)`, so with
                        //   `compile_ == false` the call is skipped but the
                        //   `return` still fires. Dropping either the
                        //   `if (compile_)` gate or the unconditional
                        //   children-skipping `return` is a bug the parser
                        //   pair will catch.
                        // - `export`: exit 0 with a dump; `visitESTreeChildren`
                        //   and `visitModuleExport` both run UNGATED by
                        //   `compile_` (cpp:1197-1202).
                        // - `import`: exit 2 with a dump; `visitModuleImport`
                        //   runs UNGATED (cpp:1203-1204) and there is no
                        //   `return`, so the branch falls through to the
                        //   children walk.
                        panic!(
                            "sema: $SHBuiltin.moduleFactory needs \
                             visitModuleFactory (cpp:1334-1380) — S4 modules"
                        );
                    } else if prop_name == self.kw().ident_export {
                        // In this case, we must visit the children first, to
                        // ensure that the exported name is resolved before we
                        // call visitModuleExport. Therefore, we return
                        // explicitly after, so we don't visit the children
                        // again below.
                        //   visitESTreeChildren(*this, node);
                        //   if (LLVM_UNLIKELY(recursionDepth_ == 0))
                        //     return;
                        //   visitModuleExport(node);
                        //   return;
                        //
                        // See the module doc: the children-first order means
                        // S4 must hand `visitModuleExport` the node
                        // `visit_children_mut` returned, not this one.
                        panic!(
                            "sema: $SHBuiltin.export needs visitModuleExport \
                             (cpp:1382-1427) — S4 modules"
                        );
                    } else if prop_name == self.kw().ident_import {
                        //   visitModuleImport(node);
                        // (no `return` — this branch falls through to the
                        // children walk below)
                        panic!(
                            "sema: $SHBuiltin.import needs visitModuleImport \
                             (cpp:1429-1467) — S4 modules"
                        );
                    }
                }
            }
        }

        // Everything from here on is C++ operating on the (possibly mutated)
        // `node`; in this port that is the rebuilt one when rewrite #3 fired.
        let node = rewritten.unwrap_or(node);
        let callee = node
            .as_call_expression()
            .expect("the CallExpression builder builds a CallExpression")
            .callee;

        if matches!(callee, Node::Super(_)) {
            let nearest = self
                .sem_ctx
                .nearest_non_arrow(self.function_context().sem_info);
            if self.sem_ctx.function(nearest).constructor_kind
                != ConstructorKind::Derived
            {
                self.sm.error_range(
                    node.range(),
                    "super() call only allowed in derived class constructor",
                );
            }
        }

        // visitESTreeChildren(*this, node)
        let result = node.visit_children_mut(gc, self);
        match result {
            // A rewrite must be reported even when the children walk of the
            // rebuilt node changed nothing.
            TransformResult::Unchanged if rewritten.is_some() => {
                TransformResult::Changed(node)
            }
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keywords::Keywords;
    use crate::sem_context::{CustomDirectives, FuncIsArrow};
    use ast::context::Context;

    /// `registerLocalEval` marks the given scope AND every ancestor, and
    /// leaves unrelated scopes alone. `local_eval` never reaches
    /// `-dump-sema`, so the differential is blind to it — this is the only
    /// test that can catch a regression.
    #[test]
    fn register_local_eval_marks_the_whole_ancestor_chain() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let func = sem_ctx.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            /* strict */ false,
            CustomDirectives::default(),
        );
        // outer -> middle -> inner, plus a sibling of `middle`.
        let outer = sem_ctx.new_scope(func, None);
        let middle = sem_ctx.new_scope(func, Some(outer));
        let inner = sem_ctx.new_scope(func, Some(middle));
        let sibling = sem_ctx.new_scope(func, Some(outer));

        register_local_eval(&mut sem_ctx, Some(inner));

        assert!(sem_ctx.scope(inner).local_eval);
        assert!(sem_ctx.scope(middle).local_eval);
        assert!(sem_ctx.scope(outer).local_eval);
        assert!(
            !sem_ctx.scope(sibling).local_eval,
            "only the ancestor chain is marked"
        );

        // A `None` scope (C++'s null `curScope_`) is a no-op, not a panic.
        register_local_eval(&mut sem_ctx, None);
        assert!(!sem_ctx.scope(sibling).local_eval);
    }
}
