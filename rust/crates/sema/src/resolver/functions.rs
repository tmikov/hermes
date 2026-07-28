/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! S1 T7: functions — parameter scopes, bodies, `arguments`; S2 T2: arrow
//! functions and **rewrite #1** (expression body → block + `return`). A
//! further `impl<'bt, 'sc, 'sm, 'ad> SemanticResolver<'bt, 'sc, 'sm, 'ad>`
//! block, split out of `resolver/mod.rs` the same way `identifiers.rs`
//! (S1 T4), `declarations.rs` (S1 T5) and `expressions.rs` (S1 T6) were —
//! see `identifiers.rs`'s module doc for why a child module sees `mod.rs`'s
//! private fields and helpers.
//!
//! Ports `SemanticResolver::visit(FunctionDeclarationNode *, Node *)`
//! (SemanticResolver.cpp:233-243), `visit(FunctionExpressionNode *, Node *)`
//! (cpp:244-248), `visit(ArrowFunctionExpressionNode *, Node *)`
//! (cpp:249-275, S2 T2), `visitFunctionLike` (cpp:1646-1683),
//! `visitFunctionLikeInFunctionContext` (cpp:1685-1882),
//! `visitFunctionBodyAfterParamsVisited` (cpp:1884-1945),
//! `visitFunctionExpression` (cpp:1947-1965) and
//! `visit(ReturnStatementNode *)` (cpp:1469-1475), plus the four
//! `ESTree::` free functions they reach (`getParams`, `getBlockStatement`,
//! `isGenerator`, `isAsync` — `lib/AST/ESTree.cpp:17-36,58-80,186-205,
//! 207-226`).
//!
//! ## The `body`/`params` out-parameters become a builder
//!
//! C++ threads `ESTree::Node *&body` and `ESTree::NodeList &params` through
//! `visitFunctionLike`/`visitFunctionLikeInFunctionContext`/
//! `visitFunctionBodyAfterParamsVisited` so the recursive visitor can
//! replace the body or a parameter *in place*, in the field of the function
//! node itself. This port's AST is immutable in its structural fields, so
//! the same capability is a [`FuncBuilder`] threaded through the same three
//! functions: `b.body(v)` is `body = v`, `b.params(l)` is the list
//! assignment, and `b.build(gc)` at the end yields `Changed` exactly when
//! at least one of them ran. Consequently neither `body` nor `params` is
//! passed as an argument here — both are read off `node` via the
//! `function_like_body`/`function_like_params` helpers below, which is what
//! the C++ callers pass anyway (`funcDecl->_body`, `funcDecl->_params`).
//!
//! ## Backref fixup: `LexicalScope::hoisted_functions` (spec §3.4 (a))
//!
//! `visit(FunctionDeclarationNode *)` pushes the *node* onto
//! `curScope_->hoistedFunctions` (cpp:236) before descending into it. In
//! C++ that pointer stays valid forever because the visitor mutates nodes
//! in place. Here, a fold anywhere inside the function (`return 1 + 2;`)
//! rebuilds the `BlockStatement`, which rebuilds the `FunctionDeclaration`
//! — the pushed `NodeRc` would then point at a node that is no longer part
//! of the returned tree, i.e. a stale sema record pinning a dead
//! allocation.
//!
//! [`SemanticResolver::visit_function_declaration`] therefore remembers
//! **which scope's list** it pushed into (`self.cur_scope` at push time —
//! the *enclosing* scope, since the function's own scopes are only created
//! further down in `visitFunctionLikeInFunctionContext`) and **at which
//! index** (the push is a `Vec::push`, so `len() - 1`; nothing ever removes
//! an entry, so the index stays valid), and patches that slot to the
//! rebuilt node when — and only when — the visit returns `Changed`. See
//! `resolver/mod.rs`'s module doc for the general obligation; the other
//! `NodeRc`-holding record, `FunctionInfo::imports`, is still S4's.
//!
//! The differential cannot catch a missed fixup: `SemContextDumper`
//! prints only `hoistedFunction <name>` (dump_context.rs), and a stale node
//! carries the same name as its rebuilt copy. The unit test
//! `hoisted_function_backref_follows_a_rebuilt_node` in `tests/resolver.rs`
//! is what pins it, by comparing the recorded node's identity against the
//! `FunctionDeclaration` in the tree `resolve_ast` returned.
//!
//! ## Rewrite #1: the arrow's expression body (spec §3.4)
//!
//! C++'s arrow visit mutates the node it was handed: it allocates a
//! `ReturnStatement` + a `BlockStatement`, writes them into
//! `arrowFunc->_body`, clears `arrowFunc->_expression` (cpp:264-265) and only
//! *then* runs the normal `visitFunctionLike` flow over the new body. Both
//! structural fields are immutable here, so the rewrite instead produces a
//! **new arrow node** and every later step — `visit_function_like`,
//! `enter_function`'s `setSemInfo`, the strictness decoration, the params/body
//! walk — runs on that new node, exactly as C++'s run on the mutated one.
//!
//! `_expression` is a `Cell<bool>`, i.e. a decoration the generated builders
//! snapshot at `from_node`. Per `resolver/mod.rs`'s "decorate before
//! recursing", it is therefore written on the rewritten arrow *before* that
//! arrow is visited — never on the node the visit was called with, which is
//! discarded. A fold inside the synthesized `ReturnStatement`
//! (`() => 1 + 2`) rebuilds the arrow a second time, and that rebuild copies
//! `expression = false` along with `sem_info` and `strictness`. Pinned by
//! `a_rewritten_arrow_whose_body_folds_keeps_its_decorations` in
//! `tests/resolver.rs`; the differential sees the rewritten `BlockStatement`/
//! `ReturnStatement` (they are printed) but not the `_expression` flag.
//!
//! ## What's dormant
//!
//! - **The `MethodDefinition` constructor branch** (cpp:1652-1661) needs
//!   `curClassContext_`, which belongs to S2's class work. It is ported as
//!   a documented seam that panics if the parent really is a
//!   `MethodDefinition` — unreachable today, since `MethodDefinition`
//!   itself panics as an unhandled kind before it could ever visit a child.
//! - **The lazy-body branch** (cpp:1724-1734) reads the three
//!   `BlockStatement` `Cell`s the pre-parser sets; nothing in S1 sets them,
//!   so it is ported but only exercised in S5.
//! - **`may_reach_implicit_return`** (cpp:1939-1944) is DEFERRED to S2 —
//!   see the comment at its site.

use ast::context::{GCLock, NodeRc};
use ast::node::{builder, BlockStatement, Node, NodeField, ReturnStatement};
use ast::node_child::{NodeList, NodeMetadata, Strictness};
use ast::visitor::{Path, TransformResult, VisitorMut};

use crate::ids::FunctionInfoId;
use crate::sem_context::{Binding, ConstructorKind, DeclKind};

use super::expressions::replacement_of;
use super::unresolver::Unresolver;
use super::{
    make_strictness, FoundDirectives, SemanticResolver, DEBUG_INFO_SETTING_ALL,
};

/// Port of `astContext_.getEnableAsyncGenerators()` (Context.h:491-493),
/// read once by `visitFunctionLikeInFunctionContext` (cpp:1694).
///
/// The setting is not ported: like `DEBUG_INFO_SETTING_ALL` in `mod.rs` it
/// is a compiler-driver knob (`-Xasync-generators`, `CompilerRuntimeFlags.h:
/// 51-55`, `llvh::cl::init(false)`; wired in `CompilerDriver.cpp:1209-1210`)
/// rather than something sema computes, and this port has no driver flags.
/// `false` is hermesc's documented default, so the `if` below keeps the
/// exact shape of the C++ condition and porting the real setting later is a
/// one-line change.
const ENABLE_ASYNC_GENERATORS: bool = false;

/// Port of `astContext_.allowReturnOutsideFunction()` (Context.h:532-534),
/// read once by `visit(ReturnStatementNode *)` (cpp:1470).
///
/// Same treatment as [`ENABLE_ASYNC_GENERATORS`]. hermesc leaves it at the
/// `Context` default `false` (Context.h:243); the only thing that sets it
/// is the typed-mode IIFE wrapper (`CompilerDriver.cpp:835-837`, i.e.
/// `-typed` without `-script`), which is S2's typed-mode scope.
const ALLOW_RETURN_OUTSIDE_FUNCTION: bool = false;

/// Port of the `typed_` member (SemanticResolver.h:84) as seen from
/// `declareParams`'s `'this'`-parameter check — always `false` in this
/// port, matching `declarations.rs`'s constant of the same name (typed mode
/// is S2 scope).
const TYPED: bool = false;

/// The `ESTree::Node *&body` / `ESTree::NodeList &params` out-parameters of
/// the three C++ functions below, as a builder for the node that owns them
/// — see the module doc.
///
/// Only the three kinds `visit_node` dispatches into function visiting can
/// occur; every other kind is a programming error, not a language construct.
enum FuncBuilder<'gc> {
    Declaration(builder::FunctionDeclaration<'gc>),
    Expression(builder::FunctionExpression<'gc>),
    Arrow(builder::ArrowFunctionExpression<'gc>),
}

impl<'gc> FuncBuilder<'gc> {
    /// \pre `node` is a `FunctionDeclaration`, a `FunctionExpression` or an
    ///   `ArrowFunctionExpression`.
    fn from_node(node: &'gc Node<'gc>) -> FuncBuilder<'gc> {
        match node {
            Node::FunctionDeclaration(n) => FuncBuilder::Declaration(
                builder::FunctionDeclaration::from_node(n),
            ),
            Node::FunctionExpression(n) => FuncBuilder::Expression(
                builder::FunctionExpression::from_node(n),
            ),
            Node::ArrowFunctionExpression(n) => FuncBuilder::Arrow(
                builder::ArrowFunctionExpression::from_node(n),
            ),
            _ => panic!(
                "sema: no function builder for {}",
                node.node_type_str()
            ),
        }
    }

    /// `params = <new list>` (C++ writes through the `NodeList &`).
    fn params(&mut self, params: NodeList<'gc>) {
        match self {
            FuncBuilder::Declaration(b) => b.params(params),
            FuncBuilder::Expression(b) => b.params(params),
            FuncBuilder::Arrow(b) => b.params(params),
        }
    }

    /// `body = <new node>` (C++ writes through the `Node *&`).
    fn body(&mut self, body: &'gc Node<'gc>) {
        match self {
            FuncBuilder::Declaration(b) => b.body(body),
            FuncBuilder::Expression(b) => b.body(body),
            FuncBuilder::Arrow(b) => b.body(body),
        }
    }

    /// `Changed` iff at least one setter above ran.
    fn build(self, gc: &'gc GCLock) -> TransformResult<&'gc Node<'gc>> {
        match self {
            FuncBuilder::Declaration(b) => b.build(gc),
            FuncBuilder::Expression(b) => b.build(gc),
            FuncBuilder::Arrow(b) => b.build(gc),
        }
    }
}

/// Port of `ESTree::Node::copyLocationFrom(const Node *src)`
/// (`include/hermes/AST/ESTree.h:124-128`): copies the source range and the
/// debug location — and, deliberately, NOT `parens_`, which a freshly
/// allocated node leaves at 0 either way.
///
/// C++ constructs the node first and then calls `copyLocationFrom` on it;
/// here the location lives in the `NodeMetadata` a constructor takes, so the
/// call becomes "build the metadata to construct it with".
pub(super) fn copy_location_from<'gc>(src: &Node<'gc>) -> NodeMetadata<'gc> {
    NodeMetadata::new_with_debug(src.range(), src.metadata().debug_loc.get())
}

/// \return the `FunctionInfoId` a function-like node's `sem_info` decoration
/// names. Port of `node->getSemInfo()` (`ESTree::FunctionLikeDecoration::
/// getSemInfo`) for the one caller that needs it, `visit(
/// ArrowFunctionExpressionNode *)` (cpp:273-274); the same six kinds as
/// `mod.rs`'s `set_node_sem_info`, which is what wrote it.
fn node_sem_info(node: &Node) -> FunctionInfoId {
    let id = match node {
        Node::Program(n) => n.sem_info.get(),
        Node::FunctionExpression(n) => n.sem_info.get(),
        Node::ArrowFunctionExpression(n) => n.sem_info.get(),
        Node::FunctionDeclaration(n) => n.sem_info.get(),
        Node::ComponentDeclaration(n) => n.sem_info.get(),
        Node::HookDeclaration(n) => n.sem_info.get(),
        _ => panic!("{} is not a function-like node", node.node_type_str()),
    };
    FunctionInfoId::from_sema_id(id.expect("semInfo must be set"))
}

/// Port of `ESTree::getParams(FunctionLikeNode *)`
/// (`lib/AST/ESTree.cpp:17-36`), restricted to the kinds that can reach the
/// function visits (the `Program` `dummyParamList` case and the Flow
/// `ComponentDeclaration`/`HookDeclaration` cases have no caller here).
fn function_like_params<'gc>(node: &'gc Node<'gc>) -> NodeList<'gc> {
    match node {
        Node::FunctionExpression(n) => n.params,
        Node::ArrowFunctionExpression(n) => n.params,
        Node::FunctionDeclaration(n) => n.params,
        _ => panic!("invalid FunctionLikeNode: {}", node.node_type_str()),
    }
}

/// \return the `_body` of a function-like node. C++ has no `getBody`: each
/// `visit` overload passes `node->_body` explicitly (cpp:239, 247, 260) and
/// `getBlockStatement` (`lib/AST/ESTree.cpp:58-80`) is the downcasting
/// variant, which `visitFunctionLikeInFunctionContext` open-codes as
/// `dyn_cast<BlockStatementNode>(body)` (cpp:1703).
fn function_like_body<'gc>(node: &'gc Node<'gc>) -> &'gc Node<'gc> {
    match node {
        Node::FunctionExpression(n) => n.body,
        Node::ArrowFunctionExpression(n) => n.body,
        Node::FunctionDeclaration(n) => n.body,
        _ => panic!("invalid FunctionLikeNode: {}", node.node_type_str()),
    }
}

/// Port of `ESTree::isGenerator(FunctionLikeNode *)`
/// (`lib/AST/ESTree.cpp:186-205`). The `default` arm's
/// `assert(kind == Program)` is a `debug_assert!` here for the same reason
/// `identifiers.rs`'s `function_like_identifier` uses one.
///
/// `pub(super)` rather than private: `expressions.rs`'s
/// `visit(YieldExpressionNode *)` calls it on `functionContext()->node`
/// (cpp:1479).
pub(super) fn is_generator(node: &Node) -> bool {
    match node {
        Node::FunctionExpression(n) => n.generator.get(),
        Node::ArrowFunctionExpression(_) => false,
        Node::FunctionDeclaration(n) => n.generator.get(),
        Node::ComponentDeclaration(_) => false,
        Node::HookDeclaration(_) => false,
        _ => {
            debug_assert!(
                matches!(node, Node::Program(_)),
                "invalid FunctionLikeNode"
            );
            false
        }
    }
}

/// Port of `ESTree::isAsync(FunctionLikeNode *)`
/// (`lib/AST/ESTree.cpp:207-226`).
fn is_async(node: &Node) -> bool {
    match node {
        Node::FunctionExpression(n) => n.r#async.get(),
        Node::ArrowFunctionExpression(n) => n.r#async.get(),
        Node::FunctionDeclaration(n) => n.r#async.get(),
        Node::ComponentDeclaration(n) => n.r#async.get(),
        Node::HookDeclaration(n) => n.r#async.get(),
        _ => {
            debug_assert!(
                matches!(node, Node::Program(_)),
                "invalid FunctionLikeNode"
            );
            false
        }
    }
}

/// Port of the `isMethodDefinition` field of `FunctionLikeDecoration`
/// (`include/hermes/AST/ESTree.h`), read by `visitFunctionLike`
/// (cpp:1673). Enumerates the same six function-like kinds as `mod.rs`'s
/// `set_node_sem_info`; `Program` has the decoration too and is always
/// `false`.
fn is_method_definition(node: &Node) -> bool {
    match node {
        Node::Program(_) => false,
        Node::FunctionExpression(n) => n.is_method_definition.get(),
        Node::ArrowFunctionExpression(n) => n.is_method_definition.get(),
        Node::FunctionDeclaration(n) => n.is_method_definition.get(),
        Node::ComponentDeclaration(n) => n.is_method_definition.get(),
        Node::HookDeclaration(n) => n.is_method_definition.get(),
        _ => panic!("{} is not a function-like node", node.node_type_str()),
    }
}

/// Port of `node->strictness = ...` (cpp:1712), i.e.
/// `ESTree::FunctionLikeDecoration::strictness`. Same six kinds as
/// `mod.rs`'s `set_node_sem_info` (`visit(ProgramNode *)` writes the
/// `Program` one through the payload directly, so `Program` never reaches
/// here).
fn set_node_strictness(node: &Node, strictness: Strictness) {
    match node {
        Node::Program(n) => n.strictness.set(strictness),
        Node::FunctionExpression(n) => n.strictness.set(strictness),
        Node::ArrowFunctionExpression(n) => n.strictness.set(strictness),
        Node::FunctionDeclaration(n) => n.strictness.set(strictness),
        Node::ComponentDeclaration(n) => n.strictness.set(strictness),
        Node::HookDeclaration(n) => n.strictness.set(strictness),
        _ => panic!("{} is not a function-like node", node.node_type_str()),
    }
}

impl<'bt, 'sc, 'sm, 'ad> SemanticResolver<'bt, 'sc, 'sm, 'ad> {
    // ---- visit(FunctionDeclarationNode *, Node *) ----------------------

    /// Port of `SemanticResolver::visit(ESTree::FunctionDeclarationNode
    /// *funcDecl, ESTree::Node *parent)` (SemanticResolver.cpp:233-243),
    /// plus this port's `hoistedFunctions` backref fixup — see the module
    /// doc.
    pub(super) fn visit_function_declaration<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
        path: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let func_decl = node
            .as_function_declaration()
            .expect("visit_function_declaration: not a FunctionDeclaration");

        // curScope_->hoistedFunctions.push_back(funcDecl);
        let hoisted_scope = self.cur_scope.expect("no active scope");
        let hoisted_list =
            &mut self.sem_ctx.scope_mut(hoisted_scope).hoisted_functions;
        hoisted_list.push(NodeRc::from_node(gc, node));
        let hoisted_idx = hoisted_list.len() - 1;

        // llvh::cast_or_null<ESTree::IdentifierNode>(funcDecl->_id): a
        // function declaration's `_id` is either absent (`export default
        // function () {}`) or an `Identifier`; anything else would be a
        // failing `cast` in C++ and is an explicit panic here.
        let id = func_decl.id.inspect(|id_node| {
            assert!(
                matches!(id_node, Node::Identifier(_)),
                "FunctionDeclaration.id is not an Identifier"
            );
        });

        let result =
            self.visit_function_like(gc, node, id, path.map(|p| p.parent));

        // Backref fixup (spec §3.4 (a)): the node recorded above is stale
        // exactly when the visit rebuilt it. See the module doc.
        if let TransformResult::Changed(new_node) = &result {
            self.sem_ctx.scope_mut(hoisted_scope).hoisted_functions
                [hoisted_idx] = NodeRc::from_node(gc, new_node);
        }
        result
    }

    // ---- visit(FunctionExpressionNode *, Node *) -----------------------

    /// Port of `SemanticResolver::visit(ESTree::FunctionExpressionNode
    /// *funcExpr, ESTree::Node *parent)` (SemanticResolver.cpp:244-248),
    /// fused with the `visitFunctionExpression` it immediately forwards to
    /// (cpp:1947-1965) — the C++ split exists only because
    /// `visitFunctionExpression` is also called from the class-method path
    /// (S2), which passes different `body`/`params` references; with those
    /// out-parameters gone (see the module doc) the two collapse into one.
    pub(super) fn visit_function_expression<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
        path: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let func_expr = node
            .as_function_expression()
            .expect("visit_function_expression: not a FunctionExpression");
        let parent = path.map(|p| p.parent);

        // dyn_cast_or_null<IdentifierNode>(node->_id)
        let ident_node = match func_expr.id {
            Some(n) if matches!(n, Node::Identifier(_)) => Some(n),
            _ => None,
        };

        let Some(ident_node) = ident_node else {
            // Otherwise, no extra scope needed, just move on.
            return self.visit_function_like(gc, node, None, parent);
        };

        // If there is a name, declare it.
        let ident = ident_node
            .as_identifier()
            .expect("checked to be an Identifier above");
        let name = ident.name.get();
        let scope_state = self.enter_scope(Some(node), false);
        let cur_scope = self.cur_scope.expect("just entered a scope");
        let decl = self.sem_ctx.new_decl_in_scope_default(
            name,
            DeclKind::FunctionExprName,
            cur_scope,
        );
        self.sem_ctx.set_declaration_decl(
            ident_node.node_id(),
            ident,
            Some(decl),
        );
        self.binding_table.try_emplace(
            name,
            Binding::new(decl, Some(NodeRc::from_node(gc, ident_node))),
        );
        let result =
            self.visit_function_like(gc, node, Some(ident_node), parent);
        self.exit_scope(scope_state);
        result
    }

    // ---- visit(ArrowFunctionExpressionNode *, Node *) ------------------

    /// Port of `SemanticResolver::visit(ESTree::ArrowFunctionExpressionNode
    /// *arrowFunc, ESTree::Node *parent)` (SemanticResolver.cpp:249-275),
    /// **rewrite #1** included — see the module doc for why the rewrite
    /// allocates a new arrow instead of mutating this one.
    pub(super) fn visit_arrow_function_expression<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
        path: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let arrow = node.as_arrow_function_expression().expect(
            "visit_arrow_function_expression: not an ArrowFunctionExpression",
        );

        // Convert expression functions to a full-body to simplify IRGen.
        //
        // `rewritten_node` is C++'s `arrowFunc` after the mutation at
        // cpp:264-265: everything below reads the body/params off it, and
        // `expression = false` is written on it BEFORE it is visited (see
        // the module doc's "Rewrite #1").
        let mut rewritten = false;
        let rewritten_node: &'gc Node<'gc> = if self.compile()
            && arrow.expression.get()
        {
            let ret_stmt = gc.alloc(Node::ReturnStatement(
                ReturnStatement::new(
                    copy_location_from(arrow.body),
                    Some(arrow.body),
                ),
            ));
            // ESTree::NodeList stmtList; stmtList.push_back(*retStmt);
            let stmt_list = NodeList::from_iter(gc, [ret_stmt]);
            let block_stmt =
                gc.alloc(Node::BlockStatement(BlockStatement::new(
                    copy_location_from(arrow.body),
                    stmt_list,
                    /* implicit */ true,
                )));

            // arrowFunc->_body = blockStmt;
            let mut b = builder::ArrowFunctionExpression::from_node(arrow);
            b.body(block_stmt);
            let new_node = b.build_forced(gc);
            // arrowFunc->_expression = false;
            new_node
                .as_arrow_function_expression()
                .expect("the arrow builder builds an arrow")
                .expression
                .set(false);
            rewritten = true;
            new_node
        } else {
            node
        };

        let result = self.visit_function_like(
            gc,
            rewritten_node,
            None,
            path.map(|p| p.parent),
        );

        // `visit_function_like` has popped the arrow's own `FunctionContext`,
        // so `cur_function_info()` is the ENCLOSING function — which is what
        // C++'s `curFunctionInfo()` names here too, the arrow's
        // `FunctionContext` having been destroyed on return from
        // `visitFunctionLike`.
        let enclosing = self.cur_function_info();
        self.sem_ctx.function_mut(enclosing).contains_arrow_functions = true;
        // `arrowFunc->getSemInfo()`: the arrow's OWN FunctionInfo, decorated
        // by `enter_function` on `rewritten_node` during the visit above. A
        // rebuild inside `visit_function_like` copies the decoration, so
        // reading it off the pre-rebuild node is the same value.
        let arrow_info = node_sem_info(rewritten_node);
        let uses = self
            .sem_ctx
            .function(enclosing)
            .contains_arrow_functions_using_arguments
            || self
                .sem_ctx
                .function(arrow_info)
                .contains_arrow_functions_using_arguments
            || self.sem_ctx.function(arrow_info).uses_arguments;
        self.sem_ctx
            .function_mut(enclosing)
            .contains_arrow_functions_using_arguments = uses;

        // The rewrite itself is a replacement, so a `visit_function_like`
        // that changed nothing further must still hand back the new arrow.
        match result {
            TransformResult::Unchanged if rewritten => {
                TransformResult::Changed(rewritten_node)
            }
            other => other,
        }
    }

    // ---- visitFunctionLike ---------------------------------------------

    /// Port of `SemanticResolver::visitFunctionLike`
    /// (SemanticResolver.cpp:1646-1683). The `body`/`params`
    /// out-parameters are gone — see the module doc.
    fn visit_function_like<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
        id: Option<&'gc Node<'gc>>,
        parent: Option<&'gc Node<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let cons_kind = ConstructorKind::None;
        if matches!(parent, Some(Node::MethodDefinition(_))) {
            // S2 SEAM (cpp:1652-1661): a `MethodDefinition` parent whose
            // `_kind` is `constructor` sets `curClassContext_->
            // hasConstructor` and derives `consKind` from
            // `curClassContext_->isDerivedClass()`. `ClassContext` belongs
            // to S2's class work, so rather than guess a `consKind` this
            // panics. Unreachable today: `MethodDefinition` has no
            // `visit_node` arm, so it panics as an unhandled kind before it
            // could visit a child.
            panic!(
                "sema S2: MethodDefinition function bodies need \
                 curClassContext_ (cpp:1652-1661)"
            );
        }

        let parent_sem_info = self.cur_function_info();
        let strict = self.sem_ctx.function(parent_sem_info).strict;
        let custom_directives =
            self.sem_ctx.function(parent_sem_info).custom_directives;
        let func_state = self.enter_function(
            gc,
            node,
            Some(parent_sem_info),
            strict,
            cons_kind,
            custom_directives,
            /* install_as_global_context */ false,
        );

        let is_arrow = matches!(node, Node::ArrowFunctionExpression(_));
        // Arrow functions should inherit their current super binding. All
        // other functions can only reference super properties if it was
        // defined as a method.
        let new_can_ref_super = if is_arrow {
            self.can_reference_super
        } else {
            is_method_definition(node)
        };
        let saved_can_ref_super = self.can_reference_super;
        self.can_reference_super = new_can_ref_super;
        // Arrow functions should inherit forbidArgumentsAsIdentifier_, all
        // other functions should reset it to false.
        let saved_forbid_arguments_as_identifier =
            self.forbid_arguments_as_identifier;
        self.forbid_arguments_as_identifier =
            if is_arrow { saved_forbid_arguments_as_identifier } else { false };

        let result = self.visit_function_like_in_function_context(gc, node, id);

        self.forbid_arguments_as_identifier =
            saved_forbid_arguments_as_identifier;
        self.can_reference_super = saved_can_ref_super;
        self.exit_function(func_state);
        result
    }

    // ---- visitFunctionLikeInFunctionContext ----------------------------

    /// Port of `SemanticResolver::visitFunctionLikeInFunctionContext`
    /// (SemanticResolver.cpp:1685-1882).
    fn visit_function_like_in_function_context<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
        id: Option<&'gc Node<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        // async generators may be lazy, in which case they aren't
        // transformed. They'll be transformed when called, so check the flag
        // to see if they're enabled to determine whether to error early.
        if self.compile()
            && is_async(node)
            && is_generator(node)
            && !ENABLE_ASYNC_GENERATORS
        {
            self.sm
                .error_range(node.range(), "async generators are unsupported");
        }

        let mut directives = FoundDirectives::default();

        // Arrow functions have their bodies turned into BlockStatement
        // before visit, but only in compile_ mode.
        let body = function_like_body(node);
        let block_body = match body {
            Node::BlockStatement(_) => Some(body),
            _ => None,
        };
        if let Some(bb) = block_body {
            let bs = bb
                .as_block_statement()
                .expect("checked to be a BlockStatement above");
            directives = self.scan_directives(bs.body.iter());
        }

        // Set the strictness if necessary.
        if directives.use_strict_node.is_some() {
            let f = self.cur_function_info();
            self.sem_ctx.function_mut(f).strict = true;
        }
        let f = self.cur_function_info();
        set_node_strictness(
            node,
            make_strictness(self.sem_ctx.function(f).strict),
        );
        if directives.source_visibility
            > self.sem_ctx.function(f).custom_directives.source_visibility
        {
            self.sem_ctx
                .function_mut(f)
                .custom_directives
                .source_visibility = directives.source_visibility;
        }
        self.sem_ctx.function_mut(f).custom_directives.always_inline =
            directives.always_inline;
        self.sem_ctx.function_mut(f).custom_directives.no_inline =
            directives.no_inline;
        self.sem_ctx.function_mut(f).custom_directives.builtin =
            directives.builtin;

        if let Some(id_node) = id {
            let ident = id_node
                .as_identifier()
                .expect("a function-like id is always an Identifier");
            // Set the expression decl of the id.
            let decl = self.sem_ctx.get_declaration_decl(ident);
            self.sem_ctx.set_expression_decl(
                id_node.node_id(),
                ident,
                decl,
            );
            self.validate_declaration_name(
                gc,
                DeclKind::FunctionExprName,
                id_node,
            );
        }

        if let Some(bb) = block_body {
            let bs = bb
                .as_block_statement()
                .expect("checked to be a BlockStatement above");
            if bs.is_lazy_function_body.get() {
                // Don't descend into lazy functions, don't create a scope.
                // But do record the surrounding scope in the FunctionInfo.
                //
                // C++ asserts `node->getSemInfo()` here ("semInfo must be
                // set in first pass") and then writes through it;
                // `enter_function` set exactly that decoration on the way
                // in, so `cur_function_info()` IS `node->getSemInfo()`.
                let f = self.cur_function_info();
                let cur = self.binding_table.current_scope();
                self.sem_ctx.function_mut(f).binding_table_scope = cur;
                self.sem_ctx.function_mut(f).contains_arrow_functions =
                    bs.contains_arrow_functions.get();
                self.sem_ctx
                    .function_mut(f)
                    .contains_arrow_functions_using_arguments =
                    bs.may_contain_arrow_functions_using_arguments.get();
                return TransformResult::Unchanged;
            }
        }

        // Set to false if the parameter list contains binding patterns.
        let mut simple_parameter_list = true;
        let mut has_parameter_expressions = false;
        // All parameter identifiers.
        let mut param_ids: Vec<&'gc Node<'gc>> = Vec::new();
        for param in function_like_params(node).iter() {
            simple_parameter_list &= !param.is_pattern();
            has_parameter_expressions |= self
                .extract_declared_idents_from_id(Some(param), &mut param_ids);
        }
        let f = self.cur_function_info();
        self.sem_ctx.function_mut(f).simple_parameter_list =
            simple_parameter_list;
        self.sem_ctx.function_mut(f).has_parameter_expressions =
            has_parameter_expressions;

        if !simple_parameter_list {
            if let Some(use_strict_node) = directives.use_strict_node {
                self.sm.error_range(
                    use_strict_node.range(),
                    "'use strict' not allowed inside function with \
                     non-simple parameter list",
                );
            }
        }

        // Whether parameters must be unique.
        let unique_params = !simple_parameter_list
            || self.sem_ctx.function(self.cur_function_info()).strict
            || matches!(node, Node::ArrowFunctionExpression(_));

        // Do we have a parameter named "arguments".
        let mut has_parameter_named_arguments = false;

        // Everything above only writes `node`'s own decorations
        // (`strictness`; `sem_info` in `enter_function`; `scope` in
        // `visit_function_expression`), so the builder — which snapshots
        // them — is created here, before the first child visit. See
        // `resolver/mod.rs`'s "decorate before recursing".
        let mut b = FuncBuilder::from_node(node);

        // Do not visit the identifier node, because that would try to
        // resolve it in an incorrect scope!
        // visitESTreeNode(*this, getIdentifier(node), node);

        // 'await' forbidden outside async functions.
        let saved_forbid_await_expression = self.forbid_await_expression;
        self.forbid_await_expression = !is_async(node);
        // Forbidden-ness of 'arguments' passes through arrow functions
        // because they use the same 'arguments'.
        let saved_forbid_special_arguments =
            self.forbid_special_arguments_reference;
        self.forbid_special_arguments_reference =
            if matches!(node, Node::ArrowFunctionExpression(_)) {
                saved_forbid_special_arguments
            } else {
                false
            };

        // Visit the parameters before we have hoisted the body
        // declarations. If there's a parameter named arguments, then the
        // parameter init expressions would refer to that declaration.
        // Note that we are not associating the function body's scope with an
        // AST node. It should be accessed from
        // FunctionInfo::getFunctionScope().
        if has_parameter_expressions {
            // Declare parameters in a separate scope, so that capturing
            // functions in the params don't capture the function's scope.
            let param_scope = self.enter_scope(None, false);
            self.declare_params(
                gc,
                &param_ids,
                unique_params,
                &mut has_parameter_named_arguments,
            );

            // Determine whether we need to declare "arguments", while
            // processing the parameter init expressions, in case they refer
            // to it.
            if !matches!(node, Node::ArrowFunctionExpression(_))
                && !has_parameter_named_arguments
            {
                // Declare 'arguments' temporarily while visiting the
                // parameters, and remove it prior to visiting the body,
                // which will perform its own check for conflicting bindings
                // of 'arguments'.
                let temporary_arguments_scope = self.enter_scope(None, false);
                self.declare_arguments();
                self.visit_params(gc, node, &mut b);
                self.exit_scope(temporary_arguments_scope);
            } else {
                self.visit_params(gc, node, &mut b);
            }

            // Create the function scope.
            // Note that we are not associating the new scope with an AST
            // node. It should be accessed from
            // FunctionInfo::getFunctionScope().
            let scope = self.enter_scope(
                /* scope_decoration */ None,
                /* function_body_scope */ true,
            );
            self.visit_function_body_after_params_visited(
                gc,
                node,
                &mut b,
                block_body,
                has_parameter_named_arguments,
            );
            self.exit_scope(scope);
            self.exit_scope(param_scope);
        } else {
            // No parameter expressions, emit parameters and body in the same
            // scope.
            let scope = self.enter_scope(
                /* scope_decoration */ None,
                /* function_body_scope */ true,
            );
            self.declare_params(
                gc,
                &param_ids,
                unique_params,
                &mut has_parameter_named_arguments,
            );
            self.visit_params(gc, node, &mut b);
            self.visit_function_body_after_params_visited(
                gc,
                node,
                &mut b,
                block_body,
                has_parameter_named_arguments,
            );
            self.exit_scope(scope);
        }

        self.forbid_special_arguments_reference =
            saved_forbid_special_arguments;
        self.forbid_await_expression = saved_forbid_await_expression;
        b.build(gc)
    }

    /// Declare the parameters. Port of the `declareParams` lambda
    /// (SemanticResolver.cpp:1762-1797).
    ///
    /// C++ captures `hasParameterNamedArguments` by reference; here it is an
    /// explicit out-parameter.
    fn declare_params<'gc>(
        &mut self,
        gc: &'gc GCLock,
        param_ids: &[&'gc Node<'gc>],
        unique_params: bool,
        has_parameter_named_arguments: &mut bool,
    ) {
        for &param_id_node in param_ids {
            let param_id = param_id_node
                .as_identifier()
                .expect("extractDeclaredIdentsFromID only pushes Identifiers");
            let name = param_id.name.get();

            if name == self.kw().ident_arguments {
                *has_parameter_named_arguments = true;
            }

            if self.compile() && !TYPED && name == self.kw().ident_this {
                self.sm.error_range(
                    param_id_node.range(),
                    "'this' parameter requires typed mode",
                );
            }

            self.validate_declaration_name(
                gc,
                DeclKind::Parameter,
                param_id_node,
            );

            let cur_scope = self.cur_scope.expect("no active scope");
            let param_decl = self.sem_ctx.new_decl_in_scope_default(
                name,
                DeclKind::Parameter,
                cur_scope,
            );
            self.sem_ctx.set_both_decl(
                param_id_node.node_id(),
                param_id,
                Some(param_decl),
            );
            let prev_name = self.binding_table.find(&name);
            let prev_in_cur_scope = match &prev_name {
                Some(prev) => {
                    self.sem_ctx.decl(prev.decl).scope == Some(cur_scope)
                }
                None => false,
            };
            if prev_in_cur_scope {
                // Check for parameter re-declaration.
                if unique_params {
                    self.sm.error_range(
                        param_id_node.range(),
                        format!(
                            "cannot declare two parameters with the same \
                             name '{}'",
                            String::from_utf8_lossy(gc.bytes(name))
                        ),
                    );
                }

                // Update the name binding to point to the latest
                // declaration.
                //
                // C++ mutates the `Binding *` in place, which updates the
                // entry in whatever binding scope it lives in. Here `find`
                // returns a copy, so the update is a `put` into the CURRENT
                // binding scope — equivalent, because the guard above is
                // exactly "the previous decl belongs to `curScope_`", and a
                // decl in `curScope_` can only have been created by an
                // earlier iteration of this very loop, whose `try_emplace`
                // put its binding in the current binding scope.
                self.binding_table.put(
                    name,
                    Binding::new(
                        param_decl,
                        Some(NodeRc::from_node(gc, param_id_node)),
                    ),
                );
            } else {
                // Just add the new parameter.
                self.binding_table.try_emplace(
                    name,
                    Binding::new(
                        param_decl,
                        Some(NodeRc::from_node(gc, param_id_node)),
                    ),
                );
            }
        }
    }

    /// Visits the parameters in the current scope. Port of the `visitParams`
    /// lambda (SemanticResolver.cpp:1800-1824).
    fn visit_params<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
        b: &mut FuncBuilder<'gc>,
    ) {
        let saved_is_formal_params = self.function_context().is_formal_params;
        self.function_context_mut().is_formal_params = true;

        let mut forbid_await_as_identifier = false;
        if let Node::ArrowFunctionExpression(arrow) = node {
            // ES13.0 15.3 and 15.9
            // ArrowFunction:
            //  ArrowParameters[?Yield, ?Await]
            // AsyncArrowHead :
            //  async [no LineTerminator here] ArrowFormalParameters[~Yield,
            //  +Await]
            // 'await' is forbidden as an identifier in arrow params when:
            //  - It's already forbidden in a normal arrow function.
            //  - The function is an async arrow function.
            if self.forbid_await_as_identifier || arrow.r#async.get() {
                forbid_await_as_identifier = true;
            }
        }
        let saved_forbid_await = self.forbid_await_as_identifier;
        self.forbid_await_as_identifier = forbid_await_as_identifier;

        // visitESTreeNodeList(*this, getParams(node), node);
        if let Some(new_params) = self.visit_node_list(
            gc,
            function_like_params(node),
            node,
            NodeField::params,
        ) {
            b.params(new_params);
        }
        // C++ follows this with `if (recursionDepth_ == 0) return;`, which
        // is a no-op: nothing but the two SaveAndRestore destructors runs
        // after it, and those run on the early return too.

        self.forbid_await_as_identifier = saved_forbid_await;
        self.function_context_mut().is_formal_params = saved_is_formal_params;
    }

    // ---- visitFunctionBodyAfterParamsVisited ---------------------------

    /// Port of `SemanticResolver::visitFunctionBodyAfterParamsVisited`
    /// (SemanticResolver.cpp:1884-1945). C++'s `id` parameter is not ported:
    /// its only use is the commented-out `visitESTreeNode` below.
    fn visit_function_body_after_params_visited<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
        b: &mut FuncBuilder<'gc>,
        block_body: Option<&'gc Node<'gc>>,
        has_parameter_named_arguments: bool,
    ) {
        // Do not visit the identifier node, because that would try to
        // resolve it in an incorrect scope!
        // visitESTreeNode(*this, getIdentifier(node), node);

        if DEBUG_INFO_SETTING_ALL {
            // Store the current scope, for compiling children of this
            // function in 'eval'.
            let f = self.cur_function_info();
            let cur = self.binding_table.current_scope();
            self.sem_ctx.function_mut(f).binding_table_scope = cur;
        }

        let saved_forbid_await_as_identifier = self.forbid_await_as_identifier;
        self.forbid_await_as_identifier = is_async(node);

        self.process_collected_declarations(gc, node);

        // Promote hoisted functions.
        if block_body.is_some()
            && !self.sem_ctx.function(self.cur_function_info()).strict
        {
            // `getPromotedScopedFuncDecls`/`processPromotedFuncDecls` are S3
            // scope, exactly as in `visit_program`. A loose-mode function
            // containing a block-nested function declaration is therefore
            // deliberately absent from the S1 corpus; assert that rather
            // than silently skipping the promotion.
            assert!(
                self.function_context()
                    .decls
                    .as_ref()
                    .expect("a function FunctionContext always has decls")
                    .scoped_func_decls()
                    .is_empty(),
                "sema S1: scoped function declarations are S3 scope"
            );
        }

        // Do we need to declare the "arguments" object? Only if we are not
        // an arrow, and don't have a parameter or a variable with that name.
        //
        // IMPORTANT: this is not spec compliant!
        // The spec allows aliasing of "arguments" with "var arguments", but
        // we treat the latter as a new declaration, because of IRGen
        // limitations preventing assignment to "arguments".
        if !matches!(node, Node::ArrowFunctionExpression(_))
            && !has_parameter_named_arguments
        {
            let prev_arguments =
                self.binding_table.find(&self.kw().ident_arguments);
            let needs_declare = match &prev_arguments {
                None => true,
                Some(prev) => {
                    self.sem_ctx.decl(prev.decl).scope != self.cur_scope
                }
            };
            if needs_declare {
                self.declare_arguments();
            }
        }

        // Finally visit the body.
        let body = function_like_body(node);
        if let Some(new_body) = replacement_of(self.call(
            gc,
            body,
            Some(Path::new(node, NodeField::body)),
        )) {
            b.body(new_body);
        }
        if self.recursion_depth == 0 {
            self.forbid_await_as_identifier = saved_forbid_await_as_identifier;
            return;
        }

        // Check for local eval and run the unresolver pass in non-strict
        // mode.
        // TODO: enable this when non-strict direct eval is supported.
        //
        // Ported AS DEAD CODE, with C++'s own literal `false` guard, so that
        // the day the TODO is honored this is the line that changes. The
        // `Unresolver` pass it names (SemanticResolver.h:679-711,
        // SemanticResolver.cpp:3186-3210) IS ported as of S2 T3
        // (`resolver/unresolver.rs`) — reached today only from
        // `visit(WithStatementNode *)`.
        let lex_scope = self
            .sem_ctx
            .function(self.cur_function_info())
            .get_function_body_scope();
        #[allow(clippy::overly_complex_bool_expr)]
        if false
            && self.sem_ctx.scope(lex_scope).local_eval
            && !self.sem_ctx.function(self.cur_function_info()).strict
        {
            let depth = self.sem_ctx.scope(lex_scope).depth;
            Unresolver::run(self.sem_ctx, depth, node);
        }

        // Determine whether the function can run the implicit return.
        //
        // DEFERRED to S2 (cpp:1939-1944):
        //   if (!sm_.getErrorCount())
        //     curFunctionInfo()->mayReachImplicitReturn =
        //         mayReachImplicitReturn(node);
        // `mayReachImplicitReturn` is a whole separate pass
        // (`lib/Sema/CheckImplicitReturn.cpp:320`) that, as its C++ comment
        // says, "relies on break and continue being properly resolved" —
        // i.e. on the label/loop machinery that S2 ports. `FunctionInfo::
        // mayReachImplicitReturn` keeps its default `true`
        // (SemContext.h:354), which is the conservative direction (it is
        // read only by FlowChecker.cpp:1772, also S2), and the field is
        // invisible to `-dump-sema`, so the differential is unaffected.

        self.forbid_await_as_identifier = saved_forbid_await_as_identifier;
    }

    // ---- visit(ReturnStatementNode *) ----------------------------------

    /// Port of `SemanticResolver::visit(ESTree::ReturnStatementNode
    /// *returnStmt)` (SemanticResolver.cpp:1469-1475).
    pub(super) fn visit_return_statement<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        if self.in_global_scope_context() && !ALLOW_RETURN_OUTSIDE_FUNCTION {
            self.sm
                .error_range(node.range(), "'return' not in a function");
        }
        node.visit_children_mut(gc, self)
    }

    // ---- helpers -------------------------------------------------------

    /// Port of `visitESTreeNodeList(*this, list, parent)`
    /// (RecursiveVisitor.h:263-277) for a hand-driven visit: the
    /// `NodeChild<NodeList>` shim the generated `visit_children_mut` uses is
    /// `pub(crate)` inside `ast`, so the walk is written out here.
    ///
    /// \return the rebuilt list, or `None` if no element changed.
    ///
    /// `pub(super)` rather than private: `statements.rs` drives
    /// `SwitchStatement`'s `_cases` list the same way (S2 T1).
    pub(super) fn visit_node_list<'gc>(
        &mut self,
        gc: &'gc GCLock,
        list: NodeList<'gc>,
        parent: &'gc Node<'gc>,
        field: NodeField,
    ) -> Option<NodeList<'gc>> {
        let path = Path::new(parent, field);
        let mut changed = false;
        let mut result: Vec<&'gc Node<'gc>> = Vec::new();
        for elem in list.iter() {
            match replacement_of(self.call(gc, elem, Some(path))) {
                Some(new_elem) => {
                    changed = true;
                    result.push(new_elem);
                }
                None => result.push(elem),
            }
        }
        changed.then(|| NodeList::from_iter(gc, result))
    }
}
