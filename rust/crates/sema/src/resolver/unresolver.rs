/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! S2 T3: port of `hermes::sema::Unresolver` — declared at
//! SemanticResolver.h:679-711, defined at SemanticResolver.cpp:3200-3224.
//!
//! "Visitor pass for marking variables as Unresolvable based on local
//! `eval()` or `with`" (SemanticResolver.h:679-680).
//!
//! ## Why it exists
//!
//! A `with` block (and, once it is supported, a non-strict direct `eval`)
//! can introduce bindings that are only known at run time, so any identifier
//! *inside* it that the resolver already resolved to a declaration in an
//! enclosing scope may in fact be shadowed dynamically. Such identifiers
//! must not keep a static resolution: this pass walks the affected subtree,
//! clears each such identifier's "expression decl" and sets its
//! `unresolvable` flag, which is what makes later stages emit a dynamic
//! (by-name) lookup instead of a variable access.
//!
//! ## The two call sites
//!
//! - `visit(WithStatementNode *)` (cpp:763-768) — ported in
//!   `statements.rs`, and the only one reachable today. It passes
//!   `curScope_->depth + 1` and the `with`'s BODY as the root, so
//!   declarations made *inside* the `with` (depth >= that) keep their
//!   resolution while everything from the enclosing scope outwards loses it.
//! - `visitFunctionBodyAfterParamsVisited` (cpp:1945-1951) — the local-`eval`
//!   case, which C++ itself disables: the condition is literally `if (false
//!   && lexScope->localEval && !curFunctionInfo()->strict)` behind a `TODO:
//!   enable this when non-strict direct eval is supported`. `functions.rs`
//!   carries that dead branch's TODO at the matching site; nothing calls this
//!   pass from there in either tree.
//!
//! ## Dump visibility
//!
//! `unresolvable` IS printed — `sema::dump` appends ` UNR` to the
//! identifier line and suppresses the `[...]` decl bracket
//! (SemResolve.cpp:125-126, `dump.rs`'s `enter_identifier`) — but the
//! differential cannot see it through a `with`, because
//! `visit(WithStatementNode *)` reports "with statement is not supported"
//! whenever `compile_` is set and hermesc then exits before dumping anything
//! (verified against `hermesc -dump-sema`: stdout empty, exit code 2). So
//! `error-with.js` in the corpus pins only the diagnostic and the exit code,
//! and `tests/resolver.rs`'s
//! `with_statement_unresolves_identifiers_above_its_depth` is what pins this
//! pass's actual effect, including the ` UNR` rendering.
//!
//! ## Deviations
//!
//! - C++ dispatches through `visitESTreeNodeNoReplace` and therefore has to
//!   supply no-op `incRecursionDepth`/`decRecursionDepth` hooks
//!   (SemanticResolver.h:693-698). This port's read-only
//!   [`ast::visitor::Visitor`] has no depth hooks at all, so those two
//!   members have no counterpart.
//! - C++'s generic `visit(Node *)` overload (SemanticResolver.h:687-689) and
//!   its `visit(IdentifierNode *)` overload become the two arms of the single
//!   `visit_node` below, exactly as `mod.rs`'s `DeclHoisting` does it.
//! - The constructor is private in C++ (`run` is the only entry point); here
//!   it is inlined into [`Unresolver::run`] for the same effect.

use ast::node::{Identifier, Node};
use ast::visitor::Visitor;

use crate::sem_context::SemContext;

/// Port of `hermes::sema::Unresolver` (SemanticResolver.h:681-711).
pub(super) struct Unresolver<'sc> {
    sem_ctx: &'sc mut SemContext,
    /// Depth of the scope which contains the construct which could shadow
    /// variables dynamically.
    /// e.g. the depth of the function containing a local `eval()`.
    depth: u32,
}

impl Unresolver<'_> {
    /// Mark all declarations that are at a lower depth than \p depth as
    /// unresolvable, starting at \p root. Port of `Unresolver::run`
    /// (SemanticResolver.cpp:3200-3204).
    ///
    /// No `GCLock` is threaded through, unlike every resolver visit: this
    /// pass allocates nothing and only reads and re-decorates existing
    /// nodes.
    pub(super) fn run<'gc>(
        sem_ctx: &mut SemContext,
        depth: u32,
        root: &'gc Node<'gc>,
    ) {
        let mut unresolver = Unresolver { sem_ctx, depth };
        // visitESTreeNodeNoReplace(unresolver, root): dispatches on `root`
        // itself, so an `Identifier` root gets the identifier arm.
        unresolver.visit_node(root);
    }

    /// Port of `Unresolver::visit(ESTree::IdentifierNode *node)`
    /// (SemanticResolver.cpp:3206-3224).
    ///
    /// `node` is the enclosing `Node` because `set_expression_decl` needs its
    /// `NodeId` (C++ keys the same side table by the node pointer it already
    /// has).
    fn visit_identifier<'gc>(
        &mut self,
        node: &'gc Node<'gc>,
        ident: &'gc Identifier<'gc>,
    ) {
        if ident.unresolvable.get() {
            return;
        }

        if let Some(decl) = self.sem_ctx.get_expression_decl(ident) {
            // `LexicalScope *scope = decl->scope;` — C++ dereferences it
            // unconditionally below, i.e. a scope-less ("special") `Decl`
            // would be a null dereference there. Nothing in this port
            // creates a `Decl` without a scope (`new_decl_in_scope` is the
            // only constructor, and `Decl::scope` is `Option` purely to
            // mirror the C++ field's nullability), so the `expect` records
            // that rather than inventing a behavior C++ doesn't have.
            let scope = self
                .sem_ctx
                .decl(decl)
                .scope
                .expect("a resolved Decl always has a scope");

            // The depth of this identifier's declaration is less than the
            // `eval`/`with` declaration that could shadow it, so we must
            // declare this identifier as unresolvable.
            if self.sem_ctx.scope(scope).depth < self.depth {
                self.sem_ctx.set_expression_decl(node.node_id(), ident, None);
                ident.unresolvable.set(true);
            }
        }

        node.visit_children(self);
    }
}

impl<'gc> Visitor<'gc> for Unresolver<'_> {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        match node {
            Node::Identifier(ident) => self.visit_identifier(node, ident),
            // void visit(ESTree::Node *node) {
            //   visitESTreeChildren(*this, node);
            // }
            _ => node.visit_children(self),
        }
    }
}
