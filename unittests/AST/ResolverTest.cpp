/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "../Parser/DiagContext.h"
#include "hermes/Parser/JSParser.h"
#include "hermes/Sema/SemContext.h"
#include "hermes/Sema/SemResolve.h"

#include "gtest/gtest.h"

using namespace hermes;
using namespace hermes::parser;

namespace {

/// Left side of assignment must be an LValue.
TEST(ResolverTest, TestBadAssignmentLValue) {
  Context ctx;
  sema::SemContext semCtx(ctx);
  DiagContext diag(ctx);
  JSParser parser(ctx, "a + 1 = 10;");
  auto parsed = parser.parse();
  ASSERT_TRUE(parsed.hasValue());

  ASSERT_FALSE(sema::resolveAST(ctx, semCtx, *parsed));
  EXPECT_EQ(1, diag.getErrCount());
}

/// For-in control expression must be an LValue.
TEST(ResolverTest, TestBadForLValue) {
  Context ctx;
  sema::SemContext semCtx(ctx);
  DiagContext diag(ctx);
  JSParser parser(ctx, "for(a + 1 in x);");
  auto parsed = parser.parse();
  ASSERT_TRUE(parsed.hasValue());

  ASSERT_FALSE(resolveAST(ctx, semCtx, *parsed));
  EXPECT_EQ(1, diag.getErrCount());
}

/// Test an anonymous break outside of a loop.
TEST(ResolverTest, UnnamedBreakLabelTest) {
  Context ctx;
  sema::SemContext semCtx(ctx);
  DiagContext diag(ctx);
  JSParser parser(ctx, "break; for(;;) break; break;");
  auto parsed = parser.parse();
  ASSERT_TRUE(parsed.hasValue());

  ASSERT_FALSE(resolveAST(ctx, semCtx, *parsed));
  ASSERT_EQ(2, diag.getErrCountClear());
}

/// Test an anonymous continue outside of a loop.
TEST(ResolverTest, UnnamedContinueLabelTest) {
  Context ctx;
  sema::SemContext semCtx(ctx);
  DiagContext diag(ctx);
  JSParser parser(ctx, "continue;");
  auto parsed = parser.parse();
  ASSERT_TRUE(parsed.hasValue());

  ASSERT_FALSE(resolveAST(ctx, semCtx, *parsed));
  ASSERT_EQ(1, diag.getErrCountClear());
}

/// Test an anonymous continue outside of a loop.
TEST(ResolverTest, ContinueInASwitchTest) {
  Context ctx;
  sema::SemContext semCtx(ctx);
  DiagContext diag(ctx);
  JSParser parser(ctx, "switch(1) { case 1: continue; }");
  auto parsed = parser.parse();
  ASSERT_TRUE(parsed.hasValue());

  ASSERT_FALSE(resolveAST(ctx, semCtx, *parsed));
  ASSERT_EQ(1, diag.getErrCountClear());
}

/// Test a continue with a block label.
TEST(ResolverTest, ContinueWithBlockLabelTest) {
  Context ctx;
  sema::SemContext semCtx(ctx);
  DiagContext diag(ctx);
  JSParser parser(ctx, "label1: { continue label1; }");
  auto parsed = parser.parse();
  ASSERT_TRUE(parsed.hasValue());

  ASSERT_FALSE(resolveAST(ctx, semCtx, *parsed));
  ASSERT_EQ(1, diag.getErrCountClear());
}

/// Test that multiple labels are correctly attached to the same statement.
TEST(ResolverTest, ChainedNamedLabelsTest) {
  Context ctx;
  sema::SemContext semCtx(ctx);
  DiagContext diag(ctx);
  JSParser parser(
      ctx,
      "label1: label2: label3: for(;;) { continue label1; continue "
      "label2; continue label3; }");
  auto parsed = parser.parse();
  ASSERT_TRUE(parsed.hasValue());

  ASSERT_TRUE(resolveAST(ctx, semCtx, *parsed));
}

/// Duplicated label in the scope of the previous one.
TEST(ResolverTest, DuplicateNamedLabelTest) {
  Context ctx;
  sema::SemContext semCtx(ctx);
  DiagContext diag(ctx);
  JSParser parser(
      ctx,
      "label1: { label1: ; }\n"
      "label2: label2: ;");
  auto parsed = parser.parse();
  ASSERT_TRUE(parsed.hasValue());

  ASSERT_FALSE(resolveAST(ctx, semCtx, *parsed));
  ASSERT_EQ(2, diag.getErrCountClear());
}

TEST(ResolverTest, CorrectDuplicateNamedLabelTest) {
  Context ctx;
  sema::SemContext semCtx(ctx);
  DiagContext diag(ctx);
  JSParser parser(
      ctx, "label1: { break label1; } label1: for(;;) break label1;");
  auto parsed = parser.parse();
  ASSERT_TRUE(parsed.hasValue());
  ASSERT_TRUE(resolveAST(ctx, semCtx, *parsed));
}

TEST(ResolverTest, ScopeNamedLabelTest) {
  Context ctx;
  sema::SemContext semCtx(ctx);
  DiagContext diag(ctx);
  JSParser parser(ctx, "label1: ; for(;;) break label1;");
  auto parsed = parser.parse();
  ASSERT_TRUE(parsed.hasValue());

  ASSERT_FALSE(resolveAST(ctx, semCtx, *parsed));
  ASSERT_EQ(1, diag.getErrCountClear());
}

TEST(ResolverTest, NamedBreakLabelTest) {
  Context ctx;
  sema::SemContext semCtx(ctx);
  DiagContext diag(ctx);
  JSParser parser(
      ctx, "break exitLoop; exitLoop: for(;;) break exitLoop; break exitLoop;");
  auto parsed = parser.parse();
  ASSERT_TRUE(parsed.hasValue());

  ASSERT_FALSE(resolveAST(ctx, semCtx, *parsed));
  ASSERT_EQ(2, diag.getErrCountClear());
}

void assertFunctionLikeSourceVisibility(
    llvh::Optional<ESTree::FunctionLikeNode *> funcLikeNode,
    SourceVisibility sourceVisibility) {
  ASSERT_TRUE(funcLikeNode.hasValue());
  ASSERT_EQ(
      (*funcLikeNode)->getSemInfo()->customDirectives.sourceVisibility,
      sourceVisibility);
}

void assertFirstNodeAsFunctionLikeWithSourceVisibility(
    llvh::Optional<ESTree::ProgramNode *> parsed,
    SourceVisibility sourceVisibility) {
  ASSERT_TRUE(parsed.hasValue());
  auto *programNode = llvh::cast<ESTree::ProgramNode>(parsed.getValue());
  ASSERT_TRUE(llvh::isa<ESTree::FunctionLikeNode>(programNode->_body.front()));
  auto *funcLikeNode =
      llvh::cast<ESTree::FunctionLikeNode>(&programNode->_body.front());

  ASSERT_EQ(
      funcLikeNode->getSemInfo()->customDirectives.sourceVisibility,
      sourceVisibility);
}

void assertSecondNodeAsFunctionLikeWithSourceVisibility(
    llvh::Optional<ESTree::ProgramNode *> parsed,
    SourceVisibility sourceVisibility) {
  ASSERT_TRUE(parsed.hasValue());
  auto *programNode = llvh::cast<ESTree::ProgramNode>(parsed.getValue());
  auto it = programNode->_body.begin();
  // Step to the 2nd node.
  it++;
  ASSERT_TRUE(llvh::isa<ESTree::FunctionLikeNode>(*it));
  auto *funcLikeNode = llvh::cast<ESTree::FunctionLikeNode>(it);

  ASSERT_EQ(
      funcLikeNode->getSemInfo()->customDirectives.sourceVisibility,
      sourceVisibility);
}

TEST(ResolverTest, SourceVisibilityTest) {
  Context context;
  sema::SemContext semCtx(context);
  // Top-level program node.
  {
    JSParser parser(context, "");
    auto parsed = parser.parse();
    resolveAST(context, semCtx, *parsed);
    assertFunctionLikeSourceVisibility(*parsed, SourceVisibility::Default);
  }
  {
    JSParser parser(context, "'show source'");
    auto parsed = parser.parse();
    resolveAST(context, semCtx, *parsed);
    // source visibility is correctly updated after semantic validation.
    assertFunctionLikeSourceVisibility(*parsed, SourceVisibility::ShowSource);
  }
  // Singleton function node.
  {
    JSParser parser(context, "function func (a, b) { return 10 }");
    auto parsed = parser.parse();
    resolveAST(context, semCtx, *parsed);
    assertFirstNodeAsFunctionLikeWithSourceVisibility(
        parsed, SourceVisibility::Default);
  }
  {
    JSParser parser(context, "function func (a, b) { 'sensitive'; return 10 }");
    auto parsed = parser.parse();
    resolveAST(context, semCtx, *parsed);
    assertFirstNodeAsFunctionLikeWithSourceVisibility(
        parsed, SourceVisibility::Sensitive);
  }
  {
    JSParser parser(
        context, "function func (a, b) { 'hide source'; return 10 }");
    auto parsed = parser.parse();
    resolveAST(context, semCtx, *parsed);
    assertFirstNodeAsFunctionLikeWithSourceVisibility(
        parsed, SourceVisibility::HideSource);
  }
  {
    JSParser parser(
        context, "function func (a, b) { 'show source'; return 10 }");
    auto parsed = parser.parse();
    resolveAST(context, semCtx, *parsed);
    assertFirstNodeAsFunctionLikeWithSourceVisibility(
        parsed, SourceVisibility::ShowSource);
  }
  // Visibility is correctly restored.
  {
    JSParser parser(context, "function foo(x) { 'sensitive' }function bar(){}");
    auto parsed = parser.parse();
    resolveAST(context, semCtx, *parsed);
    assertFirstNodeAsFunctionLikeWithSourceVisibility(
        parsed, SourceVisibility::Sensitive);
    assertSecondNodeAsFunctionLikeWithSourceVisibility(
        parsed, SourceVisibility::Default);
  }
  // Overriding.
  {
    // ShowSource > Default
    JSParser parser(
        context, "'show source'; function func (a, b) { return 10 }");
    auto parsed = parser.parse();
    resolveAST(context, semCtx, *parsed);
    assertSecondNodeAsFunctionLikeWithSourceVisibility(
        parsed, SourceVisibility::ShowSource);
  }
  {
    // HideSource > ShowSource
    JSParser parser(
        context,
        "'show source'; function func (a, b) { 'hide source'; return 10 }");
    auto parsed = parser.parse();
    resolveAST(context, semCtx, *parsed);
    assertSecondNodeAsFunctionLikeWithSourceVisibility(
        parsed, SourceVisibility::HideSource);
  }
  {
    // ShowSource < HideSource
    JSParser parser(
        context,
        "'hide source'; function func (a, b) { 'show source'; return 10 }");
    auto parsed = parser.parse();
    resolveAST(context, semCtx, *parsed);
    assertSecondNodeAsFunctionLikeWithSourceVisibility(
        parsed, SourceVisibility::HideSource);
  }
  {
    // Sensitive > HideSource
    JSParser parser(
        context,
        "'hide source'; function func (a, b) { 'sensitive'; return 10 }");
    auto parsed = parser.parse();
    resolveAST(context, semCtx, *parsed);
    assertSecondNodeAsFunctionLikeWithSourceVisibility(
        parsed, SourceVisibility::Sensitive);
  }
}

/// Parse \p src, run parser-mode semantic resolution on it, and \return the
/// value of mayReachImplicitReturn computed for the first FunctionDeclaration
/// in the program. Flow syntax, including 'match', is enabled when \p flow is
/// set; only the Flow tests need it.
///
/// Parser mode matters here: it is the mode the node addon and the wasm
/// parser use, and SemanticResolver skips several AST rewrites in it, so
/// CheckImplicitReturn sees shapes that never reach it during compilation.
///
/// Fails the current test if parsing or resolution fails, or if the program
/// contains no function declaration.
static bool firstFunctionMayReachImplicitReturn(
    llvh::StringRef src,
    bool flow = false) {
  Context ctx;
  if (flow) {
    ctx.setParseFlow(ParseFlowSetting::ALL);
    ctx.setParseFlowMatch(true);
  }
  sema::SemContext semCtx(ctx);
  DiagContext diag(ctx);
  JSParser parser(ctx, src);
  auto parsed = parser.parse();
  EXPECT_TRUE(parsed.hasValue());
  if (!parsed.hasValue())
    return false;

  // Must stop the helper, not just record a failure: getSemInfo() below
  // returns nullptr when resolution did not run, and dereferencing it would
  // crash instead of failing the test (the assert that would catch it is
  // compiled out under NDEBUG).
  if (!sema::resolveASTForParser(ctx, semCtx, *parsed)) {
    ADD_FAILURE() << "semantic resolution failed";
    return false;
  }
  EXPECT_EQ(0, diag.getErrCountClear());

  for (ESTree::Node &node : llvh::cast<ESTree::ProgramNode>(*parsed)->_body) {
    if (auto *func = llvh::dyn_cast<ESTree::FunctionDeclarationNode>(&node))
      return func->getSemInfo()->mayReachImplicitReturn;
  }
  ADD_FAILURE() << "no function declaration found";
  return false;
}

/// In parser mode SemanticResolver does not split "try B catch H finally F"
/// into nested try statements, so CheckImplicitReturn must handle a node that
/// carries both a handler and a finalizer. Ignoring the finalizer -- which is
/// what the code did once the assert was compiled out -- gives wrong answers
/// whenever the finalizer itself redirects control flow.
TEST(ResolverTest, TryCatchFinallyImplicitReturnTest) {
  // Plain fallthrough: nothing terminates, so the function falls off its end.
  EXPECT_TRUE(firstFunctionMayReachImplicitReturn(
      "function f() { try { g(); } catch (e) { h(); } finally { i(); } }"));
  // The try may throw and the catch completes normally.
  EXPECT_TRUE(firstFunctionMayReachImplicitReturn(
      "function f() { try { return 1; } catch (e) { h(); } finally { i(); } }"));
  // Try and catch both return and the finalizer completes normally, so
  // control never reaches the end of the function.
  EXPECT_FALSE(firstFunctionMayReachImplicitReturn(
      "function f() { try { return 1; } catch (e) { return 2; }"
      " finally { i(); } }"));
  // A 'return' in the finalizer overrides however the protected part
  // completed, so this function cannot fall through either. Reading only the
  // handler would report that it can, because the catch falls through.
  EXPECT_FALSE(firstFunctionMayReachImplicitReturn(
      "function f() { try { g(); } catch (e) { h(); } finally { return 1; } }"));
}

/// A 'break' out of the finalizer redirects control to after the labeled
/// statement, so the finalizer's target labels must be propagated even when
/// the protected part and its handler both terminate.
TEST(ResolverTest, TryCatchFinallyBreakLabelTest) {
  // Without the 'break lbl' the labeled block would definitely terminate, so
  // the statement after it -- and hence the end of the function -- would be
  // unreachable.
  EXPECT_TRUE(firstFunctionMayReachImplicitReturn(
      "function f() { lbl: { try { return 1; } catch (e) { return 2; }"
      " finally { break lbl; } } }"));
  // A finalizer that only sometimes breaks reaches both the label and its own
  // next statement, so it neither terminates nor unconditionally continues.
  EXPECT_TRUE(firstFunctionMayReachImplicitReturn(
      "function f(x) { lbl: { try { return 1; } catch (e) { return 2; }"
      " finally { if (x) break lbl; } } }"));
}

#if HERMES_PARSE_FLOW

/// A Flow 'match' statement must not crash CheckImplicitReturn, and since
/// exhaustiveness is not checked, the match must be treated as able to
/// complete normally even when every case returns.
TEST(ResolverTest, MatchStatementImplicitReturnTest) {
  // Every case returns, but the match may still match nothing.
  EXPECT_TRUE(firstFunctionMayReachImplicitReturn(
      "function f(x) { match (x) { 1 => { return 1; } _ => { return 2; } } }",
      /* flow */ true));
  // A case which completes normally obviously continues past the match.
  EXPECT_TRUE(firstFunctionMayReachImplicitReturn(
      "function f(x) { match (x) { 1 => { g(); } } }", /* flow */ true));
  // An unlabeled 'break' in a case body targets the enclosing loop, so it has
  // to be propagated out of the match statement. A do-while body always runs,
  // so the only way past the loop is that break: if the match dropped it (or
  // reported that it must terminate) the trailing 'return 1' would make the
  // function look unable to fall through, and this would be false.
  EXPECT_TRUE(firstFunctionMayReachImplicitReturn(
      "function f(x) { do { match (x) { 1 => { break; } } return 1; }"
      " while (true); }",
      /* flow */ true));
}

/// A 'break' inside a match case body targets the enclosing labeled statement,
/// so the labels targeted by the case bodies must be propagated out of the
/// match statement.
TEST(ResolverTest, MatchStatementBreakLabelTest) {
  // Without the match, 'lbl' would definitely terminate via the return, but
  // 'break lbl' makes the statement after the labeled block reachable.
  EXPECT_TRUE(firstFunctionMayReachImplicitReturn(
      "function f(x) { lbl: { match (x) { 1 => { break lbl; } }"
      " return 1; } }",
      /* flow */ true));
}

#endif // HERMES_PARSE_FLOW

} // anonymous namespace
