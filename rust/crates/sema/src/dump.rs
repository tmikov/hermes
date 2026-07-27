/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of `hermes::sema::ASTPrinter` and the untyped arm of `semDump`
//! (`lib/Sema/SemResolve.cpp:20-157,254-293`). Byte-exact text dumper (the
//! `-dump-sema` AST half, paired with Task 5's `SemContextDumper` for the
//! `SemContext` half) that the differential oracle depends on — every
//! space, quote, and (see below) quirk is transcribed straight from the
//! C++ `<<` chain it replaces.
//!
//! ## `ESTreeVisit` mapping
//!
//! C++ dispatches through the `ESTreeVisit`/`Node::visit` protocol: each
//! concrete node's macro-generated `visit(Visitor&)` calls
//! `V.shouldVisit(this)` (skip entirely if false), `V.enter(this)`,
//! `ESTreeVisit(V, child)` for each child field in `.def` order, then
//! `V.leave(this)` — with `enter`/`leave`/`shouldVisit` overloadable per
//! concrete node type (`BinaryExpressionNode`, `IdentifierNode`,
//! `TypeAnnotationNode`) and falling back to the generic `Node*` overload
//! otherwise.
//!
//! `ast::Visitor` (`rust/crates/ast/src/visitor.rs`) is far thinner: one
//! method, `visit_node(&mut self, node)`, whose default body just recurses
//! via `node.visit_children(self)` — no `shouldVisit`/`enter`/`leave` split
//! and no per-node-kind override point. Rather than force that shape to
//! fit (which would mean re-deriving the split manually at every call site
//! anyway), `AstPrinter` implements `Visitor` with a `visit_node` that
//! reconstructs the same three-step protocol directly:
//!
//! ```text
//! fn visit_node(node):
//!     if !should_visit(node): return   // shouldVisit
//!     enter(node)                      // enter — dispatches on node kind
//!     node.visit_children(self)        // ESTreeVisit(V, child) for each child
//!     leave(node)                      // leave
//! ```
//!
//! `node.visit_children` (generated per `.def` entry, `ast::node`) is
//! exactly the C++ macro's per-node child-field enumeration, so reusing it
//! here reproduces the same traversal order for every node kind for free.
//! `enter`/`leave` internally match on node kind to reach the
//! `BinaryExpression`/`Identifier` special cases, mirroring the C++
//! overload set.
//!
//! ## The `BinaryExpression` `+`/`-` linearization and its `BinOp` quirk
//!
//! `enter(BinaryExpressionNode*)` (cpp:70-95) prints the node itself, then
//! — for `+`/`-` only — flattens the left-recursive chain via
//! `linearizeLeft` and walks it iteratively instead of recursing normally,
//! setting `parentLinearized_ = true` right before returning. Because the
//! macro-generated `visit()` *unconditionally* calls `ESTreeVisit(V,
//! _left)`/`ESTreeVisit(V, _right)` right after `enter()` returns, that
//! flag is what suppresses the would-be duplicate re-visit of the two
//! children already handled manually inside `enter()`; `leave()` resets it
//! so an outer (non-linearized) `BinaryExpression` isn't affected.
//!
//! The Rust port reconstructs this exactly: `enter_binary_expression`
//! performs the manual traversal (via recursive `self.visit_node` calls,
//! matching `->visit(*this)`) and sets `self.parent_linearized = true`;
//! back in `visit_node`, `node.visit_children(self)` then tries to visit
//! `left`/`right` again but `should_visit` returns `false` (mirroring the
//! generic `shouldVisit(Node*) { return !parentLinearized_; }` override),
//! so both calls are no-ops; `leave` unconditionally resets the flag to
//! `false` for every `BinaryExpression`, linearized or not (matching
//! `leave(BinaryExpressionNode*)`, cpp:129-133).
//!
//! **Quirk, reproduced on purpose:** inside the loop that prints each
//! `BinOp` line (cpp:79-84), the C++ prints `list[0]->_operator->str()`
//! *every iteration* — not `e->_operator`. For a mixed chain like
//! `1 + 2 - 3` (`list[0]` is the `+` node, `list[1]` is the `-` node),
//! this means **both** `BinOp` lines print `+`, never `-`. This looks like
//! an oversight in the C++ (probably meant `e->_operator`), but the
//! differential oracle compares byte-for-byte against real `hermesc`
//! output, so it is reproduced verbatim here rather than "fixed" — see
//! `enter_binary_expression` and the `linearized_binary_1_plus_2_minus_3`
//! test, which locks this in.
//!
//! ## `getExpressionDecl` on an unresolvable identifier
//!
//! `enter(IdentifierNode*)` (cpp:101-102) calls `getExpressionDecl`
//! unconditionally, right after `getDeclarationDecl` — even when the
//! identifier `isUnresolvable()`. C++'s `getExpressionDecl` has
//! `assert(!node->isUnresolvable())` (SemContext.h:559-561), which is
//! compiled out in `NDEBUG`/Release builds; in that configuration the call
//! is harmless, because the *only* call site that ever marks an identifier
//! unresolvable (`Unresolver::visit`, `SemanticResolver.cpp:3192-3206`)
//! always clears the "have expression decl" bit first via
//! `setExpressionDecl(node, nullptr)` — so `getExpressionDecl` would
//! return `nullptr` there regardless of the assert. This port's
//! `SemContext::get_expression_decl` (`sem_context.rs`) uses `assert!`,
//! which Rust never compiles out, so calling it unconditionally here would
//! panic on exactly that (rare, `with`-shadowing) case instead of quietly
//! returning `None` the way a Release C++ build does. `enter_identifier`
//! below checks `unresolvable` first and substitutes `None` in that case —
//! reproducing the *value* a Release C++ build produces, without the
//! panic a literal transcription would introduce in every Rust build
//! configuration.
//!
//! ## `should_visit` and `TypeAnnotation`
//!
//! `shouldVisit(TypeAnnotationNode*) { return false; }` (cpp:52, Flow-only
//! in C++) unconditionally skips the Flow type-annotation wrapper node —
//! not just hiding its print, but (per the macro above) also skipping
//! entirely into its subtree. Ported unconditionally here (no `#if
//! HERMES_PARSE_FLOW`-equivalent gate — this crate doesn't split builds by
//! dialect) since it doesn't matter for untyped ASTs and matters once
//! Flow/TS corpora are dumped.

use ast::context::GCLock;
use ast::node::{BinaryExpression, Identifier, Node};
use ast::visitor::Visitor;
use ast::SemaId;

use crate::dump_context::{push_atom, push_indent, push_str, SemContextDumper};
use crate::ids::{FunctionInfoId, ScopeId};
use crate::sem_context::SemContext;

/// Port of `hermes::sema::semDump`'s untyped arm (`SemResolve.cpp:254-270`).
/// The typed/`FlowContext` arm (cpp:271-292) is deferred to the
/// FlowChecker component, per the task brief — this crate has no
/// `FlowContext` yet.
///
/// Prints `printSemContext(root_func)` + `'\n'` + an `ASTPrinter` run over
/// `root` (which itself ends with a trailing `'\n'`, cpp:48).
pub fn sem_dump<'n, 'ast, 'ctx>(
    out: &mut Vec<u8>,
    gc: &GCLock<'ast, 'ctx>,
    sem_ctx: &SemContext,
    root: &'n Node<'n>,
) {
    // "If the root is a function-like node, start the dump from its
    // FunctionInfo." (cpp:260-263)
    let root_func =
        function_like_sem_info(root).map(FunctionInfoId::from_sema_id);

    let mut sem_dumper = SemContextDumper::new();
    sem_dumper.print_sem_context(out, gc, sem_ctx, root_func);
    out.push(b'\n');

    let mut printer = AstPrinter {
        out,
        gc,
        sem_ctx,
        sem_dumper: &mut sem_dumper,
        depth: 0,
        parent_linearized: false,
    };
    printer.run(root);
}

/// Port of `hermes::sema::ASTPrinter` (`SemResolve.cpp:20-157`), untyped
/// arm only (no `flowDumper_`/`flowContext_` — see the module doc). See
/// the module doc for how this maps onto the C++ `ESTreeVisit`
/// `shouldVisit`/`enter`/`leave` protocol.
struct AstPrinter<'p, 'ast, 'ctx> {
    out: &'p mut Vec<u8>,
    gc: &'p GCLock<'ast, 'ctx>,
    sem_ctx: &'p SemContext,
    sem_dumper: &'p mut SemContextDumper,
    /// Port of `depth_` (cpp:26).
    depth: u32,
    /// Port of `parentLinearized_` (cpp:31) — see the module doc's
    /// linearization section.
    parent_linearized: bool,
}

impl<'p, 'ast, 'ctx> AstPrinter<'p, 'ast, 'ctx> {
    /// Port of `ASTPrinter::run` (cpp:46-49).
    fn run<'n>(&mut self, root: &'n Node<'n>) {
        self.visit_node(root);
        self.out.push(b'\n');
    }

    /// Port of the two `shouldVisit` overloads (cpp:52-60): the
    /// Flow-`TypeAnnotationNode`-specific one (always `false`) and the
    /// generic `Node*` one (`!parentLinearized_`).
    fn should_visit(&self, node: &Node) -> bool {
        if matches!(node, Node::TypeAnnotation(_)) {
            return false;
        }
        !self.parent_linearized
    }

    /// Dispatches to the `enter` overload matching `node`'s kind, mirroring
    /// C++ overload resolution on `enter(BinaryExpressionNode*)`,
    /// `enter(IdentifierNode*)`, and the generic `enter(Node*)` fallback
    /// (cpp:62-125).
    fn enter<'n>(&mut self, node: &'n Node<'n>) {
        match node {
            Node::BinaryExpression(bin) => {
                self.enter_binary_expression(node, bin)
            }
            Node::Identifier(ident) => self.enter_identifier(ident),
            _ => self.enter_generic(node),
        }
    }

    /// Port of the generic `enter(ESTree::Node *V)` (cpp:62-69): indent,
    /// node name, scope ref, newline. Also the first half of
    /// `enter(BinaryExpressionNode*)` (cpp:71-72), which explicitly calls
    /// this before its own special-casing.
    fn enter_generic(&mut self, node: &Node) {
        self.depth += 1;
        push_indent(self.out, self.depth - 1);
        push_str(self.out, node.node_type_str());
        self.print_scope_ref(node);
        self.out.push(b'\n');
    }

    /// Port of `printScopeRef` (cpp:136-143).
    fn print_scope_ref(&mut self, node: &Node) {
        if let Some(scope) = node_scope(node) {
            self.out.push(b' ');
            self.sem_dumper
                .print_scope_ref(self.out, ScopeId::from_sema_id(scope));
        }
    }

    /// Port of `enter(ESTree::BinaryExpressionNode *V)` (cpp:70-95). See
    /// the module doc for the linearization protocol and the `BinOp`
    /// quirk this deliberately reproduces.
    fn enter_binary_expression<'n>(
        &mut self,
        node: &'n Node<'n>,
        bin: &'n BinaryExpression<'n>,
    ) {
        // "Still print the BinaryExpressionNode itself." (cpp:71-72)
        self.enter_generic(node);

        let op = bin.operator.get();
        if op == self.sem_ctx.kw.ident_plus
            || op == self.sem_ctx.kw.ident_minus
        {
            let list = linearize_left(self.sem_ctx, bin);

            self.visit_node(list[0].left);
            for e in &list {
                push_indent(self.out, self.depth);
                push_str(self.out, "BinOp ");
                // NOT `e.operator`: cpp:82 prints `list[0]->_operator`
                // unconditionally on every iteration — see the module
                // doc's "BinOp quirk" section.
                push_atom(self.out, self.gc, list[0].operator.get());
                self.out.push(b'\n');
                self.visit_node(e.right);
            }

            // Suppresses the re-visit `node.visit_children` is about to
            // attempt on `bin.left`/`bin.right` (both already handled
            // above); `leave` resets this. See the module doc.
            self.parent_linearized = true;
        }
    }

    /// Port of `enter(ESTree::IdentifierNode *V)` (cpp:96-125).
    fn enter_identifier(&mut self, ident: &Identifier) {
        self.depth += 1;
        push_indent(self.out, self.depth - 1);
        push_str(self.out, "Id '");
        push_atom(self.out, self.gc, ident.name.get());
        self.out.push(b'\'');

        let decl_d = self.sem_ctx.get_declaration_decl(ident);
        // See the module doc's "getExpressionDecl on an unresolvable
        // identifier" section for why this doesn't call
        // `get_expression_decl` unconditionally the way cpp:102 does.
        let expr_d = if ident.unresolvable.get() {
            None
        } else {
            self.sem_ctx.get_expression_decl(ident)
        };

        if decl_d.is_some() || expr_d.is_some() {
            push_str(self.out, " [");
            // Matches the C++ if/else-if/else (cpp:103-118) branch by
            // branch, via exhaustive destructuring instead of
            // `Option::unwrap`/`expect` (an earlier version used those and
            // clippy correctly couldn't prove them sound from a separate
            // `if decl_d.is_none() || ...` check higher up).
            match (decl_d, expr_d) {
                // "!declD" half of cpp:105.
                (None, Some(e)) => {
                    push_str(self.out, "D:E:");
                    self.sem_dumper.print_decl_ref(
                        self.out, self.gc, self.sem_ctx, e, true,
                    );
                }
                // "declD == exprD" half of cpp:105.
                (Some(d), Some(e)) if d == e => {
                    push_str(self.out, "D:E:");
                    self.sem_dumper.print_decl_ref(
                        self.out, self.gc, self.sem_ctx, e, true,
                    );
                }
                // cpp:108-112: declD and exprD both present and distinct.
                (Some(d), Some(e)) => {
                    push_str(self.out, "D:");
                    self.sem_dumper.print_decl_ref(
                        self.out, self.gc, self.sem_ctx, d, false,
                    );
                    push_str(self.out, " E:");
                    self.sem_dumper.print_decl_ref(
                        self.out, self.gc, self.sem_ctx, e, true,
                    );
                }
                // cpp:113-118: "the only remaining case", declD && !exprD.
                (Some(d), None) => {
                    push_str(self.out, "D:");
                    self.sem_dumper.print_decl_ref(
                        self.out, self.gc, self.sem_ctx, d, true,
                    );
                }
                (None, None) => {
                    unreachable!("guarded by the `if` above")
                }
            }
            self.out.push(b']');
        }
        if ident.unresolvable.get() {
            push_str(self.out, " UNR");
        }
        self.out.push(b'\n');
    }

    /// Port of the generic `leave(ESTree::Node *V)` (cpp:126-128) plus
    /// `leave(ESTree::BinaryExpressionNode *V)`'s extra flag reset
    /// (cpp:129-133).
    fn leave(&mut self, node: &Node) {
        self.depth -= 1;
        if matches!(node, Node::BinaryExpression(_)) {
            self.parent_linearized = false;
        }
    }
}

impl<'gc, 'p, 'ast, 'ctx> Visitor<'gc> for AstPrinter<'p, 'ast, 'ctx> {
    /// Reconstructs the C++ `shouldVisit`/`enter`/(children)/`leave`
    /// protocol — see the module doc.
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        if !self.should_visit(node) {
            return;
        }
        self.enter(node);
        node.visit_children(self);
        self.leave(node);
    }
}

/// Port of `ESTree::linearizeLeft` (`include/hermes/AST/ESTree.h:1438-1451`),
/// specialized to `BinaryExpression` restricted to `{+, -}` — the only
/// instantiation `ASTPrinter` ever uses (cpp:76). The general C++ template
/// compares `_operator->str()` against an arbitrary `ops` string list; since
/// the two operators of interest here are always the same interned atoms
/// `Keywords` hands out, this compares atom identity instead — cheaper, and
/// exactly equivalent given `+`/`-` are always interned through the same
/// table (never spelled differently).
///
/// Converts `((a + b) - c) + d` into `[a+b, (a+b)-c, ((a+b)-c)+d]`: the
/// last element is always `e` itself, and the true "leftmost" operand is
/// reached through `list[0].left`.
fn linearize_left<'gc>(
    sem_ctx: &SemContext,
    mut e: &'gc BinaryExpression<'gc>,
) -> Vec<&'gc BinaryExpression<'gc>> {
    let mut list = vec![e];
    while let Some(left) = e.left.as_binary_expression() {
        let op = left.operator.get();
        if op != sem_ctx.kw.ident_plus && op != sem_ctx.kw.ident_minus {
            break;
        }
        e = left;
        list.push(e);
    }
    list.reverse();
    list
}

/// The `FunctionInfo` a function-like `root`'s `sem_info` decoration points
/// at, or `None` if `root` isn't function-like. Port of the
/// `llvh::dyn_cast<ESTree::FunctionLikeNode>(root)` + `getSemInfo()` guard
/// at the top of `semDump` (cpp:261-263); enumerates the 6 node kinds that
/// carry a `sem_info` Cell (`rust/crates/ast/src/node.rs`; grep
/// `sem_info: Cell<Option<SemaId>>` — `Program` counts, since
/// `ESTREE_NODE_1_ARGS(Program, FunctionLike, ...)` makes it a
/// `FunctionLikeNode` in C++ too).
fn function_like_sem_info(node: &Node) -> Option<SemaId> {
    match node {
        Node::Program(n) => n.sem_info.get(),
        Node::FunctionExpression(n) => n.sem_info.get(),
        Node::ArrowFunctionExpression(n) => n.sem_info.get(),
        Node::FunctionDeclaration(n) => n.sem_info.get(),
        Node::ComponentDeclaration(n) => n.sem_info.get(),
        Node::HookDeclaration(n) => n.sem_info.get(),
        _ => None,
    }
}

/// The scope a scope-bearing node decorates, if any. Port of
/// `ESTree::getDecoration<ScopeDecorationBase>(n)` + `getScope()`
/// (cpp:136-142): enumerates the 15 node kinds that carry a `scope` Cell
/// (`rust/crates/ast/src/node.rs`; grep `scope: Cell<Option<SemaId>>`).
fn node_scope(node: &Node) -> Option<SemaId> {
    match node {
        Node::Program(n) => n.scope.get(),
        Node::FunctionExpression(n) => n.scope.get(),
        Node::ArrowFunctionExpression(n) => n.scope.get(),
        Node::FunctionDeclaration(n) => n.scope.get(),
        Node::ComponentDeclaration(n) => n.scope.get(),
        Node::HookDeclaration(n) => n.scope.get(),
        Node::ForInStatement(n) => n.scope.get(),
        Node::ForOfStatement(n) => n.scope.get(),
        Node::ForStatement(n) => n.scope.get(),
        Node::BlockStatement(n) => n.scope.get(),
        Node::StaticBlock(n) => n.scope.get(),
        Node::SwitchStatement(n) => n.scope.get(),
        Node::CatchClause(n) => n.scope.get(),
        Node::ClassDeclaration(n) => n.scope.get(),
        Node::ClassExpression(n) => n.scope.get(),
        _ => None,
    }
}
