/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of `hermes::sema::DeclCollector` (`lib/Sema/DeclCollector.{h,cpp}`,
//! whole file): collects every declaration in every scope of a function (or
//! static block) in a single upfront pass, so all of them can be hoisted
//! without re-walking the AST. Declarations are recorded against the AST
//! node that *creates* the scope they belong to (e.g. a `BlockStatement`),
//! not the node that introduces the declaration's binding — `let x` records
//! the whole `VariableDeclaration`, not the `VariableDeclarator`/
//! `Identifier` inside it; `SemanticResolver` re-derives names/bindings from
//! those nodes later.
//!
//! ## Deviations
//!
//! - **AST node references are `hermes_ast::context::NodeRc`, not `&Node`.** C++
//!   stores raw `ESTree::Node*` in `scopes_`/`scopedFuncDecls_`; this port's
//!   `DeclCollector` is meant to outlive the `GCLock` it was built under
//!   (the C++ instance is `unique_ptr`-owned per function and consulted
//!   long after the initial walk), so every stored declaration reference is
//!   pinned via `NodeRc` — the same convention `sema::sem_context` uses for
//!   its `hoistedFunctions`/`imports` backrefs (see that module's doc).
//! - **The scope-keyed map (`scopes_`) is keyed by `hermes_ast::NodeId`, not by
//!   `ESTree::Node*` identity.** Same reasoning as `sema::sem_context`'s
//!   side tables: `NodeId` is the stable, non-aliasing identity for a node
//!   (see `hermes_ast::NodeId`'s doc comment), and callers already have the
//!   scope-creating node in hand (they're the ones walking the AST) when
//!   they want its `ScopeDecls`.
//! - **`NestedRecursionDepthTracker` becomes an explicit counter.** C++'s
//!   `RecursionDepthTracker::incRecursionDepth`/`decRecursionDepth`
//!   protocol wraps *every* dispatched-to node (see
//!   `RecursiveVisitorDispatch::visit`,
//!   `include/hermes/AST/RecursiveVisitor.h:197-232`): each visit
//!   decrements a `recursionDepth_` counter; hitting exactly zero invokes
//!   `recursionDepthExceeded(node)` once and, from then on, silently
//!   refuses to visit anything else (every subsequent `incRecursionDepth`
//!   call returns `false` immediately without decrementing further — see
//!   `RecursionDepthTracker::incRecursionDepth`,
//!   RecursiveVisitor.h:721-730). A successful visit's matching
//!   `decRecursionDepth()` restores the counter afterward. This port
//!   reproduces the identical observable behavior with a plain
//!   `remaining_depth: u32` field plus an `&mut dyn FnMut(&Node)` callback:
//!   `Collector::visit_node` (the sole entry point every recursive descent
//!   funnels through, matching `RecursiveVisitorDispatch::visit`) checks
//!   and decrements `remaining_depth` before dispatching on node kind, and
//!   increments it back afterward — `remaining_depth == 0` is the
//!   "permanently tripped" state, exactly like C++'s `recursionDepth_ == 0`.
//!   `root_` itself is never subject to this check, matching C++: `runImpl`
//!   dispatches straight into `root_`'s children (or grandchildren, for the
//!   two function cases) without ever routing `root_` through the checked
//!   dispatch.
//! - **`DeclCollector::clone` (header lines 63-65) is not ported.** It
//!   exists solely to support `ESTreeClone`, which this Rust port has not
//!   built yet (same reasoning as the `ESTreeClone`-only fields skipped in
//!   `sema::sem_context`, e.g. around SemContext.h:270,397) — revisit when
//!   `ESTreeClone` lands.
//! - `getScopeDeclsForNode`/`getScopedFuncDecls` become
//!   `scope_decls_for_node`/`scoped_func_decls`. `DeclCollectorMapTy`
//!   (header lines 24-25 — a `FunctionLikeNode* -> DeclCollector` map owned
//!   by callers) is not this module's concern; it belongs to whichever
//!   later component drives per-function `DeclCollector::run` calls.

use std::collections::HashMap;

use hermes_ast::context::{GCLock, NodeRc};
use hermes_ast::node::{CatchClause, Node, VariableDeclaration};
use hermes_ast::visitor::Visitor;
use hermes_ast::NodeId;

use crate::dump_context::push_str;
use crate::keywords::Keywords;

/// All the declarations in a scope. Port of `hermes::sema::ScopeDecls`
/// (DeclCollector.h:21).
pub type ScopeDecls = Vec<NodeRc>;

/// Port of `hermes::sema::DeclCollector` (DeclCollector.h:31-180): the
/// completed result of a single collection pass over a function-like node
/// or static block. See the module doc for the `NodeRc`/`NodeId`/
/// recursion-depth deviations and the un-ported `clone`.
pub struct DeclCollector {
    /// Port of `scopes_` (DeclCollector.h:171): associates a `ScopeDecls`
    /// with the node that creates the scope, keyed by `NodeId` rather than
    /// `ESTree::Node*` identity (see the module doc).
    scopes: HashMap<NodeId, ScopeDecls>,
    /// Port of `scopedFuncDecls_` (DeclCollector.h:175): function
    /// declarations found directly inside a block scope (not at the top of
    /// a function), subject to the Annex B 3.3 special rules.
    scoped_func_decls: Vec<NodeRc>,
}

impl DeclCollector {
    /// Port of the two `run` overloads (DeclCollector.h:41-55, both
    /// forwarding to `runCommon`): collect every declaration reachable from
    /// `root` without descending into nested functions/class bodies/etc.
    /// (see the visitor overrides below). `root` is expected to be a
    /// function-like node (`FunctionDeclaration`/`FunctionExpression`/
    /// `ArrowFunctionExpression`/`Program`/`ComponentDeclaration`/
    /// `HookDeclaration`) or a `StaticBlock` — the same two families the
    /// C++ overloads accept (`FunctionLikeNode*`/`StaticBlockNode*`); any
    /// other root simply falls into the generic branch of `run_impl` below,
    /// same as C++ would via its own generic `visit(Node*)` fallback.
    ///
    /// \param recursion_depth remaining recursion depth (port of C++'s
    ///   `recursionDepth` parameter — see the module doc's recursion-depth
    ///   mapping).
    /// \param recursion_depth_exceeded handler invoked once when the
    ///   remaining depth budget hits zero.
    pub fn run<'ast, 'g_ast, 'g_ctx, 'k>(
        root: &'ast Node<'ast>,
        gc: &'ast GCLock<'g_ast, 'g_ctx>,
        kw: &'k Keywords,
        recursion_depth: u32,
        recursion_depth_exceeded: &mut dyn FnMut(&'ast Node<'ast>),
    ) -> DeclCollector {
        let mut collector = Collector {
            gc,
            kw,
            scopes: HashMap::new(),
            scoped_func_decls: Vec::new(),
            scope_stack: Vec::new(),
            remaining_depth: recursion_depth,
            on_depth_exceeded: recursion_depth_exceeded,
        };
        collector.run_impl(root);
        debug_assert!(
            collector.scope_stack.is_empty(),
            "run_impl must close every scope it opens"
        );
        DeclCollector {
            scopes: collector.scopes,
            scoped_func_decls: collector.scoped_func_decls,
        }
    }

    /// Port of `getScopeDeclsForNode` (DeclCollector.h:73-78).
    ///
    /// \param node_id the id of an AST node which could have created a
    ///   scope (the only nodes which can are decorated with a `scope`
    ///   Cell — see `sema::dump`'s `node_scope` for the exact list).
    /// \return the `ScopeDecls` if the AST node did create a (non-empty)
    ///   scope, `None` if it didn't (either it can't create a scope, or it
    ///   did but collected no declarations — see `closeScope`'s
    ///   non-empty-only rule, ported in `Collector::close_scope` below).
    pub fn scope_decls_for_node(&self, node_id: NodeId) -> Option<&ScopeDecls> {
        self.scopes.get(&node_id)
    }

    /// Port of `getScopedFuncDecls` (DeclCollector.h:80-83).
    pub fn scoped_func_decls(&self) -> &[NodeRc] {
        &self.scoped_func_decls
    }

    /// Port of `DeclCollector::dump` (cpp:99-110): debug introspection.
    /// Prefer `scope_decls_for_node`/`scoped_func_decls` for assertions —
    /// see the deviations below for why this isn't a good fit for exact
    /// golden-text comparisons the way `sema::dump`'s dumpers are.
    ///
    /// ## Deviations
    /// - C++ prints `getNodeName()` and the raw `Node*` address for the
    ///   *scope-creating* node (the `scopes_` map key) on the left of each
    ///   line. This port's `scopes` map is keyed by `NodeId` (see the
    ///   module doc), which carries no back-pointer to the node itself, so
    ///   the key side prints as `NodeId(<n>)` instead of a name+address
    ///   pair. The `Node*` address printed for each list *element* is
    ///   likewise replaced by `NodeId`: Rust's GC arena doesn't expose (or
    ///   want to expose) a stable pointer to print, and an address wouldn't
    ///   be reproducible in a test assertion anyway.
    /// - Always emitted (no `#ifndef NDEBUG` gate): Rust doesn't distinguish
    ///   debug/release the way the `NDEBUG` macro does, and every caller of
    ///   this port is a test, which always wants the output.
    /// - Iteration order over `scopes` (a `HashMap`) is unspecified, exactly
    ///   like the C++ `llvh::DenseMap` it replaces (neither ever promised a
    ///   deterministic order) — callers must not depend on line order.
    pub fn dump(&self, out: &mut Vec<u8>, gc: &GCLock, indent: u32) {
        for (node_id, decls) in &self.scopes {
            out.resize(out.len() + indent as usize, b' ');
            push_str(out, "NodeId(");
            push_str(out, &node_id.0.to_string());
            push_str(out, "):");
            for n in decls {
                let node = n.node(gc);
                out.push(b' ');
                push_str(out, node.node_type_str());
                push_str(out, "[NodeId(");
                push_str(out, &node.node_id().0.to_string());
                push_str(out, ")]");
            }
            out.push(b'\n');
        }
    }
}

/// The transient traversal state used only while a single `DeclCollector`
/// is being built. This is `DeclCollector::runImpl` plus the private state
/// (DeclCollector.h:128-179) the visitor overrides mutate; splitting it out
/// of `DeclCollector` mirrors how the finished result (`scopes_`,
/// `scopedFuncDecls_`) outlives the walk while `scopeStack_`, `kw_`, and the
/// recursion-depth bookkeeping do not need to.
struct Collector<'ast, 'g_ast, 'g_ctx, 'cb, 'k> {
    /// The *outer* reference's lifetime is tied to `'ast` (the arena
    /// lifetime nodes are visited at), not left as an independent
    /// parameter: `NodeRc::from_node` requires the `GCLock` reference and
    /// the `Node` reference it pins to share exactly one lifetime
    /// (`Node<'gc>` is invariant in `'gc`) — see `add_to_func`/
    /// `add_to_cur`/`visit_function_declaration` below, the only methods
    /// that construct a `NodeRc`. `GCLock`'s OWN two type parameters
    /// (`'g_ast, 'g_ctx`) are deliberately kept independent of `'ast`
    /// (not reused/renamed to `'ast`/`'ctx`): tying them to `'ast` as well
    /// (so that the field reads `&'ast GCLock<'ast, 'ctx>`) type-checks
    /// this module in isolation but makes every call site unsatisfiable —
    /// it forces the borrow of the caller's local `gc` variable to survive
    /// as long as `'ast`, which conflicts with `Context`/`GCLock`'s `Drop`
    /// impls once a real `Context`/`GCLock` pair is dropped at the end of
    /// the caller's scope. Keeping `GCLock`'s own parameters distinct from
    /// `'ast` avoids that trap while still letting the *reference* live as
    /// long as `'ast` requires.
    gc: &'ast GCLock<'g_ast, 'g_ctx>,
    kw: &'k Keywords,
    /// Being built into `DeclCollector::scopes`.
    scopes: HashMap<NodeId, ScopeDecls>,
    /// Being built into `DeclCollector::scoped_func_decls`.
    scoped_func_decls: Vec<NodeRc>,
    /// Port of `scopeStack_` (DeclCollector.h:179): stack of active scopes;
    /// once closed, a scope is moved into `scopes` (if non-empty).
    scope_stack: Vec<ScopeDecls>,
    /// Port of `RecursionDepthTracker::recursionDepth_`
    /// (RecursiveVisitor.h:706) — see the module doc's recursion-depth
    /// mapping.
    remaining_depth: u32,
    /// Port of `NestedRecursionDepthTracker::recursionDepthExceeded_`
    /// (RecursiveVisitor.h:748).
    on_depth_exceeded: &'cb mut dyn FnMut(&'ast Node<'ast>),
}

impl<'ast, 'g_ast, 'g_ctx, 'cb, 'k> Collector<'ast, 'g_ast, 'g_ctx, 'cb, 'k> {
    /// Port of `DeclCollector::runImpl` (cpp:64-97).
    fn run_impl(&mut self, root: &'ast Node<'ast>) {
        match root {
            Node::FunctionDeclaration(f) => {
                self.new_scope();
                // Visit the children of the body, since we don't want to
                // associate a scope with it.
                let body = f.body.as_block_statement().expect(
                    "FunctionDeclaration body is always a BlockStatement",
                );
                for c in body.body.iter() {
                    self.visit_node(c);
                }
                self.close_scope(root);
            }
            Node::FunctionExpression(f) => {
                self.new_scope();
                // Visit the children of the body, since we don't want to
                // associate a scope with it.
                let body = f.body.as_block_statement().expect(
                    "FunctionExpression body is always a BlockStatement",
                );
                for c in body.body.iter() {
                    self.visit_node(c);
                }
                self.close_scope(root);
            }
            Node::ArrowFunctionExpression(f) => {
                self.new_scope();
                // If there is a BlockStatement, don't visit it, just visit
                // its children.
                if let Some(body) = f.body.as_block_statement() {
                    for c in body.body.iter() {
                        self.visit_node(c);
                    }
                } else {
                    root.visit_children(self);
                }
                self.close_scope(root);
            }
            _ => {
                self.new_scope();
                root.visit_children(self);
                self.close_scope(root);
            }
        }
    }

    /// Port of `RecursionDepthTracker::incRecursionDepth`
    /// (RecursiveVisitor.h:721-730) — see the module doc's recursion-depth
    /// mapping. Returns `false` when `node` (and everything under it)
    /// should NOT be visited.
    fn inc_recursion_depth(&mut self, node: &'ast Node<'ast>) -> bool {
        if self.remaining_depth == 0 {
            return false;
        }
        self.remaining_depth -= 1;
        if self.remaining_depth == 0 {
            (self.on_depth_exceeded)(node);
            return false;
        }
        true
    }

    /// Port of `RecursionDepthTracker::decRecursionDepth`
    /// (RecursiveVisitor.h:735-738).
    fn dec_recursion_depth(&mut self) {
        if self.remaining_depth != 0 {
            self.remaining_depth += 1;
        }
    }

    /// Port of `DeclCollector::addToFunc` (DeclCollector.h:147-150): add a
    /// declaration to the function's own (outermost) scope.
    fn add_to_func(&mut self, node: &'ast Node<'ast>) {
        let rc = NodeRc::from_node(self.gc, node);
        self.scope_stack
            .first_mut()
            .expect("missing function scope")
            .push(rc);
    }

    /// Port of `DeclCollector::addToCur` (DeclCollector.h:151-155): add a
    /// declaration to the current (innermost) lexical scope.
    fn add_to_cur(&mut self, node: &'ast Node<'ast>) {
        let rc = NodeRc::from_node(self.gc, node);
        self.scope_stack
            .last_mut()
            .expect("no current scope")
            .push(rc);
    }

    /// Port of `DeclCollector::newScope` (DeclCollector.h:158-160).
    fn new_scope(&mut self) {
        self.scope_stack.push(Vec::new());
    }

    /// Port of `DeclCollector::closeScope` (cpp:186-199): pop the innermost
    /// scope, attaching it to `scopes` (keyed by `node`'s `NodeId`) only if
    /// it collected anything.
    fn close_scope(&mut self, node: &'ast Node<'ast>) {
        let decls = self.scope_stack.pop().expect("no scope to close");
        if !decls.is_empty() {
            let prev = self.scopes.insert(node.node_id(), decls);
            debug_assert!(prev.is_none(), "tried to collect same node twice");
        }
    }

    /// Port of `DeclCollector::visit(VariableDeclarationNode*)` (cpp:112-119).
    fn visit_variable_declaration(
        &mut self,
        node: &'ast Node<'ast>,
        vd: &'ast VariableDeclaration<'ast>,
    ) {
        if vd.kind.get() == self.kw.ident_var {
            self.add_to_func(node);
        } else {
            self.add_to_cur(node);
        }
        node.visit_children(self);
    }

    /// Port of `DeclCollector::visit(ImportDeclarationNode*)` (cpp:124-127).
    fn visit_import_declaration(&mut self, node: &'ast Node<'ast>) {
        self.add_to_cur(node);
        node.visit_children(self);
    }

    /// Port of `DeclCollector::visit(TypeAliasNode*)` (cpp:129-132, under
    /// `#if HERMES_PARSE_FLOW` in C++ — unconditional here, see the crate
    /// doc: our single node set has all dialect nodes).
    fn visit_type_alias(&mut self, node: &'ast Node<'ast>) {
        self.add_to_cur(node);
        node.visit_children(self);
    }

    /// Port of `DeclCollector::visit(TSTypeAliasDeclarationNode*)`
    /// (cpp:134-138, under `#if HERMES_PARSE_TS` in C++ — unconditional
    /// here, see the crate doc).
    fn visit_ts_type_alias_declaration(&mut self, node: &'ast Node<'ast>) {
        self.add_to_cur(node);
        node.visit_children(self);
    }

    /// Port of `DeclCollector::visit(FunctionDeclarationNode*)`
    /// (cpp:141-147): record but don't descend.
    fn visit_function_declaration(&mut self, node: &'ast Node<'ast>) {
        self.add_to_cur(node);
        if self.scope_stack.len() > 1 {
            self.scoped_func_decls.push(NodeRc::from_node(self.gc, node));
        }
    }

    /// Port of the scope-creating overrides that just wrap normal
    /// recursion in `newScope`/`closeScope`: `BlockStatementNode`,
    /// `ForStatementNode`, `ForInStatementNode`, `ForOfStatementNode`,
    /// `SwitchStatementNode` (cpp:149-184, minus `CatchClauseNode`, which
    /// needs its own method — see `visit_catch_clause`).
    fn visit_scope_creating(&mut self, node: &'ast Node<'ast>) {
        self.new_scope();
        node.visit_children(self);
        self.close_scope(node);
    }

    /// Port of `DeclCollector::visit(CatchClauseNode*)` (cpp:169-179):
    /// unlike the other scope-creating overrides, the param (if any) and
    /// the body are visited explicitly (not via `visitESTreeChildren`), so
    /// that the body — always a `BlockStatement` — gets its OWN nested
    /// scope, separate from the one holding the catch parameter binding.
    fn visit_catch_clause(
        &mut self,
        node: &'ast Node<'ast>,
        cc: &'ast CatchClause<'ast>,
    ) {
        self.new_scope();
        if let Some(param) = cc.param {
            // NOTE: this records the CatchClauseNode itself as the
            // "declaration" representing the catch parameter binding, not
            // the param node — faithfully transcribed from cpp:172, which
            // does the same (`addToCur(node)`, not `addToCur(node->_param)`).
            self.add_to_cur(node);
            self.visit_node(param);
        }
        // CatchClauseNode is supposed to have separate scopes for the body
        // and the parameters.
        self.visit_node(cc.body);
        self.close_scope(node);
    }

    /// Dispatches to the override matching `node`'s kind, mirroring C++
    /// overload resolution across all of `DeclCollector`'s `visit`
    /// overloads (DeclCollector.h:85-126) plus the generic
    /// `visit(Node*) { visitESTreeChildren(*this, node); }` fallback
    /// (DeclCollector.h:85-87) for every other node kind.
    fn dispatch(&mut self, node: &'ast Node<'ast>) {
        match node {
            Node::VariableDeclaration(vd) => {
                self.visit_variable_declaration(node, vd)
            }
            // Don't descend into class bodies.
            Node::ClassDeclaration(_) => self.add_to_cur(node),
            // Don't descend into class bodies.
            Node::ClassExpression(_) => {}
            Node::ImportDeclaration(_) => self.visit_import_declaration(node),
            Node::TypeAlias(_) => self.visit_type_alias(node),
            // Don't descend into interface bodies.
            Node::InterfaceDeclaration(_) => {}
            Node::TSTypeAliasDeclaration(_) => {
                self.visit_ts_type_alias_declaration(node)
            }
            // Don't descend into interface bodies.
            Node::TSInterfaceDeclaration(_) => {}
            Node::FunctionDeclaration(_) => {
                self.visit_function_declaration(node)
            }
            // Don't descend.
            Node::FunctionExpression(_) => {}
            // Don't descend.
            Node::ArrowFunctionExpression(_) => {}
            Node::BlockStatement(_) => self.visit_scope_creating(node),
            Node::ForStatement(_) => self.visit_scope_creating(node),
            Node::ForInStatement(_) => self.visit_scope_creating(node),
            Node::ForOfStatement(_) => self.visit_scope_creating(node),
            Node::SwitchStatement(_) => self.visit_scope_creating(node),
            Node::CatchClause(cc) => self.visit_catch_clause(node, cc),
            // Don't descend, to avoid recursion overflow.
            Node::BinaryExpression(_) => {}
            // Don't descend, to avoid recursion overflow.
            Node::AssignmentExpression(_) => {}
            _ => node.visit_children(self),
        }
    }
}

impl<'ast, 'g_ast, 'g_ctx, 'cb, 'k> Visitor<'ast>
    for Collector<'ast, 'g_ast, 'g_ctx, 'cb, 'k>
{
    /// Port of `RecursiveVisitorDispatch::visit`
    /// (RecursiveVisitor.h:197-232): the recursion-depth check wraps every
    /// dispatched node, matching the C++ dispatcher (see the module doc).
    fn visit_node(&mut self, node: &'ast Node<'ast>) {
        if !self.inc_recursion_depth(node) {
            return;
        }
        self.dispatch(node);
        self.dec_recursion_depth();
    }
}
