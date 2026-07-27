/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of `hermes::sema::resolveAST` (`lib/Sema/SemResolve.cpp:159-191`) —
//! the public entry point that drives [`crate::resolver::SemanticResolver`].
//!
//! Only the untyped arm is ported. The `flowContext` parameter and the
//! `#if HERMES_PARSE_FLOW` block it guards (`FlowChecker::run` + `lowerAST`,
//! cpp:178-188) belong to the FlowChecker component, which this crate does
//! not have; `declCollectorMap` exists in C++ only to hand the resolver's
//! `DeclCollector`s to that checker (`flowContext ? &declCollectorMap :
//! nullptr`), so it is not ported either. The `typed` resolver argument it
//! feeds (`flowContext != nullptr`) is therefore always false.
//!
//! `PerfSection validation("Resolving JavaScript global AST")` (cpp:165) is
//! not ported: there is no `PerfSection` in this tree.

use ast::context::{GCLock, NodeRc};
use ast::node::Node;
use support::manager::SourceErrorManager;

use crate::resolver::SemanticResolver;
use crate::sem_context::SemContext;

/// Resolve the entire AST. Port of `resolveAST` (cpp:159-191), untyped arm.
///
/// \param sem_ctx the result of resolution is stored here.
/// \param root the top-level `Program` node.
/// \param ambient_decls parsed files containing global ambient declarations
///   to insert into the global scope (C++ passes this by reference and the
///   resolver takes its address; an empty slice means "none").
/// \return false on error.
///
/// S0's resolver performs no tree rewriting, so this returns `bool` exactly
/// like C++. The signature change that returns a new root (needed once the
/// resolver becomes a `VisitorMut` and the first rewrite is ported) is owned
/// by the S1 plan.
pub fn resolve_ast<'ast>(
    gc: &'ast GCLock,
    sem_ctx: &mut SemContext,
    sm: &mut SourceErrorManager,
    root: &'ast Node<'ast>,
    ambient_decls: &[NodeRc],
) -> bool {
    // The binding table must be borrowed independently of `sem_ctx` (which
    // the resolver holds by `&mut`) — see `SemContext::binding_table`'s
    // deviation note. Declaring it before `resolver` also makes it outlive
    // the resolver, as its `Scope` guards require.
    let binding_table = sem_ctx.binding_table_rc();
    let mut resolver = SemanticResolver::new(
        &binding_table,
        sem_ctx,
        sm,
        ambient_decls,
        /* compile */ true,
    );
    resolver.run(gc, root)
}
