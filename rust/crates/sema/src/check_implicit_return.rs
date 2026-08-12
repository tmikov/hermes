/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of `lib/Sema/CheckImplicitReturn.cpp` — the conservative
//! reachability analysis behind `FunctionInfo::mayReachImplicitReturn`
//! (SemContext.h:354).
//!
//! The C++ file declares its one entry point in the *internal*
//! `lib/Sema/SemanticResolver.h:718` (not in the public `SemResolve.h`) and
//! puts everything else in an anonymous namespace, so this module is private
//! to the crate and only [`may_reach_implicit_return`] is `pub(crate)`. Its
//! single caller is `resolver::functions`'s port of
//! `visitFunctionBodyAfterParamsVisited` (cpp:1953-1958).
//!
//! ## What the analysis is
//!
//! `checkTermination` answers, for one statement, "where can control go from
//! here?" as a set of *label indices* — the same indices
//! `SemanticResolver`'s loop/switch/labeled-statement visits allocate and
//! that `break`/`continue` are resolved to (`resolver::statements`, S2 T1).
//! A `break L` says "control continues after the statement whose index is
//! L"; the enclosing statement that owns L erases it and turns it into "and
//! then the next statement runs". The special member
//! [`TerminationResult::K_NEXT_STATEMENT_LABEL`] is "the next statement in
//! this list runs". An empty set means the statement always ends the
//! function. The function may reach the implicit `return undefined` exactly
//! when its body block's set still contains the next-statement marker.
//!
//! It is deliberately conservative in one big way: loop *conditions* are
//! never examined, so `while (true) { ... }` and `for (;;) { ... }` are
//! reported as able to fall out of the loop. Comments and structure are kept
//! as in the C++ so the two stay diffable.
//!
//! ## Deviations
//!
//! - **The entry point takes the body, not the function.** C++'s
//!   `mayReachImplicitReturn(FunctionLikeNode *root)` starts with
//!   `getBlockStatement(root)` (lib/AST/ESTree.cpp:58-81). It can, because
//!   C++ mutates the AST in place, so `root->_body` is up to date by the
//!   time the resolver reaches cpp:1953. This port rebuilds nodes instead
//!   (see `resolver`'s module doc), and the function-like node the resolver
//!   still holds at that point carries the *pre-visit* body — which for a
//!   `try`/`catch`/`finally` has not been split into nested `try`s yet and
//!   would trip [`CheckImplicitReturn::check_termination_try_statement`]'s
//!   assert. So the caller passes the post-visit body and the
//!   `dyn_cast<BlockStatementNode>` half of `getBlockStatement` happens
//!   here.
//! - **`LabelDecorationBase *` becomes a `u32`.**
//!   `checkTerminationLoopOrLabeledStatement` takes the decoration base only
//!   to call `getLabelIndex()` on it; every call site upcasts a statically
//!   known node kind (cpp:114-143), so the ported helper takes the index
//!   itself. `resolver::statements`'s `label_index_of` — the port of the
//!   *dynamic* `getLabelDecorationBase` — is deliberately not involved, just
//!   as it is not in the C++ of this file.
//! - **`llvh::SmallDenseSet<unsigned, 2>` becomes [`HashSet<u32>`].** Only
//!   membership, size and emptiness are ever asked of it. The
//!   `kNextStatementLabel` value is kept bit-identical even though the
//!   reason the C++ comment gives for it (`DenseMapInfo`'s reserved empty
//!   and tombstone keys) does not apply to `HashSet`.

use std::collections::HashSet;

use hermes_ast::node::{Node, SwitchStatement, TryStatement};
use hermes_ast::node_child::NodeList;

/// Encodes the result of checking for termination in CheckImplicitReturn.
/// targetLabels encodes where execution may continue to.
///
/// Port of the class at CheckImplicitReturn.cpp:23-80.
#[derive(Default)]
struct TerminationResult {
    /// Set of target label indices.
    /// Having multiple labels in this set indicates that each of them may be
    /// reached in some execution of the function.
    ///
    /// There is a special label:
    /// kNextStatementLabel indicates that the next statement may execute.
    ///
    /// Other labels indicate that the corresponding LabelDecorationBase may
    /// complete and execute the statement following it.
    /// e.g. If a SwitchStatement's associated label index is in this set,
    /// then the switch may complete and the statement after the switch may
    /// run.
    ///
    /// Only continuation points within the same function are tracked.
    /// if there's no elements in this set, then the queried statement
    /// definitely terminates (ends execution of the function).
    target_labels: HashSet<u32>,
}

impl TerminationResult {
    /// Indicates execution that continues to the next statement when stored
    /// in targetLabels.
    /// Offset the invalid key by 2 to avoid interfering with DenseMapInfo.
    const K_NEXT_STATEMENT_LABEL: u32 = u32::MAX - 2;

    /// \return true when the target label set contains kNextStatementLabel,
    /// indicating that the execution can continue to the following
    /// statement.
    fn may_execute_next_statement(&self) -> bool {
        self.target_labels.contains(&Self::K_NEXT_STATEMENT_LABEL)
    }
    /// \return true when the target label set ONLY contains
    /// kNextStatementLabel, indicating that the execution can continue to
    /// the following statement.
    fn must_execute_next_statement(&self) -> bool {
        self.target_labels.len() == 1 && self.may_execute_next_statement()
    }
    /// \return true when the target label set is empty, indicating that we
    /// can't execute any more statements anywhere inside the current
    /// function.
    fn must_terminate(&self) -> bool {
        self.target_labels.is_empty()
    }

    /// \return a TerminationResult with one label: \p label_index.
    fn make_single_label(label_index: u32) -> TerminationResult {
        let mut result = TerminationResult::default();
        result.target_labels.insert(label_index);
        result
    }
    /// \return a TerminationResult with one label: continue to next
    /// statement.
    fn make_next_statement() -> TerminationResult {
        Self::make_single_label(Self::K_NEXT_STATEMENT_LABEL)
    }
    /// \return a TerminationResult with no labels, indicating no control
    /// flow to any other statements in this function.
    fn make_must_terminate() -> TerminationResult {
        TerminationResult::default()
    }
}

/// Runs a conservative check to determine whether there are any possible
/// paths through the function which end in an implicit 'undefined' return.
///
/// Port of the class at CheckImplicitReturn.cpp:84-316. It holds no state —
/// as in C++, where the only constructor is `explicit CheckImplicitReturn()
/// {}` — so it exists purely to group the four `checkTermination*` methods.
struct CheckImplicitReturn;

impl CheckImplicitReturn {
    /// \return the termination result of any \p node.
    fn check_termination(&self, node: &Node) -> TerminationResult {
        match node {
            Node::BlockStatement(block) => {
                self.check_termination_statement_list(block.body)
            }

            Node::IfStatement(if_statement) => {
                let mut consequent_res =
                    self.check_termination(if_statement.consequent);
                if let Some(alternate) = if_statement.alternate {
                    // All targets from both branches are possible.
                    let alternate_res = self.check_termination(alternate);
                    consequent_res
                        .target_labels
                        .extend(alternate_res.target_labels);
                } else {
                    // No alternate, so this statement can also continue.
                    consequent_res
                        .target_labels
                        .insert(TerminationResult::K_NEXT_STATEMENT_LABEL);
                }
                // Added the additional labels to the consequentRes, return
                // it.
                consequent_res
            }

            Node::ForStatement(n) => self
                .check_termination_loop_or_labeled_statement(
                    n.label_index.get(),
                    n.body,
                    false,
                ),
            Node::ForInStatement(n) => self
                .check_termination_loop_or_labeled_statement(
                    n.label_index.get(),
                    n.body,
                    false,
                ),
            Node::ForOfStatement(n) => self
                .check_termination_loop_or_labeled_statement(
                    n.label_index.get(),
                    n.body,
                    false,
                ),
            Node::WhileStatement(n) => self
                .check_termination_loop_or_labeled_statement(
                    n.label_index.get(),
                    n.body,
                    false,
                ),
            Node::DoWhileStatement(n) => self
                .check_termination_loop_or_labeled_statement(
                    n.label_index.get(),
                    n.body,
                    true,
                ),
            Node::LabeledStatement(n) => self
                .check_termination_loop_or_labeled_statement(
                    n.label_index.get(),
                    n.body,
                    true,
                ),

            Node::SwitchStatement(n) => {
                self.check_termination_switch_statement(n)
            }
            Node::TryStatement(n) => self.check_termination_try_statement(n),

            Node::ReturnStatement(_) => {
                // Explicit return will always prevent implicit return.
                TerminationResult::make_must_terminate()
            }
            Node::ThrowStatement(_) => {
                // Throw will prevent the next statement in the current list
                // from executing. It's possible it will result in execution
                // of a catch or finally in this function, but that is
                // handled at the TryStatement level.
                TerminationResult::make_must_terminate()
            }

            Node::ContinueStatement(n) => {
                // For 'continue', conservatively assume that the condition
                // of the loop could be false and we will run the statement
                // after the loop.
                TerminationResult::make_single_label(n.label_index.get())
            }
            Node::BreakStatement(n) => {
                TerminationResult::make_single_label(n.label_index.get())
            }

            Node::WithStatement(n) => self.check_termination(n.body),

            Node::DebuggerStatement(_)
            | Node::EmptyStatement(_)
            | Node::ExpressionStatement(_) => {
                TerminationResult::make_next_statement()
            }

            _ => {
                // The two statement kinds that would trip this assert are
                // `StaticBlock`, which cannot appear in a statement list at
                // all, and the Flow-only `MatchStatement` — exactly as in
                // C++, where both are inside ESTree.def's `Statement` range
                // (ESTree.def:105-255) and neither has an arm above.
                debug_assert!(
                    !node.is_statement(),
                    "unhandled statement in statement list"
                );
                // This is not a JS statement, so it's not going to do any
                // control flow. e.g. this could be a TypeAliasNode, Import,
                // Export, or some other non-executing statement.
                TerminationResult::make_next_statement()
            }
        }
    }

    /// \return the termination result of the provided list of statements.
    fn check_termination_statement_list(
        &self,
        stmts: NodeList<'_>,
    ) -> TerminationResult {
        let mut result = TerminationResult::default();
        for stmt in stmts.iter() {
            // Check for continuation from previous statement and erase the
            // continue, because this is the continuation.
            result
                .target_labels
                .remove(&TerminationResult::K_NEXT_STATEMENT_LABEL);

            // Add all the possible target labels to the final result.
            let stmt_res = self.check_termination(stmt);
            // C++ reads `stmtRes.mayExecuteNextStatement()` after copying
            // its labels out; here the copy consumes `stmt_res`, so the
            // query is hoisted above it. Same answer either way — the copy
            // does not touch the source set.
            let may_execute_next_statement =
                stmt_res.may_execute_next_statement();
            result.target_labels.extend(stmt_res.target_labels);
            if !may_execute_next_statement {
                // Statement list doesn't continue, so we're done scanning
                // it.
                return result;
            }
        }
        // Made it through the whole statement list.
        result
            .target_labels
            .insert(TerminationResult::K_NEXT_STATEMENT_LABEL);
        result
    }

    /// \return the termination result of a loop or a labeled statement.
    /// \param label_index the label index of the loop or labeled statement
    ///   (C++ passes the `LabelDecorationBase *` it lives on — see the
    ///   module doc).
    /// \param must_execute whether the body must run at least once.
    fn check_termination_loop_or_labeled_statement(
        &self,
        label_index: u32,
        body: &Node,
        must_execute: bool,
    ) -> TerminationResult {
        // Whether the function may continue execution on the next statement
        // after the loop/labeled statement.
        // * Loops with preconditions aren't guaranteed to execute so they
        // may continue.
        // * Do-while must run the loop body at least once.
        // * break/continue with the label associated with the statement may
        // continue to the next statement (checked below).
        let mut may_execute_next_statement = !must_execute;

        let mut body_res = self.check_termination(body);
        if body_res.target_labels.remove(&label_index) {
            // Breaks within this labeled statement are continues after it.
            may_execute_next_statement = true;
        }

        if may_execute_next_statement {
            body_res
                .target_labels
                .insert(TerminationResult::K_NEXT_STATEMENT_LABEL);
        }
        body_res
    }

    /// \return the termination result of a try statement.
    fn check_termination_try_statement(
        &self,
        node: &TryStatement<'_>,
    ) -> TerminationResult {
        let mut try_res = self.check_termination(node.block);

        debug_assert!(
            !(node.handler.is_some() && node.finalizer.is_some()),
            "try-catch-finally should have been transformed by \
             SemanticResolver"
        );
        if let Some(handler) = node.handler {
            // Both the try and catch must be terminating if there's no
            // finalizer.
            let catch_clause = handler
                .as_catch_clause()
                .expect("a TryStatement handler is a CatchClause");
            let catch_res = self.check_termination(catch_clause.body);
            try_res.target_labels.extend(catch_res.target_labels);
            try_res
        } else {
            // C++ passes `node->_finalizer` straight to `checkTermination`,
            // which would dereference null if a `TryStatement` had neither a
            // handler nor a finalizer; the parser never builds one.
            let finalizer = node
                .finalizer
                .expect("a TryStatement has a handler or a finalizer");
            let finally_res = self.check_termination(finalizer);
            if finally_res.must_terminate() {
                // If the finally block terminates, the try-finally will
                // terminate after executing the finally.
                return finally_res;
            }
            if try_res.must_terminate()
                && finally_res.must_execute_next_statement()
            {
                // If the try definitely terminates and the finally can't
                // break to another handler in this function, the try-finally
                // will definitely terminate.
                // However, we also check that the finally has no control
                // flow that would prevent the try from being able to
                // terminate, e.g.
                //     label:
                //       try { return 1; }
                //       finally { break label; }
                // needs to avoid this branch, which mustExecuteNextStatement
                // checks.
                return try_res;
            }
            // Otherwise, we just combine the possible next points of the
            // try-finally.
            try_res.target_labels.extend(finally_res.target_labels);
            try_res
        }
    }

    /// \return the termination result of a switch statement, accounting for
    /// breaks and fallthrough.
    fn check_termination_switch_statement(
        &self,
        node: &SwitchStatement<'_>,
    ) -> TerminationResult {
        let mut result = TerminationResult::default();
        let mut found_default = false;
        for child in node.cases.iter() {
            // Check for fallthrough from previous case and erase the
            // continue, because this is the continuation.
            result
                .target_labels
                .remove(&TerminationResult::K_NEXT_STATEMENT_LABEL);

            let switch_case = child
                .as_switch_case()
                .expect("a SwitchStatement case is a SwitchCase");
            if switch_case.test.is_none() {
                found_default = true;
            }

            let case_res =
                self.check_termination_statement_list(switch_case.consequent);
            result.target_labels.extend(case_res.target_labels);
        }

        // Check for explicit break from this switch statement.
        // If there's an explicit break, remove it from the result.
        let found_explicit_break =
            result.target_labels.remove(&node.label_index.get());

        // If we found explicit breaks or if there's no default case, we can
        // make it past this switch statement.
        if found_explicit_break || !found_default {
            result
                .target_labels
                .insert(TerminationResult::K_NEXT_STATEMENT_LABEL);
        }

        result
    }
}

/// Port of `mayReachImplicitReturn(ESTree::FunctionLikeNode *root)`
/// (CheckImplicitReturn.cpp:320-332).
///
/// \param body the function's body node **after** it has been visited by the
///   resolver — C++ takes the function-like node and calls
///   `ESTree::getBlockStatement` on it, which this port cannot do; see the
///   module doc.
pub(crate) fn may_reach_implicit_return(body: &Node) -> bool {
    let visitor = CheckImplicitReturn;
    // `getBlockStatement` yields null for anything that is not a
    // `BlockStatement`. Arrow functions have their bodies turned into
    // BlockStatement before visit, but only in compile_ mode.
    if !matches!(body, Node::BlockStatement(_)) {
        return false;
    }
    let result = visitor.check_termination(body);
    debug_assert!(
        result.target_labels.is_empty() || result.must_execute_next_statement(),
        "all user-declared labels must be removed by the end of the function"
    );
    result.may_execute_next_statement()
}
