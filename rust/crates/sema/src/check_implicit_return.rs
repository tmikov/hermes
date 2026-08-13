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
//!   still holds at that point carries the *pre-visit* body, with none of
//!   the resolver's rewrites applied: an arrow's expression body is still an
//!   expression rather than the `BlockStatement`
//!   [`may_reach_implicit_return`] requires, and a `try`/`catch`/`finally`
//!   has not been split into nested `try`s. So the caller passes the
//!   post-visit body and the `dyn_cast<BlockStatementNode>` half of
//!   `getBlockStatement` happens here.
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

use hermes_ast::node::{
    MatchStatement, Node, SwitchStatement, TryStatement,
};
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
/// Port of the class at CheckImplicitReturn.cpp:84-377. It holds no state —
/// as in C++, where the only constructor is `explicit CheckImplicitReturn()
/// {}` — so it exists purely to group the five `checkTermination*` methods
/// (and `isIrrefutableMatchPattern`, which is `static` in C++ too).
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

            // `#if HERMES_PARSE_FLOW` in C++ (cpp:152-156). This port has no
            // such build gate — the Flow grammar is a runtime `Context` flag
            // (`parse_flow_match`), so the arm is unconditional here, exactly
            // like the `TypeAlias`/`TypeCastExpression` visits in
            // `resolver/mod.rs`.
            Node::MatchStatement(n) => {
                self.check_termination_match_statement(n)
            }

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
                // `StaticBlock` is now the ONLY statement kind that would
                // trip this assert, and it cannot appear in a statement list
                // at all — exactly as in C++, where it is the only member of
                // ESTree.def's `Statement` range (ESTree.def:105-255) with no
                // arm above. The Flow-only `MatchStatement` used to be the
                // other one; upstream `653e49c60` gave it a real arm
                // (cpp:152-156), mirrored above, which is what makes this
                // comment true again.
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
    /// A try statement may have a handler, a finalizer, or both. In compile
    /// mode SemanticResolver rewrites the both-present form into nested try
    /// statements before this runs, but in parser mode
    /// (`resolve_ast_for_parser`) it does not, so the node reaching here can
    /// still carry both children. The rewrite is only a restatement of the
    /// language semantics — "try B catch H finally F" is
    /// "try { try B catch H } finally F" — so the two cases are composed here
    /// in that same order rather than handled by a third rule.
    fn check_termination_try_statement(
        &self,
        node: &TryStatement<'_>,
    ) -> TerminationResult {
        debug_assert!(
            node.handler.is_some() || node.finalizer.is_some(),
            "try statement must have a handler or a finalizer"
        );

        // The result of the protected block together with its handler, i.e.
        // of the inner "try B catch H" when both children are present.
        let mut inner_res = self.check_termination(node.block);
        if let Some(handler) = node.handler {
            // Both the try and catch must be terminating for the pair to
            // terminate.
            let catch_clause = handler
                .as_catch_clause()
                .expect("a TryStatement handler is a CatchClause");
            let catch_res = self.check_termination(catch_clause.body);
            inner_res.target_labels.extend(catch_res.target_labels);
        }

        // C++ passes `node->_finalizer` straight to `checkTermination` after
        // the null test; here the `Option` is unwrapped by the `let else`,
        // which is the same test. A `TryStatement` with neither child is what
        // the assert above rules out; the parser never builds one.
        let Some(finalizer) = node.finalizer else {
            return inner_res;
        };
        self.check_termination_finalizer(inner_res, finalizer)
    }

    /// \return the termination result of a try-finally whose protected part
    /// has the termination result \p try_res and whose finalizer is
    /// \p finalizer.
    /// \p try_res describes the whole "try B" or "try B catch H" that the
    /// finally protects, since the finalizer runs however that part
    /// completes.
    fn check_termination_finalizer(
        &self,
        mut try_res: TerminationResult,
        finalizer: &Node,
    ) -> TerminationResult {
        let finally_res = self.check_termination(finalizer);
        if finally_res.must_terminate() {
            // If the finally block terminates, the try-finally will
            // terminate after executing the finally.
            return finally_res;
        }
        if try_res.must_terminate() && finally_res.must_execute_next_statement()
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

    // The two helpers below are `#if HERMES_PARSE_FLOW` in C++
    // (CheckImplicitReturn.cpp:322-376, upstream `653e49c60`); this port has
    // no build gate for the Flow grammar — see the `MatchStatement` arm of
    // [`Self::check_termination`].

    /// \return true if \p pattern accepts every value, so a case using it runs
    /// whenever it is reached. This is the 'match' equivalent of a switch's
    /// default case.
    fn is_irrefutable_match_pattern(pattern: &Node) -> bool {
        // 'as' only names what the inner pattern matched, it does not narrow
        // what is accepted.
        let mut pattern = pattern;
        while let Node::MatchAsPattern(as_pattern) = pattern {
            pattern = as_pattern.pattern;
        }
        matches!(
            pattern,
            Node::MatchWildcardPattern(_) | Node::MatchBindingPattern(_)
        )
    }

    /// \return the termination result of a Flow 'match' statement.
    /// A match statement is not a break target in Hermes: SemanticResolver
    /// never records it in currentLoopOrSwitch, so a 'break' inside a case
    /// body always refers to an enclosing loop or switch. Consequently there
    /// is no label to remove here, and the labels targeted by the case bodies
    /// are simply unioned so that such breaks are propagated to the enclosing
    /// construct.
    fn check_termination_match_statement(
        &self,
        node: &MatchStatement<'_>,
    ) -> TerminationResult {
        let mut result = TerminationResult::default();
        // Whether some case is guaranteed to run, which is what lets the match
        // as a whole terminate. Mirrors the default case of a switch
        // statement.
        let mut found_irrefutable = false;
        for child in node.cases.iter() {
            let match_case = child
                .as_match_statement_case()
                .expect("a MatchStatement case is a MatchStatementCase");
            // Cases don't fall through, so every body is checked
            // independently. A body which completes normally continues after
            // the match, which the union below propagates.
            let case_res = self.check_termination(match_case.body);
            result.target_labels.extend(case_res.target_labels);
            // A guard can fail no matter what the pattern accepts.
            if match_case.guard.is_none()
                && Self::is_irrefutable_match_pattern(match_case.pattern)
            {
                found_irrefutable = true;
                // Cases are tested in order and the first match wins, so no
                // later case can run. Stop instead of unioning labels that a
                // dead case targets, which would report control flow that
                // cannot happen.
                break;
            }
        }
        if !found_irrefutable {
            // No case has to run, so execution may continue past the match.
            // Note that exhaustiveness of the patterns taken together is not
            // computed here, so a match which covers its argument by
            // enumeration is still treated as able to complete normally.
            result
                .target_labels
                .insert(TerminationResult::K_NEXT_STATEMENT_LABEL);
        }
        result
    }
}

/// Port of `mayReachImplicitReturn(ESTree::FunctionLikeNode *root)`
/// (CheckImplicitReturn.cpp:381-393).
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
