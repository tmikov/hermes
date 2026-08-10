/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! S1 T6: the expression visits — constant-folding integration and
//! assignment/update/unary validation. A third `impl<'bt, 'sc, 'sm, 'ad>
//! SemanticResolver<'bt, 'sc, 'sm, 'ad>` block, split out of
//! `resolver/mod.rs` the same way `identifiers.rs` (S1 T4) and
//! `declarations.rs` (S1 T5) were — see `identifiers.rs`'s module doc for
//! why a child module sees `mod.rs`'s private fields.
//!
//! Ports `SemanticResolver::visit(BinaryExpressionNode *, Node **)`
//! (SemanticResolver.cpp:405-436), `visit(AssignmentExpressionNode *)`
//! (cpp:438-462), `visit(UpdateExpressionNode *)` (cpp:464-473),
//! `visit(UnaryExpressionNode *, Node **)` (cpp:475-500),
//! `validateAssignmentTarget` (cpp:2693-2725) and `isLValue`
//! (cpp:2727-2771).
//!
//! S2 T2 adds the remaining small expression visits, all of them pure
//! validation over the current resolver state (none allocates or rewrites
//! anything): `visit(YieldExpressionNode *)` (cpp:1490-1506),
//! `visit(AwaitExpressionNode *)` (cpp:1508-1522) — the resolver's only
//! reader of `forbidAwaitExpression_` — `visit(SpreadElementNode *, Node *)`
//! (cpp:1469-1481), `visit(MetaPropertyNode *)` (cpp:837-872) and the five
//! `visit(Cover*Node *)` overloads (cpp:1572-1591). Three of those error
//! paths are defensive in the C++ too (no parse can reach them): see
//! `visit_spread_element`'s and `visit_meta_property`'s doc comments for the
//! parser evidence.
//!
//! S2 T3 adds `visit(RegExpLiteralNode *)` (cpp:821-835) — a literal, hence
//! this file rather than `statements.rs`.
//!
//! S2 T5 adds `visit(MemberExpressionNode *, Node *)` (cpp:1221-1267) and
//! `visit(OptionalMemberExpressionNode *, Node *)` (cpp:1269-1309), fused
//! into one [`SemanticResolver::visit_member_like_expression`] — the two C++
//! overloads are near-duplicates and that function documents the two places
//! they genuinely differ. They belong here (rather than in `classes.rs` with
//! the rest of the private-name work) because they are ordinary expression
//! visits, and because [`CODE_GENERATION_SETTINGS_TEST262`], which gates
//! their whole restriction block, is already this module's constant. Before
//! S2 T5 both kinds were served by `visit_node`'s override-free generic arm,
//! which was exactly right while no `_property` could be a `PrivateName`.
//!
//! ## REGEX-ENGINE DEFERRED
//!
//! `visit(RegExpLiteralNode *)` exists for exactly two side effects, and
//! neither of them is resolution:
//!
//! ```text
//! if (auto compiled = CompiledRegExp::tryCompile(
//!         regexp->_pattern->str(), regexp->_flags->str(), &regexpError)) {
//!   astContext_.addCompiledRegExp(
//!       regexp->_pattern, regexp->_flags, std::move(*compiled));
//! } else {
//!   sm_.error(
//!       regexp->getSourceRange(),
//!       "Invalid regular expression: " + Twine(regexpError));
//! }
//! ```
//!
//! `CompiledRegExp::tryCompile` runs Hermes's own regex engine
//! (`lib/Regex/`, plus `RegexSerialization.cpp`) over the literal: on
//! success the bytecode it produced is cached on the `Context` for BCGen to
//! pick up later, on failure its parse error becomes a sema diagnostic. That
//! engine is a separate component this port does not have, so
//! [`try_compile_regexp`] stands in for it and accepts everything, and the
//! success branch has no cache to write to (this port's `Context` has no
//! `addCompiledRegExp`; nothing in sema or `-dump-sema` ever reads it back).
//!
//! Where the boundary sits, precisely:
//!
//! - A **valid** regex is unaffected. Verified against the oracle: a
//!   `RegExpLiteral` contributes one bare `RegExpLiteral` line to
//!   `-dump-sema` and nothing to the `SemContext`, no matter what it matches
//!   — so `regexp-literals.js` in the corpus is a real byte-for-byte test of
//!   this visit, not a placeholder.
//! - An **invalid** regex is the one thing this cannot reproduce: hermesc
//!   reports `Invalid regular expression: <engine error>` over the literal's
//!   range and exits 2 (verified: `/a(/` → "Parenthesized expression not
//!   closed"). Matching it requires the engine's exact error strings, i.e.
//!   the engine. Such a file is therefore recorded in the corpus MANIFEST as
//!   deferred-to-the-regex-component instead of being silently skipped, and
//!   the `Err` arm of the match is a **panic** rather than the ported
//!   `sm_.error` call: the deferral can only ever change dump bytes through
//!   a rejected pattern, so a rejection must be loud the day a real
//!   validator replaces the stub.
//!
//! ## The fold loop: `list[i+1]->_left` becomes a rebuild
//!
//! C++'s binary visit linearizes a `+`/`-` chain into
//! `list = [(a+b), (list[0]+c), (list[1]+d), ...]` (innermost first, see
//! `linearize::linearize_left`) and then folds it bottom-up:
//!
//! ```text
//! for (size_t i = 0, e = list.size(); i < e; ++i) {
//!   ESTree::Node **foldResult = i + 1 < e ? &list[i + 1]->_left : ppNode;
//!   if (!astFoldBinaryExpression(kw_, list[i], foldResult))
//!     break;
//! }
//! ```
//!
//! Two mechanisms are in play there, and only one of them survives the port
//! unchanged:
//!
//! 1. **Where the fold's product goes.** `foldResult` is the *storage
//!    location* the folded literal is written into: the enclosing link's
//!    `_left` for every link but the last, and the visit's own `ppNode`
//!    (i.e. the parent's child field) for the outermost one. This port
//!    keeps that discriminator verbatim — `node_of(i)` below is
//!    `i + 1 < e ? list[i+1].left() : node`, the exact same ternary read as
//!    a *node* rather than as a slot — but the write becomes a value
//!    threaded through the loop in `replacement`, because chain nodes here
//!    are immutable in their structural fields.
//! 2. **Why the loop `break`s.** Folding link `i` mutates link `i+1` *in
//!    place*, so every fold above it operates on the already-updated node.
//!    Once a fold fails there is nothing left to do: the remaining links are
//!    already correct, in the AST, and pointed at by their parents.
//!
//! Point 2 is what does NOT carry over. Here, folding link `i` produces a
//! *new* literal that nothing points at yet; making it part of the tree
//! requires REBUILDING link `i+1` around it (and, if that rebuild happens,
//! rebuilding everything above it in turn). So the loop below cannot stop at
//! the failed fold — it must run to the end of the chain, doing rebuild-only
//! work from that point on, and return the outermost node as `Changed`.
//! Stopping the way C++ does would silently discard every fold that already
//! succeeded. The `folding` flag is therefore the port of `break`: it
//! disables *folding* for the rest of the loop, not the loop itself. (This
//! is the sharp edge called out in `resolver/mod.rs`'s module doc.)
//!
//! What the flag preserves exactly is the C++'s left-to-right, bottom-up
//! folding *order*, including its one counter-intuitive consequence:
//! `x + 1 + 2` folds nothing. `list[0]` is `x + 1`, which does not fold, so
//! `folding` goes false and link 1 (`(x+1) + 2`) is never even attempted —
//! and rightly so, since `1` and `2` are not operands of the same link. The
//! port must reproduce that, not "fold whatever it can"; see
//! `tests/resolver.rs`'s `binary_chain_stops_folding_at_the_first_failure`.
//!
//! Per link, the loop builds the link's *current* value with the generated
//! builder — left replaced by `replacement` (the product of the link below:
//! a fold, a rebuild, or nothing), right replaced by whatever visiting that
//! link's `_right` returned — and folds THAT, never the original node. This
//! is what makes a fold at link `i+1` operate on the rebuilt link `i+1`,
//! matching C++, where the mutation is what the next iteration reads. When
//! the builder reports no change, the link's own node (`node_of(i)`) is used
//! instead, so an untouched chain allocates nothing and comes back
//! `Unchanged` — pointer-identical, like C++'s untouched AST.
//!
//! `visit(AssignmentExpressionNode *)` has the same shape without the fold:
//! `linearizeRight` gives the spine outermost-first, each link's `_left` is
//! visited and validated, the innermost `_right` is visited last, and the
//! spine is then rebuilt innermost-first by
//! [`rebuild_assignment_chain`] — the mirror image of the binary loop's
//! direction, because `linearizeRight` collects in the opposite order.
//!
//! ## Visit order and recursion-depth accounting
//!
//! C++ reaches every child of a linearized chain through `visitESTreeNode`,
//! which is `RecursiveVisitorDispatch::visit` — the function that brackets
//! the kind dispatch with `incRecursionDepth`/`decRecursionDepth`
//! (RecursiveVisitor.h:197-232). The *chain nodes themselves* (`list[0]` ..
//! `list[n-2]`) are never handed to it: the visit walks them iteratively,
//! which is the entire point of linearizing. So the depth an `n`-link chain
//! consumes is 1 (the dispatch of the outermost node, `list[n-1]`, which is
//! what got us into this visit) plus exactly one bracket per *child* visit —
//! not `n`.
//!
//! This port reproduces both properties structurally: the only entry into
//! the resolver is [`SemanticResolver::call`], which carries the same
//! brackets, and this file calls it once per child, in C++'s order
//! (`list[0]._left`, then every `list[i]._right` ascending for binary; every
//! `list[i]._left` descending then `list.back()._right` for assignment). The
//! interior chain nodes are visited by neither — they are only read,
//! rebuilt and folded. `visit_children_mut`, used by the non-linearized
//! paths, routes each child through `NodeChild::visit_child_mut`, which
//! itself calls `call` (`node_child.rs`) — so those paths get the same
//! one-bracket-per-child accounting as C++'s `visitESTreeChildren`.
//!
//! The `parent` this port hands each child visit is likewise the C++ one:
//! `Path::new(node_of(i), field)` is the `e` in
//! `visitESTreeNode(*this, e->_right, e)`, i.e. the ORIGINAL link node, not
//! the rebuilt one. That matters because `visit(IdentifierNode *, Node *)`
//! makes decisions on the parent's kind, and the rebuild has not happened
//! yet at the time the child is visited — exactly as in C++, where the
//! rebuild never happens at all.

use ast::context::GCLock;
use ast::node::{builder, AssignmentExpression, Node, NodeField};
use ast::visitor::{Path, TransformResult, VisitorMut};

use crate::ast_eval::{
    ast_fold_binary_expression, ast_fold_unary_expression,
};
use crate::linearize::{
    linearize_left, linearize_right, OperatorExpr, MAX_NESTED_ASSIGNMENTS,
    MAX_NESTED_BINARY,
};
use crate::sem_context::{Atom, Constness, DeclKind, DeclSpecial};

use super::declarations::atom_str;
use super::functions::is_generator;
use super::SemanticResolver;

/// Port of `astContext_.getCodeGenerationSettings().test262`
/// (`include/hermes/AST/Context.h`), read by `visit(UnaryExpressionNode *)`
/// (SemanticResolver.cpp:485) and `isLValue` (cpp:2735).
///
/// `CodeGenerationSettings` is a compiler-driver knob (`hermesc -test262`),
/// not something sema computes, and this port has no driver flag that could
/// set it — so both uses test this documented constant instead, following
/// the `DEBUG_INFO_SETTING_ALL` precedent in `resolver/mod.rs`. The `if`
/// statements keep the exact shape of the C++ so that porting the real
/// setting later is a one-line change.
///
/// `false` is also hermesc's default, which is what the differential corpus
/// compares against.
const CODE_GENERATION_SETTINGS_TEST262: bool = false;

/// Rebuild an `=` chain that `linearize_right` produced, innermost link
/// first, threading each rebuilt link into the enclosing link's `_right`.
///
/// This is the assignment counterpart of the binary fold loop's rebuild (see
/// the module doc): C++ writes replacement children straight into
/// `e->_left` / `list.back()->_right` and is done, while here a replaced
/// child forces its link — and therefore every link outside it — to be
/// rebuilt.
///
/// \param list the spine, outermost first (`linearize_right`'s order).
/// \param left_repl per-link replacement for `_left`, `None` when the
///   child's visit returned `Unchanged`. Must have `list.len()` entries.
/// \param right_repl replacement for the INNERMOST link's `_right` (the only
///   `_right` in the chain that isn't another link).
/// \return `Changed` with the new outermost node if anything was replaced,
///   `Unchanged` otherwise (in which case nothing was allocated).
fn rebuild_assignment_chain<'gc>(
    gc: &'gc GCLock,
    list: &[&'gc AssignmentExpression<'gc>],
    left_repl: &[Option<&'gc Node<'gc>>],
    right_repl: Option<&'gc Node<'gc>>,
) -> TransformResult<&'gc Node<'gc>> {
    debug_assert_eq!(list.len(), left_repl.len());
    let mut child = right_repl;
    for i in (0..list.len()).rev() {
        let mut b = builder::AssignmentExpression::from_node(list[i]);
        if let Some(v) = left_repl[i] {
            b.left(v);
        }
        if let Some(v) = child {
            b.right(v);
        }
        child = match b.build(gc) {
            TransformResult::Changed(v) => Some(v),
            TransformResult::Unchanged => None,
            other => unreachable!("a builder never yields {other:?}"),
        };
    }
    match child {
        Some(v) => TransformResult::Changed(v),
        None => TransformResult::Unchanged,
    }
}

/// Reduce a child visit's result to "the replacement, if any".
pub(super) fn replacement_of<'gc>(
    result: TransformResult<&'gc Node<'gc>>,
) -> Option<&'gc Node<'gc>> {
    match result {
        TransformResult::Changed(v) => Some(v),
        TransformResult::Unchanged => None,
        // `Removed`/`Expanded` have no meaning for a required single child
        // (see `resolver/mod.rs`'s module doc: a hand-driven visit must
        // decide for itself, and no resolver visit produces one). C++ can't
        // express them here at all — `visitESTreeNode` takes a `Node *&`.
        other => unreachable!(
            "the resolver never removes or expands a child: {other:?}"
        ),
    }
}

impl SemanticResolver<'_, '_, '_, '_> {
    // ---- visit(BinaryExpressionNode *, Node **) ------------------------

    /// Port of `SemanticResolver::visit(ESTree::BinaryExpressionNode *node,
    /// ESTree::Node **ppNode)` (SemanticResolver.cpp:405-436). See the
    /// module doc for the fold-loop mapping.
    pub(super) fn visit_binary_expression<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let be = node
            .as_binary_expression()
            .expect("visit_binary_expression: not a BinaryExpression");

        // Handle nested +/- non-recursively.
        let ops = [self.kw().ident_plus, self.kw().ident_minus];
        if be.operator.get() == ops[0] || be.operator.get() == ops[1] {
            let list = linearize_left(be, &ops);
            if list.len() > MAX_NESTED_BINARY as usize {
                self.recursion_depth_exceeded(node);
                return TransformResult::Unchanged;
            }
            // The `&list[i + 1]->_left : ppNode` discriminator of the fold
            // loop below, read as a node instead of as a storage slot: the
            // node at `list[i]`, which is the enclosing link's `_left` for
            // every link but the outermost, and `node` itself for that one.
            let node_of = |i: usize| -> &'gc Node<'gc> {
                if i + 1 < list.len() {
                    list[i + 1].left()
                } else {
                    node
                }
            };

            // visitESTreeNode(*this, list[0]->_left, list[0]);
            let left0 = replacement_of(self.call(
                gc,
                list[0].left(),
                Some(Path::new(node_of(0), NodeField::left)),
            ));
            // for (auto *e : list) visitESTreeNode(*this, e->_right, e);
            let mut right_repl: Vec<Option<&'gc Node<'gc>>> =
                Vec::with_capacity(list.len());
            for (i, e) in list.iter().enumerate() {
                right_repl.push(replacement_of(self.call(
                    gc,
                    e.right(),
                    Some(Path::new(node_of(i), NodeField::right)),
                )));
            }

            // If compiling, fold all expressions bottom up (left to right).
            //
            // `replacement` is C++'s `*foldResult` in flight: what the link
            // below produced, and therefore what this link's `_left` must
            // become. `folding` is C++'s `break` — see the module doc for
            // why the loop itself must keep running past it.
            let mut replacement = left0;
            let mut folding = self.compile();
            for i in 0..list.len() {
                let mut b = builder::BinaryExpression::from_node(list[i]);
                if let Some(v) = replacement {
                    b.left(v);
                }
                if let Some(v) = right_repl[i] {
                    b.right(v);
                }
                let (cur, cur_changed) = match b.build(gc) {
                    TransformResult::Changed(v) => (v, true),
                    TransformResult::Unchanged => (node_of(i), false),
                    other => unreachable!("a builder never yields {other:?}"),
                };
                if folding {
                    let cur_be = cur.as_binary_expression().expect(
                        "a rebuilt BinaryExpression is a BinaryExpression",
                    );
                    if let Some(folded) =
                        ast_fold_binary_expression(gc, self.kw(), cur_be)
                    {
                        replacement = Some(folded);
                        continue;
                    }
                    // Attempt to fold the expression. If it fails, stop
                    // folding, since all subsequent expressions depend on
                    // the result of this one.
                    folding = false;
                }
                replacement = if cur_changed { Some(cur) } else { None };
            }
            return match replacement {
                Some(v) => TransformResult::Changed(v),
                None => TransformResult::Unchanged,
            };
        }

        let result = node.visit_children_mut(gc, self);
        if !self.compile() {
            return result;
        }
        let cur = match result {
            TransformResult::Changed(v) => v,
            TransformResult::Unchanged => node,
            ref other => unreachable!(
                "the resolver never removes or expands a child: {other:?}"
            ),
        };
        let cur_be = cur
            .as_binary_expression()
            .expect("a rebuilt BinaryExpression is a BinaryExpression");
        match ast_fold_binary_expression(gc, self.kw(), cur_be) {
            Some(folded) => TransformResult::Changed(folded),
            // The fold declined, so the visit's result is whatever visiting
            // the children produced. (Reading `result` after the `match`
            // above is fine: binding `&Node` — a `Copy` type — out of a
            // `TransformResult` copies rather than moves it.)
            None => result,
        }
    }

    // ---- visit(AssignmentExpressionNode *) -----------------------------

    /// Port of `SemanticResolver::visit(ESTree::AssignmentExpressionNode
    /// *assignment)` (SemanticResolver.cpp:438-462).
    ///
    /// The two `if (LLVM_UNLIKELY(recursionDepth_ == 0)) return;` early
    /// exits return the spine rebuilt from the replacements gathered *so
    /// far*, with the untouched links left as they are — which is precisely
    /// the tree C++'s `return` leaves behind, since it has already written
    /// those same replacements into the nodes it walked past.
    ///
    /// Neither exit is reachable today, in either implementation: the depth
    /// budget can only be spent by the `_left` visit above, and a `_left`
    /// deep enough to spend the whole budget never gets past the *parser* —
    /// on either side. Both parsers trip their own, much lower nesting limit
    /// first (`MAX_RECURSION_DEPTH`, ~127 usable levels in the builds that
    /// take the `HERMES_LIMIT_STACK_DEPTH` branch), and the Rust port
    /// enforces it at exactly the same productions and the same boundary as
    /// C++ — verified site-by-site and by ladder in the 2026-08-04
    /// recursion-parity pass. So these ports are what keep the visit honest
    /// if a hand-built AST ever does reach here.
    pub(super) fn visit_assignment_expression<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let assignment = node
            .as_assignment_expression()
            .expect("visit_assignment_expression: not an AssignmentExpression");

        // Handle nested "=" non-recursively.
        let ops = [self.kw().ident_assign];
        if assignment.operator.get() == ops[0] {
            let list = linearize_right(assignment, &ops);
            if list.len() > MAX_NESTED_ASSIGNMENTS as usize {
                self.recursion_depth_exceeded(node);
                return TransformResult::Unchanged;
            }
            // `linearize_right` collects outermost first, and each link is
            // the enclosing link's `_right` — so `list[i]`'s own node is
            // `list[i-1]._right`, and `node` for the outermost.
            let node_of = |i: usize| -> &'gc Node<'gc> {
                if i == 0 {
                    node
                } else {
                    list[i - 1].right()
                }
            };

            let mut left_repl: Vec<Option<&'gc Node<'gc>>> =
                vec![None; list.len()];
            for i in 0..list.len() {
                let e = list[i];
                let left_node = match self.call(
                    gc,
                    e.left(),
                    Some(Path::new(node_of(i), NodeField::left)),
                ) {
                    TransformResult::Changed(v) => {
                        left_repl[i] = Some(v);
                        v
                    }
                    TransformResult::Unchanged => e.left(),
                    other => unreachable!(
            "the resolver never removes or expands a child: {other:?}"
        ),
                };
                if self.recursion_depth == 0 {
                    return rebuild_assignment_chain(
                        gc, &list, &left_repl, None,
                    );
                }
                self.validate_assignment_target(left_node);
            }
            let last = list.len() - 1;
            let right_repl = replacement_of(self.call(
                gc,
                list[last].right(),
                Some(Path::new(node_of(last), NodeField::right)),
            ));
            return rebuild_assignment_chain(
                gc, &list, &left_repl, right_repl,
            );
        }

        let mut b = builder::AssignmentExpression::from_node(assignment);
        let left_node = match self.call(
            gc,
            assignment.left,
            Some(Path::new(node, NodeField::left)),
        ) {
            TransformResult::Changed(v) => {
                b.left(v);
                v
            }
            TransformResult::Unchanged => assignment.left,
            other => unreachable!(
            "the resolver never removes or expands a child: {other:?}"
        ),
        };
        if self.recursion_depth == 0 {
            return b.build(gc);
        }
        self.validate_assignment_target(left_node);
        if let TransformResult::Changed(v) = self.call(
            gc,
            assignment.right,
            Some(Path::new(node, NodeField::right)),
        ) {
            b.right(v);
        }
        b.build(gc)
    }

    // ---- visit(UpdateExpressionNode *) ---------------------------------

    /// Port of `SemanticResolver::visit(ESTree::UpdateExpressionNode *node)`
    /// (SemanticResolver.cpp:464-473).
    pub(super) fn visit_update_expression<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let result = node.visit_children_mut(gc, self);
        if self.recursion_depth == 0 {
            return result;
        }
        // C++ reads `node->_argument` after the children were visited, i.e.
        // through whatever `visitESTreeChildren` may have replaced it with;
        // here that value lives on the rebuilt node instead.
        let cur = match result {
            TransformResult::Changed(v) => v,
            TransformResult::Unchanged => node,
            ref other => unreachable!(
                "the resolver never removes or expands a child: {other:?}"
            ),
        };
        let argument = cur
            .as_update_expression()
            .expect("a rebuilt UpdateExpression is an UpdateExpression")
            .argument;
        if !self.is_lvalue(argument) {
            self.sm.error_range(
                argument.range(),
                "invalid operand in update operation",
            );
        }
        result
    }

    // ---- visit(UnaryExpressionNode *, Node **) -------------------------

    /// Port of `SemanticResolver::visit(ESTree::UnaryExpressionNode *node,
    /// ESTree::Node **ppNode)` (SemanticResolver.cpp:475-500).
    pub(super) fn visit_unary_expression<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let ue = node
            .as_unary_expression()
            .expect("visit_unary_expression: not a UnaryExpression");

        // Check for unqualified delete in strict mode.
        if ue.operator.get() == self.kw().ident_delete {
            if self.sem_ctx.function(self.cur_function_info()).strict
                && matches!(ue.argument, Node::Identifier(_))
            {
                self.sm.error_range(
                    node.range(),
                    "'delete' of a variable is not allowed in strict mode",
                );
            }
            // Unless we are running under compliance tests, report an error
            // on `delete super.x`.
            if !CODE_GENERATION_SETTINGS_TEST262 {
                if let Node::MemberExpression(mem) = ue.argument {
                    if matches!(mem.object, Node::Super(_)) {
                        self.sm.error_range(
                            node.range(),
                            "'delete' of super property is not allowed",
                        );
                    }
                }
            }
        }
        let result = node.visit_children_mut(gc, self);
        if !self.compile() {
            return result;
        }
        let cur = match result {
            TransformResult::Changed(v) => v,
            TransformResult::Unchanged => node,
            ref other => unreachable!(
                "the resolver never removes or expands a child: {other:?}"
            ),
        };
        let cur_ue = cur
            .as_unary_expression()
            .expect("a rebuilt UnaryExpression is a UnaryExpression");
        match ast_fold_unary_expression(gc, self.kw(), cur_ue) {
            Some(folded) => TransformResult::Changed(folded),
            None => result,
        }
    }

    // ---- validateAssignmentTarget / isLValue ---------------------------

    /// Port of `SemanticResolver::validateAssignmentTarget`
    /// (SemanticResolver.cpp:2693-2725).
    ///
    /// C++'s `return validateAssignmentTarget(...)` on a `void` function is
    /// a tail call written for brevity; here each is a call followed by a
    /// `return`, which is the same thing.
    pub(super) fn validate_assignment_target(&mut self, node: &Node) {
        if matches!(node, Node::Empty(_)) {
            return;
        }

        if let Node::AssignmentPattern(assign) = node {
            self.validate_assignment_target(assign.left);
            return;
        }

        if let Node::Property(prop) = node {
            self.validate_assignment_target(prop.value);
            return;
        }

        if let Node::ArrayPattern(arr) = node {
            for elem in arr.elements.iter() {
                self.validate_assignment_target(elem);
            }
            return;
        }

        if let Node::ObjectPattern(obj) = node {
            for prop_node in obj.properties.iter() {
                self.validate_assignment_target(prop_node);
            }
            return;
        }

        if let Node::RestElement(rest) = node {
            self.validate_assignment_target(rest.argument);
            return;
        }

        if !self.is_lvalue(node) {
            self.sm.error_range(
                node.range(),
                "invalid assignment left-hand side",
            );
        }
    }

    /// Port of `SemanticResolver::isLValue` (SemanticResolver.cpp:
    /// 2713-2757).
    ///
    /// The C++ `assert(decl && "Identifier must be resolved")` is an
    /// `expect` here: both callers run after the identifier's own visit, so
    /// an unresolved identifier is a resolver bug, not an input error.
    ///
    /// `OptionalMemberExpression` is deliberately NOT an lvalue — C++ tests
    /// `isa<MemberExpressionNode>` only, and `a?.b = 1` is indeed a syntax
    /// error rather than a valid target.
    pub(super) fn is_lvalue(&self, node: &Node) -> bool {
        if matches!(node, Node::MemberExpression(_)) {
            return true;
        }

        if let Node::Identifier(id) = node {
            let decl = self
                .sem_ctx
                .get_expression_decl(id)
                .expect("Identifier must be resolved");

            // Unless we are running under compliance tests, report an error
            // on reassignment to const.
            if !CODE_GENERATION_SETTINGS_TEST262 {
                let constness = self.sem_ctx.decl(decl).kind.constness();
                if constness == Constness::Always
                    || (self
                        .sem_ctx
                        .function(self.cur_function_info())
                        .strict
                        && constness == Constness::StrictModeOnly)
                {
                    return false;
                }
            }

            // In strict mode, assigning to the identifier "eval" or
            // "arguments" is invalid, regardless of what they are bound to
            // in surrounding scopes. This is invalid:
            //     let eval;
            //     function foo() {
            //       "use strict";
            //       eval = 0; // ERROR!
            //     }
            if self.sem_ctx.function(self.cur_function_info()).strict {
                if id.name.get() == self.kw().ident_arguments
                    || id.name.get() == self.kw().ident_eval
                {
                    return false;
                }
            } else {
                // IMPORTANT: this is not spec compliant!
                // In loose mode it should be possible to assign to
                // "arguments". But that is a corner case that is difficult
                // to handle, so for now we are prohibiting it.
                if self.sem_ctx.decl(decl).special == DeclSpecial::Arguments {
                    return false;
                }
            }

            return true;
        }

        false
    }

    // ==== S2 T2: yield / await / spread / meta / the Cover nodes =========

    // ---- visit(YieldExpressionNode *) ----------------------------------

    /// Port of `SemanticResolver::visit(ESTree::YieldExpressionNode *node)`
    /// (SemanticResolver.cpp:1490-1506).
    pub(super) fn visit_yield_expression<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        // functionContext()->node && !ESTree::isGenerator(
        //     functionContext()->node)
        //
        // The `node` of a `FunctionContext` is null only for the contexts
        // created by the `ExistingGlobalScopeTag`/`FunctionInfo *`
        // constructors (S5), hence the `Option`.
        let in_non_generator = match &self.function_context().node {
            Some(n) => !is_generator(n.node(gc)),
            None => false,
        };
        if self.in_global_scope_context() || in_non_generator {
            self.sm.error_range(
                node.range(),
                "'yield' not in a generator function",
            );
        }

        if self.function_context().is_formal_params {
            // For generators functions (the only time YieldExpression is
            // parsed):
            // It is a Syntax Error if UniqueFormalParameters Contains
            // YieldExpression is true.
            self.sm.error_range(
                node.range(),
                "'yield' not allowed in a formal parameter",
            );
        }

        node.visit_children_mut(gc, self)
    }

    // ---- visit(AwaitExpressionNode *) ----------------------------------

    /// Port of `SemanticResolver::visit(ESTree::AwaitExpressionNode
    /// *awaitExpr)` (SemanticResolver.cpp:1508-1522). cpp:1509 is the ONLY
    /// read of `forbidAwaitExpression_` anywhere in the C++ resolver, which
    /// is why the flag has been write-only in this port since S1 T7 wired
    /// `visitFunctionLikeInFunctionContext`'s save/restore (cpp:1845-1846).
    /// Three more writers exist — `visit(ClassPrivatePropertyNode *)`
    /// (cpp:976), `visit(ClassPropertyNode *)` (cpp:1033) and
    /// `visit(StaticBlockNode *)` (cpp:1084), each forcing it to `true` — and
    /// they are S2 T4/T5's; those are also the only paths that
    /// can make the error below fire, since the parser will not build an
    /// `AwaitExpression` outside an async function otherwise (see
    /// `test/Parser/await-field-error.js`).
    pub(super) fn visit_await_expression<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        if self.forbid_await_expression {
            self.sm
                .error_range(node.range(), "'await' not in an async function");
        }

        if self.function_context().is_formal_params {
            // ES14.0 15.8.1
            // It is a Syntax Error if FormalParameters Contains
            // AwaitExpression is true.
            self.sm.error_range(
                node.range(),
                "'await' not allowed in a formal parameter",
            );
        }

        node.visit_children_mut(gc, self)
    }

    // ---- visit(SpreadElementNode *, Node *) ----------------------------

    /// Port of `SemanticResolver::visit(ESTree::SpreadElementNode *node,
    /// Node *parent)` (SemanticResolver.cpp:1469-1481).
    ///
    /// The `RecordExpressionProperties` arm is `#if HERMES_PARSE_FLOW` in
    /// C++; this port has a single, unconditional node set (the `ast` crate
    /// generates all 271 kinds from `ESTree.def` with no dialect `cfg`), so
    /// it is simply part of the whitelist here.
    ///
    /// **Not corpus-reachable.** No `JSParserImpl` path can build a
    /// `SpreadElementNode` whose parent is outside this whitelist: the three
    /// construction sites are `parseArrayLiteral`/`parseObjectProperties`
    /// (`JSParserImpl.cpp:2727, 2770` → `parseSpreadElement`, :2815-2827) and
    /// `parseArguments` (:3610, whose result is a `CallExpression`,
    /// `OptionalCallExpression` or `NewExpression`), and `...` anywhere else
    /// is either an `invalid expression` parse error or reinterpreted into a
    /// `RestElement`/`CoverRestElement`. So the error below is defensive in
    /// C++ too, and the corpus can only pin the accepting arms. Verified by
    /// probing hermesc with `import(...a)`, `switch (...a)`,
    /// `` tag`${...a}` `` and the destructuring reinterpret paths — all parse
    /// errors or `RestElement`s.
    pub(super) fn visit_spread_element<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
        path: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        // `parent` is `None` only for the root node, which a `SpreadElement`
        // never is; `matches!` on `None` therefore takes the error branch,
        // exactly like C++'s chain of failing `isa<>` tests would.
        if !matches!(
            path.map(|p| p.parent),
            Some(
                Node::ObjectExpression(_)
                    | Node::ArrayExpression(_)
                    | Node::CallExpression(_)
                    | Node::OptionalCallExpression(_)
                    | Node::NewExpression(_)
                    | Node::RecordExpressionProperties(_)
            )
        ) {
            self.sm
                .error_range(node.range(), "spread operator is not supported");
        }
        node.visit_children_mut(gc, self)
    }

    // ---- visit(MetaPropertyNode *) -------------------------------------

    /// Port of `SemanticResolver::visit(ESTree::MetaPropertyNode *node)`
    /// (SemanticResolver.cpp:837-872).
    ///
    /// Every path through the C++ function ends in a bare `return` or falls
    /// off the end — there is no `visitESTreeChildren` — so the two
    /// `IdentifierNode` children are deliberately NOT resolved, and this
    /// visit can only ever return `Unchanged`.
    pub(super) fn visit_meta_property<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let mp = node
            .as_meta_property()
            .expect("visit_meta_property: not a MetaProperty");
        let meta = meta_property_name(mp.meta);
        let property = meta_property_name(mp.property);

        if meta == self.kw().ident_new && property == self.kw().ident_target {
            if self.in_global_scope_context() {
                // ES9.0 15.1.1:
                // It is a Syntax Error if StatementList Contains NewTarget
                // unless the source code containing NewTarget is eval code
                // that is being processed by a direct eval.
                // Hermes does not support local eval, so we assume that this
                // is not inside a local eval call.
                self.sm.error_range(
                    node.range(),
                    "'new.target' not in a function",
                );
            }
            let sem_info = self.function_context().sem_info;
            if self.sem_ctx.nearest_non_arrow(sem_info)
                == self.sem_ctx.get_global_function()
            {
                self.sm.error_range(
                    node.range(),
                    "'new.target' not allowed in arrow function in global \
                     scope",
                );
            }
            return TransformResult::Unchanged;
        }

        // C++ compares `_name->str()` against the literal "import"/"meta"
        // rather than against `kw_`; identifier names are uniqued, so
        // comparing the interned atoms is the same test.
        if meta == self.kw().ident_import && property == self.kw().ident_meta {
            if self.compile() {
                self.sm.error_range(
                    node.range(),
                    "'import.meta' is currently unsupported",
                );
            }
            return TransformResult::Unchanged;
        }

        // Not corpus-reachable: `parseNewExpressionOrOptionalExpression` and
        // `parseOptionalExpressionExceptNew` only build a `MetaProperty` after
        // matching `new` `.` `target` / `import` `.` `meta` exactly, and
        // report `'target'/'meta' expected in member expression` otherwise.
        // Ported for fidelity.
        self.sm.error_range(
            node.range(),
            format!(
                "invalid meta property {}.{}",
                atom_str(gc, meta),
                atom_str(gc, property)
            ),
        );
        TransformResult::Unchanged
    }

    // ---- the Cover visits ----------------------------------------------

    /// Port of the four (five, with the Flow one) `visit(Cover*Node *)`
    /// overloads (SemanticResolver.cpp:1572-1591). The parser produces a
    /// `Cover*` node when a construct is only legal as part of something it
    /// turned out not to be part of — most often arrow parameters — and
    /// leaves rejecting it to sema, which is this function.
    ///
    /// None of the five C++ overloads calls `visitESTreeChildren`, so each
    /// reports its error and stops: no child of a cover node is ever
    /// resolved, and the visit is always `Unchanged`.
    ///
    /// `CoverTypedIdentifier` is `#if HERMES_PARSE_FLOW` in C++; see
    /// `visit_spread_element`'s note on why this port has no such gate.
    pub(super) fn visit_cover_node<'gc>(
        &mut self,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        match node {
            Node::CoverEmptyArgs(_) => {
                self.sm.error_range(
                    node.range(),
                    "invalid empty parentheses '( )'",
                );
            }
            Node::CoverTrailingComma(_) => {
                self.sm
                    .error_range(node.range(), "expression expected after ','");
            }
            // `sm_.error(node->getStartLoc(), ...)`: a location, not a range
            // — the only one of the five that points rather than underlines.
            Node::CoverInitializer(_) => {
                self.sm.error(
                    node.range().start,
                    "':' expected in property initialization",
                );
            }
            Node::CoverRestElement(_) => {
                self.sm.error_range(
                    node.range(),
                    "'...' not allowed in this context",
                );
            }
            Node::CoverTypedIdentifier(_) => {
                self.sm.error_range(
                    node.range(),
                    "typecast not allowed in this context",
                );
            }
            _ => unreachable!(
                "visit_cover_node on a {}",
                node.node_type_str()
            ),
        }
        TransformResult::Unchanged
    }

    // ---- visit(TypeCastExpressionNode *) / visit(AsExpressionNode *) ---

    /// Port of `SemanticResolver::visit(ESTree::TypeCastExpressionNode
    /// *node)` (SemanticResolver.cpp:1605-1608, `#if HERMES_PARSE_FLOW`; no
    /// dialect gate here for the same reason `visit_spread_element`'s note
    /// gives for `CoverTypedIdentifier`).
    ///
    /// ```text
    /// void SemanticResolver::visit(TypeCastExpressionNode *node) {
    ///   // Visit the expression, but not the type annotation.
    ///   visitESTreeNode(*this, node->_expression, node);
    /// }
    /// ```
    ///
    /// `_expression` is the only child visited — deliberately, same as
    /// `ObjectPattern`/`ArrayPattern`'s `_typeAnnotation` skip
    /// (`resolver/mod.rs`'s module doc), so the type annotation's own
    /// `Identifier`s (e.g. `number` in a `GenericTypeAnnotation`) never get
    /// `[D:E:...]` resolution. `(x: number);` under untyped `-parse-flow`
    /// reaches this visit directly — no `-typed` is needed, since the C++
    /// site is unconditional on `typed_`.
    pub(super) fn visit_type_cast_expression<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let n = node.as_type_cast_expression().expect(
            "visit_type_cast_expression: not a TypeCastExpression",
        );
        let mut b = builder::TypeCastExpression::from_node(n);
        if let TransformResult::Changed(v) = self.call(
            gc,
            n.expression,
            Some(Path::new(node, NodeField::expression)),
        ) {
            b.expression(v);
        }
        b.build(gc)
    }

    /// Port of `SemanticResolver::visit(ESTree::AsExpressionNode *node)`
    /// (SemanticResolver.cpp:1610-1613, `#if HERMES_PARSE_FLOW`) — same
    /// shape and same "visit the expression, not the type annotation"
    /// comment as [`Self::visit_type_cast_expression`] above. `x as number;`
    /// under untyped `-parse-flow` (no `-typed`) reaches this visit: the
    /// parser accepts Flow's `as` operator whenever `getParseFlow()` is set
    /// (`JSParserImpl.cpp:4329-4350`), independent of `typed_`.
    ///
    /// `AsConstExpressionNode` (`x as const`, JSParserImpl.cpp:4331-4344) is
    /// a sibling node the parser can also produce here, but it has no
    /// `visit()` override in C++ (SemanticResolver.h's inventory), so it
    /// falls to the ordinary `visitESTreeChildren` catch-all there — i.e.
    /// this port's override-free generic arm, exactly like `NewExpression`/
    /// `ConditionalExpression`. Not ported by this task: no corpus file
    /// reaches it yet (see `resolver/mod.rs`'s generic-arm whitelist for
    /// where to add it once one does).
    pub(super) fn visit_as_expression<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let n = node
            .as_as_expression()
            .expect("visit_as_expression: not an AsExpression");
        let mut b = builder::AsExpression::from_node(n);
        if let TransformResult::Changed(v) = self.call(
            gc,
            n.expression,
            Some(Path::new(node, NodeField::expression)),
        ) {
            b.expression(v);
        }
        b.build(gc)
    }

    // ---- visit(RegExpLiteralNode *) -------------------------------------

    /// Port of `SemanticResolver::visit(RegExpLiteralNode *regexp)`
    /// (SemanticResolver.cpp:821-835). See the module doc's **REGEX-ENGINE
    /// DEFERRED** section for the one thing this cannot do yet.
    pub(super) fn visit_regexp_literal<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let regexp = node
            .as_reg_exp_literal()
            .expect("visit_regexp_literal: not a RegExpLiteral");

        // llvh::StringRef regexpError;
        if self.compile() {
            match try_compile_regexp(regexp.pattern.get(), regexp.flags.get()) {
                Ok(()) => {
                    // astContext_.addCompiledRegExp(
                    //     regexp->_pattern, regexp->_flags,
                    //     std::move(*compiled));
                    //
                    // `Context::addCompiledRegExp` is a *cache* the bytecode
                    // generator later drains (`Context::getCompiledRegExp`);
                    // nothing sema itself, or `-dump-sema`, reads it. There
                    // is no such table on this port's `Context` yet, so the
                    // successful branch has nothing to record — see the
                    // module doc.
                }
                Err(regexp_error) => panic!(
                    "sema: REGEX-ENGINE DEFERRED — the stub validator \
                     rejected /{}/{}: {regexp_error}. Replacing it with a \
                     real validator means porting \
                     SemanticResolver.cpp:829-832's diagnostic at the same \
                     time: sm_.error(regexp->getSourceRange(), \"Invalid \
                     regular expression: \" + Twine(regexpError))",
                    atom_str(gc, regexp.pattern.get()),
                    atom_str(gc, regexp.flags.get()),
                ),
            }
        }
        // visitESTreeChildren(*this, regexp): a `RegExpLiteral` has no node
        // children at all (`_pattern`/`_flags` are `NodeLabel`s), so this is
        // unconditionally `Unchanged`.
        node.visit_children_mut(gc, self)
    }

    // ---- visit(MemberExpressionNode *, Node *) ---------------------------
    // ---- visit(OptionalMemberExpressionNode *, Node *) -------------------

    /// Port of `SemanticResolver::visit(ESTree::MemberExpressionNode *node,
    /// ESTree::Node *parent)` (cpp:1221-1267) and
    /// `visit(ESTree::OptionalMemberExpressionNode *node, ESTree::Node
    /// *parent)` (cpp:1269-1309).
    ///
    /// The two C++ overloads are near-duplicates; the ONLY differences, both
    /// preserved below, are:
    ///
    /// - the `isa<SuperNode>(node->_object)` / "Cannot lookup private names
    ///   on super." check exists only on the non-optional overload
    ///   (cpp:1227-1230). It is not needed on the other one: the parser
    ///   rejects `super?.` outright (`'(', '[' or '.' expected after 'super'
    ///   keyword`), so an `OptionalMemberExpression` never has a `Super`
    ///   object — the same grammar fact that makes `visit(SuperNode *)`'s
    ///   `OptionalMemberExpression` arm dead (see `classes.rs`).
    /// - the `delete` diagnostic points at DIFFERENT nodes in the two
    ///   overloads: `node` for `MemberExpression` (cpp:1233) but `parent`
    ///   (i.e. the whole `delete ...` expression) for
    ///   `OptionalMemberExpression` (cpp:1276-1277). Verified against
    ///   hermesc: `delete o.#x` underlines `o.#x`, `delete o?.#x` underlines
    ///   `delete o?.#x`. Faithfully reproduced, not unified.
    ///
    /// For any `_property` that is not a `PrivateName` — every member
    /// expression the corpus could reach before this task — the whole body
    /// reduces to `visitESTreeChildren(*this, node)`, which is what let S1
    /// route both kinds through `visit_node`'s override-free generic arm.
    ///
    /// \param path the parent, as always (C++'s `parent`). `None` cannot
    ///   happen (a `Program` is always the root); every check below is
    ///   simply skipped for it, which is also what a null `parent` would do
    ///   in C++.
    pub(super) fn visit_member_like_expression<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
        path: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let (object, property, optional) = match node {
            Node::MemberExpression(m) => (m.object, m.property, false),
            Node::OptionalMemberExpression(m) => (m.object, m.property, true),
            _ => unreachable!(
                "visit_member_like_expression on a {}",
                node.node_type_str()
            ),
        };
        if let Node::PrivateName(name) = property {
            // The following conditions are forbidden by the grammar but it's
            // harder to enforce in the parser.
            if !optional && matches!(object, Node::Super(_)) {
                self.sm.error_range(
                    node.range(),
                    "Cannot lookup private names on super.",
                );
            }
            let delete_parent = path.filter(|p| {
                matches!(p.parent, Node::UnaryExpression(op)
                    if op.operator.get() == self.kw().ident_delete)
            });
            if let Some(p) = delete_parent {
                // See the doc comment: the two overloads deliberately report
                // this at different ranges.
                let range = if optional {
                    p.parent.range()
                } else {
                    node.range()
                };
                self.sm
                    .error_range(range, "Cannot `delete` with a private name.");
            }
            if !CODE_GENERATION_SETTINGS_TEST262 {
                let decl = self.resolve_private_name(gc, name.id);
                // There is no decl when a nonexistent private name is
                // referenced, in which case there's no further validation
                // that can be done here.
                if let Some(decl) = decl {
                    // `assign && assign->_left == node`: the `Path` already
                    // says which field of the parent `node` occupies, so the
                    // pointer comparison becomes a field test.
                    let assign_to_this = path.filter(|p| {
                        matches!(p.parent, Node::AssignmentExpression(_))
                            && p.field == NodeField::left
                    });
                    if let Some(p) = assign_to_this {
                        // Validate stores of a private name are using a name
                        // that is eligible for stores.
                        let kind = self.sem_ctx.decl(decl).kind;
                        if kind == DeclKind::PrivateGetter {
                            self.sm.error_range(
                                p.parent.range(),
                                "Cannot store to a private name that only \
                                 defines a getter.",
                            );
                        } else if kind == DeclKind::PrivateMethod {
                            self.sm.error_range(
                                p.parent.range(),
                                "Cannot store to a private name that defines \
                                 a method.",
                            );
                        }
                    } else {
                        // Validate loads of a private name are using a name
                        // that is eligible for loads.
                        if self.sem_ctx.decl(decl).kind
                            == DeclKind::PrivateSetter
                        {
                            self.sm.error_range(
                                node.range(),
                                "Cannot load from a private name that only \
                                 defines a setter.",
                            );
                        }
                    }
                }
            }
        }
        node.visit_children_mut(gc, self)
    }
}

/// Stand-in for `CompiledRegExp::tryCompile(pattern, flags, &error)`
/// (`include/hermes/Regex/RegexSerialization.h`), which compiles the literal
/// with Hermes's own regex engine purely to *validate* it.
///
/// **REGEX-ENGINE DEFERRED.** That engine (`lib/Regex/`, plus
/// `CompiledRegExp`'s serialization) is a whole separate component this port
/// does not have; reimplementing it here would be a second, unreviewed copy
/// of it. So this accepts every input, and the caller's `Err` arm is a loud
/// panic rather than a diagnostic: the ONLY way the deferral can change
/// `-dump-sema` bytes is a pattern C++ rejects and this accepts, and the
/// panic is what makes that visible the moment a real validator lands here.
///
/// See `expressions.rs`'s module doc for the corpus consequence.
fn try_compile_regexp(_pattern: Atom, _flags: Atom) -> Result<(), String> {
    Ok(())
}

/// The `_name` of a `MetaProperty`'s `_meta`/`_property`. Port of
/// `llvh::cast<IdentifierNode>(node->_meta)` + `->_name` (cpp:838-839, 841):
/// a failing `cast` in C++ is an explicit panic here.
fn meta_property_name(node: &Node) -> Atom {
    match node {
        Node::Identifier(id) => id.name.get(),
        _ => panic!(
            "MetaProperty child is a {}, not an Identifier",
            node.node_type_str()
        ),
    }
}

#[cfg(test)]
mod tests {
    use ast::context::Context;
    use ast::node::{Identifier, NumericLiteral, Program};
    use ast::node_child::{NodeList, NodeMetadata};
    use support::location::{SMLoc, SMRange};
    use support::manager::SourceErrorManager;
    use support::persistent_scoped_map::Scope;

    use super::*;
    use crate::keywords::Keywords;
    use crate::sem_context::{
        Binding, ConstructorKind, CustomDirectives, DeclKind, SemContext,
    };

    /// A fixture that stands up just enough resolver state for `isLValue`:
    /// one (optionally strict) function context whose scope holds a single
    /// declaration `name` of kind `kind` and specialness `special`, plus an
    /// `Identifier` node already resolved to it.
    ///
    /// \return the number of errors `validate_assignment_target` reported
    ///   for that identifier, and what `is_lvalue` said about it.
    fn check_identifier_target(
        name: &str,
        kind: DeclKind,
        special: DeclSpecial,
        strict: bool,
    ) -> (bool, usize) {
        let mut ctx = Context::new();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("lv.js", b"x");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let range = SMRange {
            start: loc,
            end: loc,
        };
        let gc = ctx.lock();
        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let atom = gc.atom_bytes(name);
        let ident_node = gc.alloc(Node::Identifier(Identifier::new(
            NodeMetadata::new(range),
            atom,
            None,
            false,
        )));
        let program = gc.alloc(Node::Program(Program::new(
            NodeMetadata::new(range),
            NodeList::from_iter(&gc, []),
        )));

        let binding_table = sem_ctx.binding_table_rc();
        let is_lv = {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                /* compile */ true,
            );
            let func_state = resolver.enter_function(
                &gc,
                program,
                None,
                strict,
                ConstructorKind::None,
                CustomDirectives::default(),
                /* install_as_global_context */ true,
            );
            let scope_state = resolver.enter_scope(None, true);
            let scope = resolver.cur_scope.expect("scope just entered");
            let decl = resolver
                .sem_ctx
                .new_decl_in_scope(atom, kind, scope, special);
            resolver
                .binding_table
                .try_emplace(atom, Binding::new(decl, None));
            let ident = ident_node
                .as_identifier()
                .expect("just built an Identifier");
            resolver.sem_ctx.set_expression_decl(
                ident_node.node_id(),
                ident,
                Some(decl),
            );

            let is_lv = resolver.is_lvalue(ident_node);
            resolver.validate_assignment_target(ident_node);
            resolver.exit_scope(scope_state);
            resolver.exit_function(func_state);
            is_lv
        };
        let errors = sm.error_count() as usize;
        (is_lv, errors)
    }

    /// A plain `let` is assignable and reports nothing.
    #[test]
    fn is_lvalue_accepts_a_mutable_binding() {
        let (is_lv, errors) = check_identifier_target(
            "x",
            DeclKind::Let,
            DeclSpecial::NotSpecial,
            false,
        );
        assert!(is_lv);
        assert_eq!(errors, 0);
    }

    /// `Decl::Constness::Always` (a `const`) is never assignable, in either
    /// mode, and `validateAssignmentTarget` turns that into the
    /// "invalid assignment left-hand side" error.
    #[test]
    fn is_lvalue_rejects_const_in_both_modes() {
        for strict in [false, true] {
            let (is_lv, errors) = check_identifier_target(
                "c",
                DeclKind::Const,
                DeclSpecial::NotSpecial,
                strict,
            );
            assert!(!is_lv, "const must not be an lvalue (strict={strict})");
            assert_eq!(errors, 1);
        }
    }

    /// `Decl::Constness::StrictModeOnly` (a function-expression name) is
    /// assignable in loose mode and rejected in strict mode.
    #[test]
    fn is_lvalue_rejects_strict_mode_only_constness_only_when_strict() {
        let (loose, loose_errs) = check_identifier_target(
            "f",
            DeclKind::FunctionExprName,
            DeclSpecial::NotSpecial,
            false,
        );
        assert!(loose, "a function expression name is writable in loose mode");
        assert_eq!(loose_errs, 0);

        let (strict, strict_errs) = check_identifier_target(
            "f",
            DeclKind::FunctionExprName,
            DeclSpecial::NotSpecial,
            true,
        );
        assert!(!strict);
        assert_eq!(strict_errs, 1);
    }

    /// In strict mode the NAMES `eval` and `arguments` are rejected outright
    /// — independently of what they are bound to (here: an ordinary,
    /// perfectly writable `let`).
    #[test]
    fn is_lvalue_rejects_eval_and_arguments_by_name_in_strict_mode() {
        for name in ["eval", "arguments"] {
            let (is_lv, errors) = check_identifier_target(
                name,
                DeclKind::Let,
                DeclSpecial::NotSpecial,
                true,
            );
            assert!(!is_lv, "strict mode must reject `{name}` as a target");
            assert_eq!(errors, 1);
            // The same binding is assignable in loose mode: the strict check
            // is the only thing rejecting it.
            let (loose, loose_errs) = check_identifier_target(
                name,
                DeclKind::Let,
                DeclSpecial::NotSpecial,
                false,
            );
            assert!(loose, "loose mode must accept `{name}` bound to a let");
            assert_eq!(loose_errs, 0);
        }
    }

    /// The loose-mode `arguments` quirk (cpp:2759-2765): the *special*
    /// `arguments` decl is rejected even in loose mode, where the spec would
    /// allow it. Not reachable from the S1 corpus (the special decl is
    /// created by `declareArguments`, which needs the function visits), so
    /// it is pinned here directly.
    #[test]
    fn is_lvalue_rejects_the_special_arguments_decl_in_loose_mode() {
        let (is_lv, errors) = check_identifier_target(
            "arguments",
            DeclKind::Var,
            DeclSpecial::Arguments,
            false,
        );
        assert!(!is_lv);
        assert_eq!(errors, 1);
    }

    /// Non-identifier, non-member targets are not lvalues at all — the
    /// `return false` that ends `isLValue`.
    #[test]
    fn is_lvalue_rejects_a_literal() {
        let mut ctx = Context::new();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("lv.js", b"1");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let range = SMRange {
            start: loc,
            end: loc,
        };
        let gc = ctx.lock();
        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let lit = gc.alloc(Node::NumericLiteral(NumericLiteral::new(
            NodeMetadata::new(range),
            1.0,
        )));
        let binding_table = sem_ctx.binding_table_rc();
        let _scope = Scope::new(&binding_table);
        let is_lv = {
            let resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            resolver.is_lvalue(lit)
        };
        assert!(!is_lv);
    }

    /// `Decl::Kind` constness table sanity, so the two branches above are
    /// pinned to the kinds they claim to be about
    /// (`Decl::getKindConstness`, SemContext.h).
    #[test]
    fn constness_table_matches_the_kinds_used_above() {
        assert_eq!(DeclKind::Const.constness(), Constness::Always);
        assert_eq!(
            DeclKind::FunctionExprName.constness(),
            Constness::StrictModeOnly
        );
        assert_eq!(DeclKind::Let.constness(), Constness::Never);
    }
}
