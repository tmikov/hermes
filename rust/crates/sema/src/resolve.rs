/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of `hermes::sema::resolveAST` (`lib/Sema/SemResolve.cpp:159-191`) and
//! `resolveASTForParser` (`cpp:295-306`) — the two public entry points that
//! drive [`crate::resolver::SemanticResolver`].
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
/// \return the resolved (possibly new) root, or `None` on error.
///
/// C++ returns `bool` and resolves in place. This port's resolver is a
/// transforming visitor (see `resolver`'s module doc): rewriting any node
/// rebuilds its ancestors, so the root that comes out is the one carrying
/// the resolution results and is what callers must go on to compile or
/// dump. `None` is C++'s `false`.
pub fn resolve_ast<'ast>(
    gc: &'ast GCLock,
    sem_ctx: &mut SemContext,
    sm: &mut SourceErrorManager,
    root: &'ast Node<'ast>,
    ambient_decls: &[NodeRc],
) -> Option<&'ast Node<'ast>> {
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

/// Perform semantic resolution of the entire AST, without preparing the AST
/// for compilation. Port of `resolveASTForParser` (`SemResolve.cpp:295-306`)
/// — the entry point `hermes-parser-wasm.cpp:104` uses. Unlike [`resolve_ast`]
/// this will not error on features we can parse but not compile, transform
/// the AST, or perform compilation-specific validation (`compile = false`);
/// it also never takes ambient declarations, matching the C++ constructor
/// call `SemanticResolver{astContext, semCtx, /* ambientDecls */ nullptr,
/// /* saveDecls */ nullptr, /* compile */ false}`.
///
/// \param root the top-level `Program` node.
/// \return the resolved (possibly new) root, or `None` on error — see
///   [`resolve_ast`]'s doc for why this differs from C++'s in-place `bool`.
pub fn resolve_ast_for_parser<'ast>(
    gc: &'ast GCLock,
    sem_ctx: &mut SemContext,
    sm: &mut SourceErrorManager,
    root: &'ast Node<'ast>,
) -> Option<&'ast Node<'ast>> {
    let binding_table = sem_ctx.binding_table_rc();
    let mut resolver = SemanticResolver::new(
        &binding_table,
        sem_ctx,
        sm,
        /* ambient_decls */ &[],
        /* compile */ false,
    );
    resolver.run(gc, root)
}
