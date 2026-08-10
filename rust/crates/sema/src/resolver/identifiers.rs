/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! S1 T4: the identifier-resolution core, split out of `resolver/mod.rs`
//! once that module passed ~1.5k lines (per the task brief's suggestion).
//! Everything here is an `impl<'bt, 'sc, 'sm, 'ad> SemanticResolver<'bt,
//! 'sc, 'sm, 'ad>` method — a second inherent-impl block for the type
//! defined in `mod.rs`, which Rust allows anywhere in the same crate.
//! Being a child module of `resolver`, this file sees `resolver`'s private
//! fields and helper methods (`kw()`, `cur_function_info()`,
//! `function_context()`, `binding_table`, `sem_ctx`, `sm`, `global_scope`,
//! the five forbid/permission flags, ...) the same way any other method in
//! `mod.rs` would.
//!
//! Ports `SemanticResolver::visit(IdentifierNode *, Node *)`
//! (SemanticResolver.cpp:277-323), `resolveIdentifier` (cpp:1981-2045),
//! `checkIdentifierResolved` (cpp:2082-2100), `declareArguments`
//! (SemanticResolver.h:349-355), and the two helpers
//! `resolveIdentifier`'s strict-mode warning needs:
//! `FunctionContext::getFunctionName` (cpp:3086-3093) and the free function
//! `ESTree::getIdentifier(FunctionLikeNode *)` (`lib/AST/ESTree.cpp:83-92`).
//! See the task report for the two deliberate scope cuts (the C++
//! `MemberExpression`/`OptionalMemberExpression` overrides' private-name
//! handling, and the `typeof missing;` corpus file) and why each is safe to
//! defer. The first of the two landed in S2 T5 — see
//! `expressions::visit_member_like_expression`.
//!
//! S2 T5 also adds this file's private-name pair, `declarePrivateName`
//! (cpp:2047-2064) and `resolvePrivateName` (cpp:2066-2080): they live here
//! because they are `checkIdentifierResolved`'s immediate C++ neighbours and
//! share its decl-state-machine plumbing, while everything that *calls* them
//! (the class-body early errors, `visit(PrivateNameNode *)`, the member
//! restriction checks) lives in `classes.rs` / `expressions.rs`. The `#`
//! mangling both of them start from is
//! [`crate::sem_context::private_name_identifier`].

use ast::context::{GCLock, NodeRc};
use ast::node::{Node, NodeField};
use ast::visitor::{Path, TransformResult};
use support::diag::{Subsystem, Warning};

use crate::ids::DeclId;
use crate::sem_context::{
    private_name_identifier, Atom, Binding, DeclKind, DeclSpecial,
};

use super::SemanticResolver;

impl<'bt, 'sc, 'sm, 'ad> SemanticResolver<'bt, 'sc, 'sm, 'ad> {
    /// Declare 'arguments' for use in either the function or the
    /// parameters. Port of `SemanticResolver::declareArguments`
    /// (SemanticResolver.h:349-355).
    ///
    /// Called from the function visits (S1 T7, `functions.rs`) at the two
    /// C++ call sites those cover — the temporary-arguments scope
    /// (cpp:1874) and the function-body declaration (cpp:1937) — and, since
    /// S2 T4/T5, from the two class field-initializer visits (cpp:1049 for
    /// `ClassProperty`, cpp:999 for `ClassPrivateProperty`). Those are all
    /// four C++ call sites.
    pub(super) fn declare_arguments(&mut self) {
        let func = self.cur_function_info();
        let arguments_name = self.kw().ident_arguments;
        let args_decl = self.sem_ctx.func_arguments_decl(func, arguments_name);
        self.binding_table
            .try_emplace(arguments_name, Binding::new(args_decl, None));
    }

    // ---- Identifier resolution (S1 T4) ------------------------------------

    /// Port of `SemanticResolver::checkIdentifierResolved`
    /// (SemanticResolver.cpp:2082-2100).
    ///
    /// Deviation: takes the `Identifier` node itself rather than the
    /// brief's suggested `(NodeId, &Identifier)` pair — both are derivable
    /// from `node` (`node.node_id()`, `node.as_identifier()`), and folding
    /// them into one parameter removes the possibility of a caller passing
    /// a mismatched pair (`SemContext::set_expression_decl` already
    /// `debug_assert!`s the two agree, so nothing is lost).
    ///
    /// \param node an `Identifier` node.
    /// \return the declaration the identifier resolves to, or `None` if it
    ///   could not be resolved.
    fn check_identifier_resolved(&mut self, node: &Node) -> Option<DeclId> {
        let identifier = node
            .as_identifier()
            .expect("check_identifier_resolved: not an Identifier node");

        // If identifier already resolved or unresolvable, pick the resolved
        // declaration.
        if identifier.unresolvable.get() {
            return None;
        }

        if let Some(decl) = self.sem_ctx.get_expression_decl(identifier) {
            return Some(decl);
        }

        // If we find the binding, assign the associated declaration and
        // return it.
        if let Some(binding) = self.binding_table.find(&identifier.name.get()) {
            self.sem_ctx.set_expression_decl(
                node.node_id(),
                identifier,
                Some(binding.decl),
            );
            return Some(binding.decl);
        }

        // Failed to resolve.
        None
    }

    /// \return the name of the function whose `FunctionContext` is
    /// `node`'s (Flow's `ComponentDeclaration`/`HookDeclaration` are not
    /// reachable through this port's parser yet, so unlike the C++ they
    /// cannot trip the `debug_assert!` below in practice). Port of the free
    /// function `ESTree::getIdentifier(FunctionLikeNode *)`
    /// (`lib/AST/ESTree.cpp:83-92`), restricted to the three cases
    /// `FunctionContext::getFunctionName` (below) can ever ask about.
    fn function_like_identifier<'gc>(
        node: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        match node {
            Node::FunctionExpression(n) => n.id,
            Node::FunctionDeclaration(n) => n.id,
            Node::ArrowFunctionExpression(_) => None,
            _ => {
                debug_assert!(
                    matches!(node, Node::Program(_)),
                    "invalid FunctionLikeNode"
                );
                None
            }
        }
    }

    /// \return the name of the current function, if it has an explicit
    /// name (a named `FunctionExpression`/`FunctionDeclaration`). Port of
    /// `FunctionContext::getFunctionName` (SemanticResolver.cpp:3086-3093).
    fn function_name(&self, gc: &GCLock) -> Option<Atom> {
        let node_rc = self.function_context().node.as_ref()?;
        let id_node = Self::function_like_identifier(node_rc.node(gc))?;
        match id_node {
            Node::Identifier(id) => Some(id.name.get()),
            _ => None,
        }
    }

    // ---- Private names (S2 T5) --------------------------------------------

    /// Create a new declaration of a private name in the current scope. Port
    /// of `SemanticResolver::declarePrivateName` (cpp:2047-2064).
    ///
    /// \pre An `Identifier` node which has the same `_name` field has not
    ///   been declared in this same `LexicalScope`.
    /// \param ident_node the AST node containing the name.
    /// \param kind the kind of decl to be created.
    /// \param is_static is only valid for methods / accessors. C++ defaults
    ///   it to `false` (SemanticResolver.h:412); the field/`PrivateField`
    ///   call site (cpp:2196) is the one that relies on the default, and
    ///   passes `false` explicitly here.
    /// \return the newly created decl.
    pub(super) fn declare_private_name<'gc>(
        &mut self,
        gc: &'gc GCLock,
        ident_node: &'gc Node<'gc>,
        kind: DeclKind,
        is_static: bool,
    ) -> DeclId {
        let identifier = ident_node
            .as_identifier()
            .expect("declare_private_name: not an Identifier node");
        let private_name_str =
            private_name_identifier(gc, identifier.name.get());
        let cur_scope = self
            .cur_scope
            .expect("a private name is always declared in the class scope");
        let decl = self.sem_ctx.new_decl_in_scope(
            private_name_str,
            kind,
            cur_scope,
            if is_static {
                DeclSpecial::PrivateStatic
            } else {
                DeclSpecial::NotSpecial
            },
        );
        let res = self.binding_table.try_emplace(
            private_name_str,
            Binding::new(decl, Some(NodeRc::from_node(gc, ident_node))),
        );
        debug_assert!(
            res,
            "cannot re-declare a private name in the same scope."
        );
        self.sem_ctx
            .set_both_decl(ident_node.node_id(), identifier, Some(decl));
        decl
    }

    /// Resolve an identifier for a private name to a declaration and record
    /// the resolution. Port of `SemanticResolver::resolvePrivateName`
    /// (cpp:2066-2080).
    ///
    /// Unlike its sibling `check_identifier_resolved`, this one does NOT test
    /// `isUnresolvable()`: the C++ doesn't either (a private name is never
    /// run through the `Unresolver`, which only walks the *variable*
    /// references inside a `with` body).
    ///
    /// \param ident_node the `Identifier` inside a `PrivateName`, or a
    ///   `ClassPrivateProperty`'s `_key`.
    /// \return the declaration the private name resolves to, or `None` when
    ///   no enclosing class declared it. NOTE (unlike the header's wording
    ///   at SemanticResolver.h:414-417): this function raises no error
    ///   itself; each caller decides — `visit(PrivateNameNode *)` reports
    ///   "was not declared in any enclosing class", while the
    ///   `MemberExpression` branches (cpp:1238-1239) silently skip their
    ///   extra validation.
    pub(super) fn resolve_private_name(
        &mut self,
        gc: &GCLock,
        ident_node: &Node,
    ) -> Option<DeclId> {
        let identifier = ident_node
            .as_identifier()
            .expect("resolve_private_name: not an Identifier node");
        if let Some(decl) = self.sem_ctx.get_expression_decl(identifier) {
            return Some(decl);
        }

        // If we find the binding, assign the associated declaration and
        // return it.
        let private_name_str =
            private_name_identifier(gc, identifier.name.get());
        if let Some(binding) = self.binding_table.find(&private_name_str) {
            self.sem_ctx.set_expression_decl(
                ident_node.node_id(),
                identifier,
                Some(binding.decl),
            );
            return Some(binding.decl);
        }

        None
    }

    /// Port of `SemanticResolver::resolveIdentifier`
    /// (SemanticResolver.cpp:1981-2045). See the module doc's "identifier
    /// resolution" notes and the task brief for the exact ordering this
    /// preserves (Arguments-special check, then the two forbid-flag
    /// checks, THEN the early `decl` return, then the strict-mode warning /
    /// ambient-global creation — every check below runs in that order
    /// regardless of whether `decl` already resolved, exactly like the C++).
    ///
    /// `pub(super)` for the second C++ call site, which is not in this file:
    /// `visit(CallExpressionNode *)`'s `$SHBuiltin` rewrite (cpp:1177, see
    /// `calls::visit_call_expression`).
    ///
    /// \param node an `Identifier` node.
    /// \param in_typeof whether `node` is the direct operand of `typeof`.
    /// \return the resolved (or newly created ambient-global) declaration.
    pub(super) fn resolve_identifier<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
        in_typeof: bool,
    ) -> DeclId {
        let identifier = node
            .as_identifier()
            .expect("resolve_identifier: not an Identifier node");
        let decl = self.check_identifier_resolved(node);

        // Is this the "arguments" object?
        if let Some(d) = decl {
            if self.sem_ctx.decl(d).special == DeclSpecial::Arguments {
                if self.forbid_special_arguments_reference {
                    self.sm.error_range(
                        identifier.metadata.range.get(),
                        "invalid use of 'arguments'",
                    );
                }
                let f = self.cur_function_info();
                self.sem_ctx.function_mut(f).uses_arguments = true;
            }
        }

        if identifier.name.get() == self.kw().ident_await
            && self.forbid_await_as_identifier
        {
            self.sm.error_range(
                identifier.metadata.range.get(),
                "await is not a valid identifier name in an async function",
            );
        }

        if identifier.name.get() == self.kw().ident_arguments
            && self.forbid_arguments_as_identifier
        {
            self.sm.error_range(
                identifier.metadata.range.get(),
                "invalid use of 'arguments' as an identifier",
            );
        }

        // Resolved the identifier to a declaration, done.
        if let Some(d) = decl {
            return d;
        }

        // Undeclared variables outside `typeof` cause runtime errors in
        // strict mode. $SHBuiltin is special-cased: it is always resolved
        // as an undeclared global property without a warning, since it is a
        // compiler intrinsic.
        if !in_typeof
            && self.sem_ctx.function(self.cur_function_info()).strict
            && identifier.name.get() != self.kw().ident_sh_builtin
        {
            let func_name_atom = self.function_name(gc);
            let mut func_name = match func_name_atom {
                Some(a) => String::from_utf8_lossy(gc.bytes(a)).into_owned(),
                None => String::new(),
            };
            if self.in_global_scope_context() && func_name.is_empty() {
                func_name = "global".to_string();
            }
            let func_type =
                if self.sem_ctx.function(self.cur_function_info()).arrow {
                    "arrow function"
                } else {
                    "function"
                };
            let disp_name = if !func_name.is_empty() {
                format!("{func_type} \"{func_name}\"")
            } else {
                format!("anonymous {func_type}")
            };
            let ident_name =
                String::from_utf8_lossy(gc.bytes(identifier.name.get()))
                    .into_owned();
            self.sm.warning_range(
                Warning::UndefinedVariable,
                identifier.metadata.range.get(),
                format!(
                    "the variable \"{ident_name}\" was not declared in \
                     {disp_name}"
                ),
                Subsystem::Unspecified,
            );
        }

        // Declare an ambient global property.
        let name = identifier.name.get();
        let new_decl = self
            .sem_ctx
            .new_global(name, DeclKind::UndeclaredGlobalProperty);
        self.sem_ctx.set_expression_decl(
            node.node_id(),
            identifier,
            Some(new_decl),
        );
        self.binding_table.try_emplace_into_scope(
            &self.global_scope,
            name,
            Binding::new(new_decl, Some(NodeRc::from_node(gc, node))),
        );
        new_decl
    }

    /// Port of `SemanticResolver::visit(ESTree::IdentifierNode *identifier,
    /// ESTree::Node *parent)` (SemanticResolver.cpp:277-323).
    ///
    /// Every parent-kind skip below is ported even though the S1 corpus
    /// cannot reach all of them (private-name/meta-property/break/continue/
    /// labeled parents need class/label/`new.target` support that lands in
    /// later stages) — see the task brief: these are parent-kind tests, not
    /// visits, so they cost nothing to port now and there is no reason for
    /// this visit to diverge from the C++ order.
    pub(super) fn visit_identifier<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
        path: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let identifier = node
            .as_identifier()
            .expect("visit_identifier: not an Identifier node");

        if let Some(p) = path {
            // { identifier: ... }
            if let Node::Property(prop) = p.parent {
                if !prop.computed.get() && p.field == NodeField::key {
                    return TransformResult::Unchanged;
                }
            }

            // expr.identifier
            // expr?.identifier
            let member_like_skip = match p.parent {
                Node::MemberExpression(m) => {
                    !m.computed.get() && p.field == NodeField::property
                }
                Node::OptionalMemberExpression(m) => {
                    !m.computed.get() && p.field == NodeField::property
                }
                _ => false,
            };
            if member_like_skip {
                return TransformResult::Unchanged;
            }

            // Identifiers that aren't variables.
            if matches!(
                p.parent,
                Node::MetaProperty(_)
                    | Node::BreakStatement(_)
                    | Node::ContinueStatement(_)
                    | Node::LabeledStatement(_)
            ) {
                return TransformResult::Unchanged;
            }

            // typeof
            //
            // NOTE (faithful port, not a bug): C++ has no early return here
            // (cpp:304-308) — after this `resolveIdentifier(identifier,
            // true)` call, control falls through the `$SHBuiltin`/
            // `PrivateNameNode` checks below and reaches the UNCONDITIONAL
            // `resolveIdentifier(identifier, false)` at the end of the
            // function. That second call is not a second resolution: the
            // first call already cached a decl on `identifier` (either by
            // finding a binding or by creating the ambient global — see
            // `resolve_identifier`), so `check_identifier_resolved` inside
            // the second call hits that cache and returns immediately,
            // before the second call's `in_typeof: false` could ever matter
            // (in particular, no second, non-typeof warning is possible).
            // This is why `typeof missing;` in strict mode creates the
            // ambient global for `missing` with no warning — see the
            // "typeof" test in this file's `tests` module, which pins the
            // mechanism directly, and `tests/sema_corpus/typeof-strict.js`,
            // which pins the same behavior end-to-end now that S1 T6's
            // `UnaryExpression` visit routes here.
            if let Node::UnaryExpression(unary) = p.parent {
                if unary.operator.get() == self.kw().ident_typeof {
                    self.resolve_identifier(gc, node, true);
                }
            }
        }

        // $SHBuiltin should have been replaced with SHBuiltinNode as part of
        // a member call expression earlier. Any use that gets here is
        // invalid.
        //
        // "earlier" is `visit(CallExpressionNode *)`'s rewrite #3
        // (cpp:1163-1182, `calls::visit_call_expression`, S2 T6), which runs
        // BEFORE the call's children are walked and so removes the
        // identifier before this visit could ever see it. Every shape it
        // declines to rewrite lands here instead — see
        // `tests/sema_corpus/error-shbuiltin.js`.
        if identifier.name.get() == self.kw().ident_sh_builtin {
            self.sm.error_range(
                identifier.metadata.range.get(),
                "invalid use of $SHBuiltin",
            );
        }

        // Identifiers belonging to a PrivateNameNode are validated in the
        // PrivateNameNode visitor.
        if let Some(p) = path {
            if matches!(p.parent, Node::PrivateName(_)) {
                return TransformResult::Unchanged;
            }
        }

        self.resolve_identifier(gc, node, false);
        TransformResult::Unchanged
    }
}

#[cfg(test)]
mod tests {
    use ast::context::Context;
    use ast::node::Identifier;
    use ast::node_child::NodeMetadata;
    use support::location::{SMLoc, SMRange};
    use support::manager::SourceErrorManager;
    use support::persistent_scoped_map::Scope;

    use super::*;
    use crate::keywords::Keywords;
    use crate::resolver::FunctionContext;
    use crate::sem_context::{ConstructorKind, CustomDirectives, SemContext};

    /// Allocate an `Identifier` node named `name` at `loc`.
    fn alloc_identifier<'gc>(
        gc: &'gc ast::context::GCLock,
        name: &str,
        loc: SMLoc,
    ) -> &'gc Node<'gc> {
        let atom = gc.atom_bytes(name);
        gc.alloc(Node::Identifier(Identifier::new(
            NodeMetadata::new(SMRange {
                start: loc,
                end: loc,
            }),
            atom,
            None,
            false,
        )))
    }

    /// Expression-decl caching: the SECOND resolution of the same
    /// `Identifier` node must hit the node's own `Cell` (via
    /// `SemContext::get_expression_decl`), not repeat the binding-table
    /// lookup. Proven here by dropping the binding-table scope (so a fresh
    /// `find` would return `None`) between the two calls: if the second
    /// call still returns the original decl, it can only have come from the
    /// cached `Cell`.
    #[test]
    fn expression_decl_is_cached_on_the_node_not_relooked_up() {
        let mut ctx = Context::new();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("ident.js", b"x");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let gc = ctx.lock();
        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let func = sem_ctx.new_function(
            crate::sem_context::FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            false,
            CustomDirectives::default(),
        );
        let scope = sem_ctx.new_scope(func, None);
        let name = gc.atom_bytes("x");
        let decl = sem_ctx.new_decl_in_scope_default(
            name,
            crate::sem_context::DeclKind::Let,
            scope,
        );
        let node = alloc_identifier(&gc, "x", loc);

        let binding_table = sem_ctx.binding_table_rc();
        let d1 = {
            let bscope = Scope::new(&binding_table);
            binding_table.try_emplace(name, Binding::new(decl, None));
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            let d = resolver.resolve_identifier(&gc, node, false);
            drop(bscope); // pops the scope: `find(name)` now returns `None`.
            d
        };
        assert_eq!(d1, decl);

        // No binding-table scope is open at all now, so a fresh
        // `check_identifier_resolved` lookup (`find`) would panic/return
        // `None` — the fact that this resolves at all proves the Cell hit.
        let d2 = {
            let mut resolver2 = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            resolver2.resolve_identifier(&gc, node, false)
            // `resolver2` (and its `&mut sm`/`&mut sem_ctx` borrows) drops
            // here — required before the counts below can borrow `sm`
            // again: `SemanticResolver`'s `Drop` impl keeps those borrows
            // alive for its whole scope, not just its last use.
        };
        assert_eq!(d2, decl, "second resolution must hit the cached decl");
        assert_eq!(sm.error_count(), 0);
        assert_eq!(sm.warning_count(), 0);
    }

    /// `forbidSpecialArgumentsReference_`: referencing the special
    /// `arguments` decl while forbidden reports "invalid use of
    /// 'arguments'" — and, regardless of the flag,
    /// `curFunctionInfo()->usesArguments` is set (cpp:1994-1998).
    #[test]
    fn forbid_special_arguments_reference_reports_invalid_use() {
        let mut ctx = Context::new();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("ident.js", b"x");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let gc = ctx.lock();
        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let func = sem_ctx.new_function(
            crate::sem_context::FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            false,
            CustomDirectives::default(),
        );
        let scope = sem_ctx.new_scope(func, None);
        let name = gc.atom_bytes("arguments");
        let decl = sem_ctx.new_decl_in_scope(
            name,
            crate::sem_context::DeclKind::Var,
            scope,
            DeclSpecial::Arguments,
        );
        let node = alloc_identifier(&gc, "arguments", loc);

        let binding_table = sem_ctx.binding_table_rc();
        let _bscope = Scope::new(&binding_table);
        binding_table.try_emplace(name, Binding::new(decl, None));

        let d = {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            // `resolve_identifier` reads `curFunctionInfo()` unconditionally
            // once `decl.special == Arguments`, so a function context must
            // be active — pushed directly (same-module test), matching
            // `enter_function`'s effect without its `DeclCollector`
            // overhead.
            resolver.function_stack.push(FunctionContext {
                sem_info: func,
                node: None,
                label_map: Default::default(),
                current_loop: None,
                current_loop_or_switch: None,
                is_formal_params: false,
                decls: None,
                promoted_func_decls: Default::default(),
                binding_table_scope_depth: 0,
            });
            resolver.forbid_special_arguments_reference = true;
            resolver.resolve_identifier(&gc, node, false)
            // `resolver` drops here, releasing its `&mut sm`/`&mut sem_ctx`
            // borrows before the asserts below need them.
        };
        assert_eq!(d, decl);
        assert_eq!(sm.error_count(), 1);
        assert_eq!(sm.warning_count(), 0);
        assert!(sem_ctx.function(func).uses_arguments);
    }

    /// `forbidAwaitAsIdentifier_`: using `await` as an identifier while
    /// forbidden reports the async-arrow-parameter-list error, regardless
    /// of whether the identifier itself resolves.
    #[test]
    fn forbid_await_as_identifier_reports_error() {
        let mut ctx = Context::new();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("ident.js", b"x");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let gc = ctx.lock();
        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let func = sem_ctx.new_function(
            crate::sem_context::FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            false,
            CustomDirectives::default(),
        );
        let scope = sem_ctx.new_scope(func, None);
        let name = sem_ctx.kw.ident_await;
        let decl = sem_ctx.new_decl_in_scope_default(
            name,
            crate::sem_context::DeclKind::Let,
            scope,
        );
        let node = alloc_identifier(&gc, "await", loc);

        let binding_table = sem_ctx.binding_table_rc();
        let _bscope = Scope::new(&binding_table);
        binding_table.try_emplace(name, Binding::new(decl, None));

        let d = {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            resolver.forbid_await_as_identifier = true;
            resolver.resolve_identifier(&gc, node, false)
        };
        assert_eq!(d, decl, "the forbid check must not prevent resolution");
        assert_eq!(sm.error_count(), 1);
    }

    /// `forbidArgumentsAsIdentifier_`: using `arguments` as a (non-special)
    /// identifier while forbidden reports "invalid use of 'arguments' as an
    /// identifier" — a different message from the special-reference error
    /// above, and triggered purely by the name, independent of `decl`.
    #[test]
    fn forbid_arguments_as_identifier_reports_error() {
        let mut ctx = Context::new();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("ident.js", b"x");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let gc = ctx.lock();
        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let func = sem_ctx.new_function(
            crate::sem_context::FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            false,
            CustomDirectives::default(),
        );
        let scope = sem_ctx.new_scope(func, None);
        let name = sem_ctx.kw.ident_arguments;
        // NotSpecial on purpose: this isolates the identifier-name forbid
        // check from the special-Arguments-decl check above.
        let decl = sem_ctx.new_decl_in_scope_default(
            name,
            crate::sem_context::DeclKind::Let,
            scope,
        );
        let node = alloc_identifier(&gc, "arguments", loc);

        let binding_table = sem_ctx.binding_table_rc();
        let _bscope = Scope::new(&binding_table);
        binding_table.try_emplace(name, Binding::new(decl, None));

        let d = {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            resolver.forbid_arguments_as_identifier = true;
            resolver.resolve_identifier(&gc, node, false)
        };
        assert_eq!(d, decl);
        assert_eq!(sm.error_count(), 1);
    }

    /// `typeof missing` in strict mode must NOT warn (the `inTypeof` guard
    /// on the strict-mode `UndefinedVariable` warning) but MUST still
    /// create the ambient global (`resolveIdentifier`'s ambient-global
    /// creation is unconditional — only the warning is gated). See the
    /// module doc's "typeof" note on `visit_identifier` for why this is
    /// tested by calling `resolve_identifier(..., in_typeof: true)`
    /// directly rather than through a corpus file.
    #[test]
    fn resolve_identifier_typeof_creates_ambient_global_without_warning() {
        let mut ctx = Context::new();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("ident.js", b"x");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let gc = ctx.lock();
        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let binding_table = sem_ctx.binding_table_rc();
        let node = alloc_identifier(&gc, "missing", loc);
        // `enter_function` finishes by calling `set_node_sem_info(node,
        // ...)`, which panics unless `node` is one of the six function-like
        // kinds — so, unlike the identifier under test, the placeholder
        // handed to `enter_function` must be a (trivial, empty) `Program`.
        let program_placeholder =
            gc.alloc(Node::Program(ast::node::Program::new(
                NodeMetadata::new(SMRange {
                    start: loc,
                    end: loc,
                }),
                ast::node_child::NodeList::from_iter(&gc, []),
            )));

        let (d1, d2) = {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            // Mirror `visit_program`'s prologue (cpp:203-227) minus
            // directive scanning: a strict, global function context with
            // one scope designated as the global scope, so the
            // ambient-global path (which needs `self.global_scope` set)
            // works exactly as it does through the real entry point.
            let func_state = resolver.enter_function(
                &gc,
                program_placeholder,
                None,
                /* strict */ true,
                ConstructorKind::None,
                CustomDirectives::default(),
                /* install_as_global_context */ true,
            );
            let scope_state = resolver.enter_scope(None, true);
            resolver.global_scope = resolver.cur_binding_scope().ptr();
            resolver
                .sem_ctx
                .set_binding_table_global_scope(resolver.global_scope.clone());

            // `typeof missing`: no warning, but the ambient global is
            // created and cached on `node`. (Asserting `sm.warning_count()`
            // here would conflict with `resolver`'s live `&mut sm` borrow —
            // see the note on `d2` above — so all counts are checked after
            // this block instead.)
            let d1 = resolver.resolve_identifier(&gc, node, true);

            // A plain (non-typeof) re-resolution of the SAME node must hit
            // the cache and return the identical decl — not re-run the
            // strict-mode warning path (which would otherwise now find
            // `decl` unset and warn).
            let d2 = resolver.resolve_identifier(&gc, node, false);

            resolver.exit_scope(scope_state);
            resolver.exit_function(func_state);
            (d1, d2)
        };
        assert_eq!(d1, d2);
        assert_eq!(sm.warning_count(), 0);
        assert_eq!(sm.error_count(), 0);
        assert_eq!(
            sem_ctx.decl(d1).kind,
            crate::sem_context::DeclKind::UndeclaredGlobalProperty
        );
    }
}
