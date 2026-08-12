/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Tests for `CheckImplicitReturn` (`lib/Sema/CheckImplicitReturn.cpp`), the
//! conservative reachability analysis that decides
//! `FunctionInfo::mayReachImplicitReturn`.
//!
//! Unlike everything else in the resolver, this analysis is **invisible to
//! `-dump-sema`**: `SemContextDumper` never prints the flag, so
//! `sema_differential.rs` cannot see it and these tests are the only gate.
//! Every expected value below is derived by reading
//! `lib/Sema/CheckImplicitReturn.cpp` (cited per case) — including the cases
//! where the C++ answer is deliberately *wrong but conservative*
//! (`while (true) {}` "may" fall off the end).
//!
//! The tests go through the whole resolver rather than calling the analysis
//! directly, which also pins the wiring (SemanticResolver.cpp:1939-1944):
//! the flag must be written on the right `FunctionInfo`, and only when the
//! resolution produced no errors.
//!
//! The parse-driver setup is trimmed from `resolver.rs`'s.

use hermes_ast::context::{Context, GCLock};
use hermes_ast::node::Node;
use hermes_parser::js::JSParserImpl;
use hermes_parser::lexer::{GrammarContext, JSLexer};
use hermes_sema::ids::FunctionInfoId;
use hermes_sema::keywords::Keywords;
use hermes_sema::resolve::resolve_ast;
use hermes_sema::sem_context::SemContext;
use hermes_support::manager::SourceErrorManager;

/// Parse `src` as a `Program` and return its root node, panicking on any
/// parse error.
fn parse<'gc>(
    gc: &'gc GCLock,
    sm: &mut SourceErrorManager,
    src: &str,
) -> &'gc Node<'gc> {
    let buf_id = sm.add_buffer_bytes("input", src.as_bytes());
    let result: Option<&Node> = {
        let atoms = &gc.ctx().atom_table;
        let lexer =
            JSLexer::new(buf_id, sm, atoms, GrammarContext::AllowRegExp);
        let mut parser = JSParserImpl::new(gc, lexer);
        parser.parse()
    };
    assert_eq!(sm.error_count(), 0, "unexpected parse errors in: {src}");
    result.expect("parser returned no Program")
}

/// Resolve `src` and return `may_reach_implicit_return` for every
/// `FunctionInfo`, in creation order — index 0 is the global function, index
/// 1 the first function in source order, and so on — paired with the
/// resolution error count.
fn flags_and_errors(src: &str) -> (Vec<bool>, u32) {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    // The result is deliberately dropped: the two error-path tests below
    // want the `SemContext` that a *failed* resolution leaves behind.
    let _resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[]);
    let flags = (0..sem_ctx.functions_len())
        .map(|i| {
            let id = FunctionInfoId::from_sema_id(hermes_ast::SemaId(i as u32));
            sem_ctx.function(id).may_reach_implicit_return
        })
        .collect();
    (flags, sm.error_count())
}

/// [`flags_and_errors`] for a source that must resolve cleanly.
fn flags(src: &str) -> Vec<bool> {
    let (flags, errors) = flags_and_errors(src);
    assert_eq!(errors, 0, "unexpected resolution errors in: {src}");
    flags
}

/// The flag of the first function in `src` (source order).
fn flag(src: &str) -> bool {
    let flags = flags(src);
    assert!(flags.len() >= 2, "no function in: {src}");
    flags[1]
}

/// The flag of `function f() { <body> }`. `x`, `y` and `o` are declared so
/// that no case below depends on how unresolved globals are treated.
fn body_flag(body: &str) -> bool {
    flag(&format!("var x, y, o;\nfunction f() {{\n{body}\n}}\n"))
}

/// Runs a table of `(body, expected)` rows through [`body_flag`].
#[track_caller]
fn check_bodies(rows: &[(&str, bool)]) {
    for (body, expected) in rows {
        assert_eq!(
            body_flag(body),
            *expected,
            "mayReachImplicitReturn of `function f() {{ {body} }}`"
        );
    }
}

// ---- BlockStatement / statement lists -----------------------------------
//
// cpp:91-94 (BlockStatement) and cpp:193-212
// (checkTerminationStatementList): a list continues iff every statement in
// it continues, and the trailing `insert(kNextStatementLabel)` at cpp:210
// is what makes an empty list — and a body that falls off the end — reach
// the implicit return.

#[test]
fn statement_lists_fall_off_the_end() {
    check_bodies(&[
        // Empty body: the `for` at cpp:195 never runs, cpp:210 inserts
        // kNextStatementLabel.
        ("", true),
        // ExpressionStatement / EmptyStatement / DebuggerStatement all
        // return makeNextStatement() (cpp:175-178).
        ("x;", true),
        (";", true),
        ("debugger;", true),
        // A nested block is just another statement list (cpp:91-94).
        ("{ x; }", true),
        ("{ }", true),
    ]);
}

#[test]
fn return_and_throw_terminate_the_list() {
    check_bodies(&[
        // ReturnStatement -> makeMustTerminate (cpp:152-154).
        ("return;", false),
        ("return 1;", false),
        // ThrowStatement -> makeMustTerminate (cpp:155-160).
        ("throw x;", false),
        // A terminating statement ends the scan (cpp:204-207), so the
        // unreachable statement after it cannot put kNextStatementLabel
        // back.
        ("return 1; x;", false),
        ("throw x; x;", false),
        // Reached through the nested list (cpp:91-94).
        ("{ return 1; }", false),
        ("x; { x; throw x; }", false),
    ]);
}

#[test]
fn non_statement_children_of_a_block_just_continue() {
    // The `default` arm (cpp:180-187): a node that is not an
    // ESTree::StatementNode does no control flow. `VariableDeclaration`,
    // `FunctionDeclaration` and `ClassDeclaration` are all outside
    // ESTree.def's `Statement` range (ESTree.def:105-255), so they land
    // here rather than on the `assert(!isa<StatementNode>(node))`.
    check_bodies(&[
        ("var v = 1;", true),
        ("let v = 1;", true),
        ("function g() { return 1; }", true),
        ("class C {}", true),
        ("var v = 1; return 1;", false),
    ]);
}

// ---- IfStatement --------------------------------------------------------
//
// cpp:96-112: the union of both branches, or of the consequent and
// "continue" when there is no alternate.

#[test]
fn if_statement_unions_its_branches() {
    check_bodies(&[
        // Both branches terminate -> no targets at all (cpp:101-104).
        ("if (x) return 1; else return 2;", false),
        ("if (x) { return 1; } else { throw x; }", false),
        (
            "if (x) { return 1; } else { if (y) return 2; else return 3; }",
            false,
        ),
        // No alternate: cpp:106-108 adds kNextStatementLabel explicitly.
        ("if (x) return 1;", true),
        // One branch falls through.
        ("if (x) return 1; else x;", true),
        ("if (x) x; else return 1;", true),
        // The if terminates, so the list stops scanning at it.
        ("if (x) return 1; else return 2; x;", false),
        // The if continues, so the statement after it decides.
        ("if (x) return 1; return 2;", false),
    ]);
}

// ---- Loops --------------------------------------------------------------
//
// cpp:114-138 dispatch to checkTerminationLoopOrLabeledStatement
// (cpp:216-241) with `mustExecute` false for the four pre-condition loops
// and true for `do`-`while`.

#[test]
fn precondition_loops_always_continue() {
    // `mayExecuteNextStatement = !mustExecute` (cpp:227) is `true` for
    // `while`/`for`/`for-in`/`for-of` regardless of the body, because the
    // condition may be false on the first iteration. This is where the
    // analysis is deliberately conservative: cpp never looks at the test
    // expression, so `while (true)` and `for (;;)` — which cannot fall out
    // — are still reported as reaching the implicit return.
    check_bodies(&[
        ("while (x) return 1;", true),
        ("while (true) { }", true),
        ("while (true) { return 1; }", true),
        ("while (true) { throw x; }", true),
        ("for (;;) return 1;", true),
        ("for (var k = 0; k < 10; ++k) return 1;", true),
        ("for (var k in o) return 1;", true),
        ("for (var k of o) return 1;", true),
    ]);
}

#[test]
fn do_while_must_run_its_body() {
    // `mustExecute` is true (cpp:134-138), so cpp:227 starts at `false` and
    // only a `break`/`continue` targeting this loop can add
    // kNextStatementLabel back.
    check_bodies(&[
        ("do return 1; while (x);", false),
        ("do { throw x; } while (x);", false),
        ("do { } while (x);", true),
        ("do { if (x) return 1; } while (x);", true),
        // `break` with no label targets this loop (SemanticResolver.cpp:
        // 709-713), so its label index is erased at cpp:231-235 and
        // cpp:237-239 puts kNextStatementLabel back.
        ("do { break; } while (x);", true),
        // Ditto `continue` — cpp:162-166 conservatively assumes the loop
        // condition may then be false.
        ("do { continue; } while (x);", true),
        // Unreachable code after the terminating do-while.
        ("do return 1; while (x); x;", false),
    ]);
}

#[test]
fn breaks_targeting_an_outer_statement_are_not_erased_by_the_inner_one() {
    // `break outer` resolves to the *loop*, not to the LabeledStatement
    // wrapping it (SemanticResolver.cpp:642-652 picks the loop as
    // targetStatement), so the inner do-while at cpp:230 does not find its
    // own index in the body result and stays terminating.
    check_bodies(&[
        ("outer: do { do { break outer; } while (y); } while (x);", true),
        ("outer: do { do { return 1; } while (y); } while (x);", false),
        // Same for a labeled `continue`, which resolves to the loop too
        // (SemanticResolver.cpp:728-731).
        (
            "outer: do { do { continue outer; } while (y); } while (x);",
            true,
        ),
        // The label is erased by the statement that owns it, and the
        // `return` after it then terminates the list.
        ("outer: do { break outer; } while (x); return 1;", false),
        (
            "outer: do { do { continue outer; } while (y); } while (x); \
             return 1;",
            false,
        ),
    ]);
}

// ---- LabeledStatement ---------------------------------------------------
//
// cpp:139-143: a LabeledStatement is checkTerminationLoopOrLabeledStatement
// with `mustExecute` true. Its own label index is only ever the target of a
// `break` when the labeled statement is not a loop (SemanticResolver.cpp:
// 642-652).

#[test]
fn labeled_statement_body_must_execute() {
    check_bodies(&[
        ("L: { return 1; }", false),
        ("L: { throw x; }", false),
        ("L: { }", true),
        // The break's target index is the LabeledStatement's own, so
        // cpp:231-235 erases it and cpp:237-239 continues.
        ("L: { break L; }", true),
        ("L: { if (x) break L; return 1; }", true),
        // ... and the statement after the label then decides.
        ("L: { break L; } return 1;", false),
        ("L: L2: { break L; } return 1;", false),
    ]);
}

// ---- SwitchStatement ----------------------------------------------------
//
// cpp:286-315: fallthrough between cases, `break` erasure, and the
// "no default means the switch may be skipped entirely" rule.

#[test]
fn switch_without_default_may_be_skipped() {
    // `!foundDefault` at cpp:310 inserts kNextStatementLabel.
    check_bodies(&[
        ("switch (x) { }", true),
        ("switch (x) { case 1: return 1; }", true),
        ("switch (x) { case 1: return 1; case 2: return 2; }", true),
    ]);
}

#[test]
fn switch_with_exhaustive_default_terminates() {
    check_bodies(&[
        ("switch (x) { default: return 1; }", false),
        ("switch (x) { case 1: return 1; default: return 2; }", false),
        ("switch (x) { default: return 1; case 1: return 2; }", false),
        ("switch (x) { default: throw x; }", false),
        // A default whose consequent is empty falls through the end of the
        // switch: checkTerminationStatementList on an empty list returns
        // kNextStatementLabel (cpp:210).
        ("switch (x) { default: }", true),
        // Fallthrough: `case 1:` is empty, so its result continues into
        // `default:` (cpp:293) and only the default's `return` remains.
        ("switch (x) { case 1: default: return 1; }", false),
        // Fallthrough out of the last case.
        ("switch (x) { default: case 1: }", true),
    ]);
}

#[test]
fn switch_break_makes_the_switch_completable() {
    // cpp:306: erasing the switch's own label index reports `true`, so
    // cpp:310-312 inserts kNextStatementLabel.
    check_bodies(&[
        ("switch (x) { default: break; }", true),
        ("switch (x) { case 1: break; default: return 1; }", true),
        ("switch (x) { default: if (x) break; return 1; }", true),
        // The switch continues, so the statement after it decides.
        ("switch (x) { default: break; } return 1;", false),
        // A `break` in a switch nested in a terminating switch's default is
        // erased by the inner switch only.
        (
            "switch (x) { default: switch (y) { default: break; } return 1; }",
            false,
        ),
    ]);
}

// ---- TryStatement -------------------------------------------------------
//
// cpp:244-282. Note the assert at cpp:248-250: `try`/`catch`/`finally` is
// rewritten into nested `try`s by the resolver
// (SemanticResolver.cpp:771-811), so only try-catch and try-finally reach
// the analysis.

#[test]
fn try_catch_unions_both_bodies() {
    // cpp:251-257.
    check_bodies(&[
        ("try { return 1; } catch (e) { return 2; }", false),
        ("try { throw x; } catch (e) { throw x; }", false),
        ("try { return 1; } catch (e) { }", true),
        ("try { } catch (e) { return 1; }", true),
        ("try { } catch (e) { }", true),
        // ES2019 optional catch binding, same shape.
        ("try { return 1; } catch { return 2; }", false),
    ]);
}

#[test]
fn try_finally_is_decided_by_the_finalizer_first() {
    check_bodies(&[
        // The finalizer terminates -> the whole statement does (cpp:260-264).
        ("try { } finally { return 1; }", false),
        ("try { x; } finally { throw x; }", false),
        // The try terminates and the finalizer only continues -> the try's
        // result wins (cpp:265-276).
        ("try { return 1; } finally { }", false),
        ("try { return 1; } finally { x; }", false),
        // Neither terminates -> union (cpp:277-280).
        ("try { } finally { }", true),
        ("try { if (x) return 1; } finally { }", true),
    ]);
}

#[test]
fn a_finalizer_that_breaks_out_defeats_the_terminating_try() {
    // The case called out by the C++ comment at cpp:269-274: the try
    // definitely terminates, but the finalizer's `break label` means
    // control reaches the statement after the label instead, so
    // `mustExecuteNextStatement()` (cpp:265) must reject the shortcut.
    check_bodies(&[
        ("L: try { return 1; } finally { break L; }", true),
        // Without the `break` the same shape terminates.
        ("L: try { return 1; } finally { }", false),
        // And what follows the label is then reachable.
        ("L: try { return 1; } finally { break L; } return 2;", false),
    ]);
}

#[test]
fn try_catch_finally_is_analyzed_after_the_resolver_rewrite() {
    // SemanticResolver.cpp:771-811 turns these into
    // `try { try {} catch {} } finally {}` before the analysis runs, so the
    // outer statement is a try-finally over a block holding a try-catch.
    check_bodies(&[
        (
            "try { return 1; } catch (e) { return 2; } finally { return 3; }",
            false,
        ),
        ("try { return 1; } catch (e) { return 2; } finally { }", false),
        ("try { return 1; } catch (e) { } finally { }", true),
        ("try { } catch (e) { } finally { }", true),
    ]);
}

// ---- The wiring (SemanticResolver.cpp:1939-1944) ------------------------

#[test]
fn the_flag_is_per_function() {
    // Each function's own body decides its own flag, and a nested function
    // declaration is invisible to the enclosing analysis.
    let f = flags(
        "function a() { return 1; }\n\
         function b() { }\n\
         function c() { function d() { } return 1; }\n",
    );
    // 0 is the global function; a, b, c, d are entered in that order (`d`
    // last, because `c`'s body is visited after `c` is created).
    assert_eq!(f, vec![true, false, true, false, true]);
}

#[test]
fn the_program_node_keeps_the_default() {
    // `visit(ProgramNode *)` (SemanticResolver.cpp:193-231) does not go
    // through visitFunctionLikeInFunctionContext, so the global function's
    // flag is never computed and keeps SemContext.h:354's `true` — even
    // when the program itself ends in a `return`-like shape.
    assert!(flags("x;")[0]);
    assert!(flags("var x;\nfunction f() { return 1; }\n")[0]);
}

#[test]
fn arrow_functions_use_the_rewritten_block_body() {
    // Rewrite #1 (SemanticResolver.cpp:249-275) turns `x => x` into
    // `x => { return x; }` before the visit, so `getBlockStatement` finds a
    // block and the `return` in it terminates.
    assert!(!flag("var f = x => x;"));
    assert!(!flag("var f = () => { return 1; };"));
    assert!(flag("var f = () => { };"));
    assert!(flag("var f = () => { if (0) return 1; };"));
}

#[test]
fn class_methods_and_accessors_get_the_flag() {
    // Methods go through the same visitFunctionLike path. Only the flag of
    // the method is asserted, by shape: index 0 is the global function and
    // the method is the only function-like node in the source.
    assert!(!flag("class C { m() { return 1; } }"));
    assert!(flag("class C { m() { } }"));
    assert!(!flag("var o2 = { get p() { return 1; } };"));
}

#[test]
fn generators_and_async_functions_are_not_special_cased() {
    // `yield`/`await` are expressions, so their statements just continue
    // (cpp:175-178).
    assert!(flag("function* g() { yield 1; }"));
    assert!(!flag("function* g() { yield 1; return 2; }"));
    assert!(flag("async function g() { }"));
}

#[test]
fn the_flag_is_not_computed_when_resolution_failed() {
    // cpp:1939-1942: "CheckImplicitReturn relies on break and continue
    // being properly resolved, and if there's errors during resolution they
    // might not be." An unresolved `break` has no label index at all, so
    // running the analysis would both read a garbage index and trip the
    // assert at cpp:328-330; the flag must stay at its default instead.
    let (f, errors) = flags_and_errors("function f() { break; return 1; }");
    assert_eq!(errors, 1, "expected the 'break' to be rejected");
    assert!(f[1], "the flag must keep SemContext.h:354's default");
}

#[test]
fn with_statements_never_reach_the_analysis() {
    // The WithStatement arm (cpp:171-173) is dead in this port: `with` is
    // rejected outright in compile mode (SemanticResolver.cpp:757-759), so
    // the error count is non-zero by the time the check would run. Pinned
    // so that a future non-compile mode notices the arm is untested.
    let (f, errors) = flags_and_errors("function f() { with (x) return 1; }");
    assert!(errors > 0, "expected `with` to be rejected");
    assert!(f[1], "the flag must keep SemContext.h:354's default");
}
