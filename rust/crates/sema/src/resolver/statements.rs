/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! S2 T1: statements — loops, labeled statements, `break`/`continue` and
//! `switch`. A further `impl<'bt, 'sc, 'sm, 'ad> SemanticResolver<'bt, 'sc,
//! 'sm, 'ad>` block, split out of `resolver/mod.rs` the same way
//! `identifiers.rs` (S1 T4), `declarations.rs` (S1 T5), `expressions.rs`
//! (S1 T6) and `functions.rs` (S1 T7) were — see `identifiers.rs`'s module
//! doc for why a child module sees `mod.rs`'s private fields and helpers.
//!
//! Ports `SemanticResolver::visit(SwitchStatementNode *)`
//! (SemanticResolver.cpp:520-539), `visit(ForInStatementNode *)`
//! (cpp:541-543), `visit(ForOfStatementNode *)` (cpp:545-547),
//! `visitForInOf` (cpp:549-598), `visit(ForStatementNode *)` (cpp:600-614),
//! `visit(DoWhileStatementNode *)` (cpp:616-625),
//! `visit(WhileStatementNode *)` (cpp:626-635),
//! `visit(LabeledStatementNode *)` (cpp:637-678), the file-static
//! `getLabelDecorationBase` (cpp:680-693),
//! `visit(BreakStatementNode *)` (cpp:695-721) and
//! `visit(ContinueStatementNode *)` (cpp:723-755).
//!
//! ## `label_index` is invisible to the differential
//!
//! `SemContextDumper` never prints a label index (it is an AST decoration,
//! not a `SemContext` record), so `tests/sema_differential.rs` cannot see a
//! wrong — or missing — `label_index` at all: a `break` pointing at the
//! wrong loop is byte-identical to a correct one in `hermesc -dump-sema`
//! output. The unit tests in `tests/resolver.rs` (the `label_index` helper
//! and the eight tests around it) are the only pin on this decoration; they
//! read the `Cell`s off the tree `resolve_ast` RETURNED.
//!
//! ## THE decorate-after-children exception: `visit(SwitchStatementNode *)`
//!
//! Every other visit in this file writes its decorations before recursing,
//! satisfying `resolver/mod.rs`'s "decorate before recursing" invariant
//! directly. `visit(SwitchStatementNode *)` cannot: C++ deliberately visits
//! `_discriminant` FIRST (cpp:522, "Visit the discriminant before creating a
//! new scope") and only then calls `setLabelIndex` (cpp:526). A folding
//! discriminant (`switch (1 + 2)`) makes that child visit return `Changed`,
//! so the `SwitchStatement` is rebuilt — and a builder snapshotted before
//! the label was written would hand back `INVALID_LABEL`.
//!
//! The fix is not to reorder (that would change the C++'s observable order:
//! the label counter would advance before the discriminant's own nested
//! loops got theirs) but to make sure the label lands on the node this visit
//! RETURNS. `builder::SwitchStatement::from_node` copies `label_index` and
//! `scope` by value (node.rs), so [`SemanticResolver::visit_switch_statement`]
//! writes both decorations on the original node and only *then* creates the
//! builder, seeding it with the already-visited discriminant. The rebuilt
//! node therefore carries them, and so does the original — which matters,
//! because the original is what `currentLoopOrSwitch` points at while the
//! cases are being visited (see below).
//!
//! ## Why reading an ancestor's `label_index` is sound
//!
//! `break`/`continue` read the label index of a statement that is still
//! being visited: an enclosing loop/switch (via `current_loop` /
//! `current_loop_or_switch`) or the target of a `labelMap` entry. Those
//! `NodeRc`s always name the ORIGINAL (pre-rebuild) node, because the
//! enclosing visit cannot have rebuilt itself yet — its builder is only
//! `build()`-ed after its children (the `break`/`continue` among them) have
//! been visited. The `label_index` `Cell` on that original node was written
//! on entry, before recursing, so the value read here is the final one; and
//! since builders copy `Cell`s on rebuild, the rebuilt ancestor ends up with
//! the very same index. No fixup is needed and none of these `NodeRc`s
//! outlives the visit that installed it.

use std::collections::hash_map::Entry;

use ast::context::{GCLock, NodeRc};
use ast::node::{builder, Node, NodeField};
use ast::visitor::{Path, TransformResult, VisitorMut};
use support::diag::Subsystem;

use super::declarations::atom_str;
use super::expressions::replacement_of;
use super::{Label, SemanticResolver};

/// Port of the file-static `getLabelDecorationBase(StatementNode *)`
/// (SemanticResolver.cpp:680-693) fused with its only use, `->getLabelIndex()`
/// — C++ needs the intermediate `LabelDecorationBase *` because the
/// decoration is a base class; here it is a `Cell<u32>` field repeated on
/// each node kind, so the read is the whole function.
///
/// The `LoopStatementNode` arm (cpp:682-683) covers the five kinds in
/// `ESTree.def`'s `LoopStatement` range (ESTree.def:117-169); the
/// `Break`/`Continue` arms (cpp:686-689) are carried over for fidelity even
/// though no caller can pass one (targets are always loops, switches or
/// labeled statements).
pub(super) fn label_index_of(node: &Node) -> u32 {
    match node {
        Node::WhileStatement(n) => n.label_index.get(),
        Node::DoWhileStatement(n) => n.label_index.get(),
        Node::ForInStatement(n) => n.label_index.get(),
        Node::ForOfStatement(n) => n.label_index.get(),
        Node::ForStatement(n) => n.label_index.get(),
        Node::SwitchStatement(n) => n.label_index.get(),
        Node::BreakStatement(n) => n.label_index.get(),
        Node::ContinueStatement(n) => n.label_index.get(),
        Node::LabeledStatement(n) => n.label_index.get(),
        // llvm_unreachable("invalid node type")
        _ => {
            unreachable!("{} carries no label decoration", node.node_type_str())
        }
    }
}

/// The `ESTree::Node *&left` / `&right` / `&body` out-parameters of
/// `visitForInOf` (SemanticResolver.h:230-235), as a builder for the node
/// that owns them — the same deviation `functions.rs`'s `FuncBuilder`
/// documents. `b.left(v)` is `left = v`, and `b.build(gc)` yields `Changed`
/// exactly when at least one setter ran.
///
/// Only the two kinds that call `visitForInOf` (cpp:542, 546) can occur.
enum ForInOfBuilder<'gc> {
    In(builder::ForInStatement<'gc>),
    Of(builder::ForOfStatement<'gc>),
}

impl<'gc> ForInOfBuilder<'gc> {
    /// \pre `node` is a `ForInStatement` or a `ForOfStatement`, and its
    ///   `label_index`/`scope` decorations have already been written (they
    ///   are snapshotted here — see the module doc).
    fn from_node(node: &'gc Node<'gc>) -> ForInOfBuilder<'gc> {
        match node {
            Node::ForInStatement(n) => {
                ForInOfBuilder::In(builder::ForInStatement::from_node(n))
            }
            Node::ForOfStatement(n) => {
                ForInOfBuilder::Of(builder::ForOfStatement::from_node(n))
            }
            _ => unreachable!("visitForInOf on a {}", node.node_type_str()),
        }
    }

    fn left(&mut self, left: &'gc Node<'gc>) {
        match self {
            ForInOfBuilder::In(b) => b.left(left),
            ForInOfBuilder::Of(b) => b.left(left),
        }
    }

    fn right(&mut self, right: &'gc Node<'gc>) {
        match self {
            ForInOfBuilder::In(b) => b.right(right),
            ForInOfBuilder::Of(b) => b.right(right),
        }
    }

    fn body(&mut self, body: &'gc Node<'gc>) {
        match self {
            ForInOfBuilder::In(b) => b.body(body),
            ForInOfBuilder::Of(b) => b.body(body),
        }
    }

    /// `Changed` iff at least one setter above ran.
    fn build(self, gc: &'gc GCLock) -> TransformResult<&'gc Node<'gc>> {
        match self {
            ForInOfBuilder::In(b) => b.build(gc),
            ForInOfBuilder::Of(b) => b.build(gc),
        }
    }
}

/// The `left`/`right`/`body` a `visit(ForIn/ForOfStatementNode *)` passes to
/// `visitForInOf` (cpp:542, 546).
fn for_in_of_children<'gc>(
    node: &'gc Node<'gc>,
) -> (&'gc Node<'gc>, &'gc Node<'gc>, &'gc Node<'gc>) {
    match node {
        Node::ForInStatement(n) => (n.left, n.right, n.body),
        Node::ForOfStatement(n) => (n.left, n.right, n.body),
        _ => unreachable!("visitForInOf on a {}", node.node_type_str()),
    }
}

/// Port of `node->setLabelIndex(index)`, i.e.
/// `ESTree::LabelDecorationBase::setLabelIndex`, for the kinds this file
/// writes it on. `Break`/`Continue` write theirs through their own payload
/// (they have it in hand), so they are not listed.
fn set_label_index(node: &Node, index: u32) {
    match node {
        Node::WhileStatement(n) => n.label_index.set(index),
        Node::DoWhileStatement(n) => n.label_index.set(index),
        Node::ForInStatement(n) => n.label_index.set(index),
        Node::ForOfStatement(n) => n.label_index.set(index),
        Node::ForStatement(n) => n.label_index.set(index),
        Node::SwitchStatement(n) => n.label_index.set(index),
        Node::LabeledStatement(n) => n.label_index.set(index),
        _ => {
            unreachable!("{} carries no label decoration", node.node_type_str())
        }
    }
}

/// The saved values of the two `FunctionContext` loop/switch cursors, i.e.
/// what the C++ `llvh::SaveAndRestore` objects hold on the stack
/// (cpp:557-560, 603-606, 619-622, 629-632; only the second one for
/// `switch`, cpp:528-529).
struct LoopState {
    current_loop: Option<NodeRc>,
    current_loop_or_switch: Option<NodeRc>,
}

impl SemanticResolver<'_, '_, '_, '_> {
    // ---- allocateLabel + the two SaveAndRestore cursors ----------------

    /// Port of `node->setLabelIndex(curFunctionInfo()->allocateLabel())`,
    /// the first statement of every loop/switch/labeled visit below.
    fn allocate_label_for(&mut self, node: &Node) {
        let f = self.cur_function_info();
        let index = self.sem_ctx.function_mut(f).allocate_label();
        set_label_index(node, index);
    }

    /// Port of the pair
    /// ```text
    /// llvh::SaveAndRestore<LoopStatementNode *> saveLoop(
    ///     functionContext()->currentLoop, node);
    /// llvh::SaveAndRestore<StatementNode *> saveSwitch(
    ///     functionContext()->currentLoopOrSwitch, node);
    /// ```
    /// (cpp:557-560), used verbatim by all five loop visits.
    fn enter_loop<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> LoopState {
        let node_rc = NodeRc::from_node(gc, node);
        let fc = self.function_context_mut();
        LoopState {
            current_loop: fc.current_loop.replace(node_rc.clone()),
            current_loop_or_switch: fc.current_loop_or_switch.replace(node_rc),
        }
    }

    /// The two `SaveAndRestore` destructors, which run in reverse
    /// declaration order (`saveSwitch` first).
    fn exit_loop(&mut self, state: LoopState) {
        let fc = self.function_context_mut();
        fc.current_loop_or_switch = state.current_loop_or_switch;
        fc.current_loop = state.current_loop;
    }

    // ---- visit(SwitchStatementNode *) ----------------------------------

    /// Port of `SemanticResolver::visit(ESTree::SwitchStatementNode *node)`
    /// (SemanticResolver.cpp:520-539). **The decorate-after-children
    /// exception** — see the module doc for why the builder is created as
    /// late as it is.
    pub(super) fn visit_switch_statement<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let sw = node
            .as_switch_statement()
            .expect("visit_switch_statement: not a SwitchStatement");

        // Visit the discriminant before creating a new scope.
        let discriminant = replacement_of(self.call(
            gc,
            sw.discriminant,
            Some(Path::new(node, NodeField::discriminant)),
        ));
        if self.recursion_depth == 0 {
            // C++ `return`s here with the (possibly already replaced)
            // discriminant left in place and no label allocated. "In place"
            // under this mechanism means: hand the rebuilt node back.
            let mut b = builder::SwitchStatement::from_node(sw);
            if let Some(v) = discriminant {
                b.discriminant(v);
            }
            return b.build(gc);
        }

        self.allocate_label_for(node);

        // llvh::SaveAndRestore<StatementNode *> saveSwitch(
        //     functionContext()->currentLoopOrSwitch, node);
        //
        // A switch is not a loop, so `currentLoop` is deliberately left
        // alone: an unlabeled `continue` inside a switch targets the
        // enclosing LOOP (cpp:746-748).
        let saved_switch = self
            .function_context_mut()
            .current_loop_or_switch
            .replace(NodeRc::from_node(gc, node));

        let scope_state = self.enter_scope(Some(node), false);
        // Only process a lexical scope if there are declarations in it.
        // (`process_collected_declarations` is exactly C++'s `if (declsOpt)
        // processDeclarations(*declsOpt)` — see its doc comment.)
        self.process_collected_declarations(gc, node);

        // Both decorations are on `node` by now, so the snapshot the
        // builder takes here carries them into the rebuilt node.
        let mut b = builder::SwitchStatement::from_node(sw);
        if let Some(v) = discriminant {
            b.discriminant(v);
        }
        // visitESTreeNodeList(*this, node->_cases, node);
        if let Some(cases) =
            self.visit_node_list(gc, sw.cases, node, NodeField::cases)
        {
            b.cases(cases);
        }
        let result = b.build(gc);

        self.exit_scope(scope_state);
        self.function_context_mut().current_loop_or_switch = saved_switch;
        result
    }

    // ---- visit(ForIn/ForOfStatementNode *) + visitForInOf --------------

    /// Port of `SemanticResolver::visit(ESTree::ForInStatementNode *node)`
    /// (cpp:541-543) and `visit(ESTree::ForOfStatementNode *node)`
    /// (cpp:545-547), both of which are a single `visitForInOf` call, fused
    /// with `visitForInOf` itself (cpp:549-598).
    ///
    /// C++'s `scopeDeco` parameter is not ported: both call sites pass
    /// `node` for it (cpp:542, 546).
    pub(super) fn visit_for_in_of<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        self.allocate_label_for(node);
        let loop_state = self.enter_loop(gc, node);

        // ScopeRAII nameScope{*this, scopeDeco};
        let scope_state = self.enter_scope(Some(node), false);
        self.process_collected_declarations(gc, node);

        let result = self.visit_for_in_of_children(gc, node);

        self.exit_scope(scope_state);
        self.exit_loop(loop_state);
        result
    }

    /// The part of `visitForInOf` (cpp:567-597) that runs inside the scope,
    /// split out so that the `if (recursionDepth_ == 0) return;` at cpp:
    /// 568-569 — which in C++ just runs the `ScopeRAII`/`SaveAndRestore`
    /// destructors — is a plain `return` here too.
    fn visit_for_in_of_children<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let (left, right, body) = for_in_of_children(node);
        // `from_node` runs after the label and the scope were written on
        // `node`, so the rebuilt node carries both.
        let mut b = ForInOfBuilder::from_node(node);

        // visitESTreeNode(*this, left, node);
        //
        // C++ re-reads `left` below through the `Node *&`, i.e. sees
        // whatever the visit replaced it with; `left_node` is that value.
        let left_node =
            match self.call(gc, left, Some(Path::new(node, NodeField::left))) {
                TransformResult::Changed(v) => {
                    b.left(v);
                    v
                }
                TransformResult::Unchanged => left,
                other => unreachable!(
                    "the resolver never removes or expands a child: {other:?}"
                ),
            };
        if self.recursion_depth == 0 {
            return b.build(gc);
        }

        // Ensure the initializer is valid.
        if let Node::VariableDeclaration(vd) = left_node {
            debug_assert_eq!(
                vd.declarations.iter().count(),
                1,
                "for-in/for-of must have a single binding"
            );

            let Some(Node::VariableDeclarator(declarator)) =
                vd.declarations.iter().next()
            else {
                panic!("for-in/for-of binding is not a VariableDeclarator")
            };

            if let Some(init) = declarator.init {
                // The `strict`/`kind` reads are hoisted out of the `else
                // if` so that the immutable borrows of `self` they need end
                // before `self.sm` is borrowed mutably below.
                let is_for_in = matches!(node, Node::ForInStatement(_));
                let strict =
                    self.sem_ctx.function(self.cur_function_info()).strict;
                let is_var = vd.kind.get() == self.kw().ident_var;
                if declarator.id.is_pattern() {
                    self.sm.error_range(
                        init.range(),
                        "destructuring declaration cannot be initialized \
                         in for-in/for-of loop",
                    );
                } else if !(is_for_in && !strict && is_var) {
                    self.sm.error_range(
                        init.range(),
                        "for-in/for-of variable declaration may not be \
                         initialized",
                    );
                }
            }
        } else {
            self.validate_assignment_target(left_node);
        }

        // visitESTreeNode(*this, right, node);
        if let Some(v) = replacement_of(self.call(
            gc,
            right,
            Some(Path::new(node, NodeField::right)),
        )) {
            b.right(v);
        }
        // visitESTreeNode(*this, body, node);
        if let Some(v) = replacement_of(self.call(
            gc,
            body,
            Some(Path::new(node, NodeField::body)),
        )) {
            b.body(v);
        }
        b.build(gc)
    }

    // ---- visit(ForStatementNode *) -------------------------------------

    /// Port of `SemanticResolver::visit(ESTree::ForStatementNode *node)`
    /// (SemanticResolver.cpp:600-614).
    pub(super) fn visit_for_statement<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        self.allocate_label_for(node);
        let loop_state = self.enter_loop(gc, node);

        let scope_state = self.enter_scope(Some(node), false);
        self.process_collected_declarations(gc, node);
        // visitESTreeChildren(*this, node) — still inside the scope, and
        // after both decorations, so a rebuild keeps them (see the module
        // doc's "decorate before recursing" reference).
        let result = node.visit_children_mut(gc, self);

        self.exit_scope(scope_state);
        self.exit_loop(loop_state);
        result
    }

    // ---- visit(DoWhile/WhileStatementNode *) ---------------------------

    /// Port of `SemanticResolver::visit(ESTree::DoWhileStatementNode *node)`
    /// (cpp:616-625) and `visit(ESTree::WhileStatementNode *node)`
    /// (cpp:626-635) — the two are character-for-character identical apart
    /// from the node type, and neither creates a scope.
    pub(super) fn visit_while_like<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        self.allocate_label_for(node);
        let loop_state = self.enter_loop(gc, node);
        let result = node.visit_children_mut(gc, self);
        self.exit_loop(loop_state);
        result
    }

    // ---- visit(LabeledStatementNode *) ---------------------------------

    /// Port of `SemanticResolver::visit(ESTree::LabeledStatementNode *node)`
    /// (SemanticResolver.cpp:637-678).
    pub(super) fn visit_labeled_statement<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let labeled = node
            .as_labeled_statement()
            .expect("visit_labeled_statement: not a LabeledStatement");
        self.allocate_label_for(node);

        // Determine the target statement. We need to check if it directly
        // encloses a loop or another label enclosing a loop.
        let mut target_stmt: &'gc Node<'gc> = node;
        {
            let mut cur_stmt: &'gc Node<'gc> = node;
            while let Node::LabeledStatement(cur_labeled) = cur_stmt {
                if cur_labeled.body.is_loop_statement() {
                    target_stmt = cur_labeled.body;
                    break;
                }
                cur_stmt = cur_labeled.body;
            }
        }
        debug_assert!(
            target_stmt.is_loop_statement()
                || matches!(target_stmt, Node::LabeledStatement(_)),
            "invalid target statement detected for label"
        );

        // auto *id = cast<IdentifierNode>(node->_label);
        let Node::Identifier(id) = labeled.label else {
            panic!(
                "LabeledStatement.label is a {}, not an Identifier",
                labeled.label.node_type_str()
            )
        };
        let name = id.name.get();

        // Define the new label, checking for a previous definition.
        //
        // `labelMap.try_emplace(name, ...)` inserts only if absent and
        // reports the existing entry otherwise, which is precisely what the
        // `Entry` API below does; `inserted` is C++'s `insertRes.second`.
        let mut inserted = true;
        let mut prev_declaration: Option<NodeRc> = None;
        match self.function_context_mut().label_map.entry(name) {
            Entry::Occupied(e) => {
                inserted = false;
                prev_declaration = Some(e.get().declaration_node.clone());
            }
            Entry::Vacant(e) => {
                e.insert(Label {
                    declaration_node: NodeRc::from_node(gc, labeled.label),
                    target_statement: NodeRc::from_node(gc, target_stmt),
                });
            }
        }
        if let Some(prev_declaration) = prev_declaration {
            self.sm.error_range(
                labeled.label.range(),
                format!("label '{}' is already defined", atom_str(gc, name)),
            );
            self.sm.note_range(
                prev_declaration.node(gc).range(),
                "previous definition",
                Subsystem::Unspecified,
            );
        }

        let result = node.visit_children_mut(gc, self);

        // Auto-erase the label on exit, if we inserted it.
        //
        // C++ uses `llvh::make_scope_exit` so the erase also runs on an
        // early return; this visit (cpp:637-678) has none — `_label`'s
        // range and `visitESTreeChildren` are the last two statements — so
        // an explicit erase after the children is exactly equivalent. (A
        // Rust `Drop` guard is not an option here anyway: it would have to
        // hold `&mut self` across the children visit. See
        // `resolver/mod.rs`'s "ScopeRAII / FunctionContext are explicit
        // push/pop pairs" note.)
        if inserted {
            self.function_context_mut().label_map.remove(&name);
        }
        result
    }

    // ---- visit(BreakStatementNode *) -----------------------------------

    /// Port of `SemanticResolver::visit(ESTree::BreakStatementNode *node)`
    /// (SemanticResolver.cpp:695-721).
    pub(super) fn visit_break_statement<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let brk = node
            .as_break_statement()
            .expect("visit_break_statement: not a BreakStatement");

        if let Some(label_node) = brk.label {
            let name = label_identifier_name(label_node);
            // `it->second.targetStatement`, cloned out so the `labelMap`
            // borrow ends before `self.sm` is used.
            let target = self
                .function_context()
                .label_map
                .get(&name)
                .map(|l| l.target_statement.clone());
            match target {
                Some(target) => {
                    brk.label_index.set(label_index_of(target.node(gc)));
                }
                None => {
                    self.sm.error_range(
                        label_node.range(),
                        format!(
                            "label '{}' is not defined",
                            atom_str(gc, name)
                        ),
                    );
                }
            }
        } else {
            let target = self.function_context().current_loop_or_switch.clone();
            match target {
                Some(target) => {
                    brk.label_index.set(label_index_of(target.node(gc)));
                }
                None => {
                    self.sm.error_range(
                        node.range(),
                        "'break' not within a loop or a switch",
                    );
                }
            }
        }

        node.visit_children_mut(gc, self)
    }

    // ---- visit(ContinueStatementNode *) --------------------------------

    /// Port of `SemanticResolver::visit(ESTree::ContinueStatementNode
    /// *node)` (SemanticResolver.cpp:723-755).
    pub(super) fn visit_continue_statement<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let cont = node
            .as_continue_statement()
            .expect("visit_continue_statement: not a ContinueStatement");

        if let Some(label_node) = cont.label {
            let name = label_identifier_name(label_node);
            let found = self.function_context().label_map.get(&name).map(|l| {
                (l.target_statement.clone(), l.declaration_node.clone())
            });
            match found {
                Some((target, declaration_node)) => {
                    if target.node(gc).is_loop_statement() {
                        cont.label_index.set(label_index_of(target.node(gc)));
                    } else {
                        self.sm.error_range(
                            label_node.range(),
                            format!(
                                "'continue' label '{}' is not a loop label",
                                atom_str(gc, name)
                            ),
                        );
                        self.sm.note_range(
                            declaration_node.node(gc).range(),
                            "label defined here",
                            Subsystem::Unspecified,
                        );
                    }
                }
                None => {
                    self.sm.error_range(
                        label_node.range(),
                        format!(
                            "label '{}' is not defined",
                            atom_str(gc, name)
                        ),
                    );
                }
            }
        } else {
            let target = self.function_context().current_loop.clone();
            match target {
                Some(target) => {
                    cont.label_index.set(label_index_of(target.node(gc)));
                }
                None => {
                    self.sm.error_range(
                        node.range(),
                        "'continue' not within a loop",
                    );
                }
            }
        }

        node.visit_children_mut(gc, self)
    }
}

/// The `_name` of a `break`/`continue` label. Port of
/// `llvh::cast<IdentifierNode>(node->_label)->_name` (cpp:697, 725).
fn label_identifier_name(label_node: &Node) -> ast::node_child::NodeLabel {
    match label_node {
        Node::Identifier(id) => id.name.get(),
        _ => panic!(
            "break/continue label is a {}, not an Identifier",
            label_node.node_type_str()
        ),
    }
}
