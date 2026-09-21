/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/IR/IRVerifier.h"
#include "hermes/AST/Context.h"
#include "hermes/IR/IR.h"
#include "hermes/IR/IRBuilder.h"
#include "hermes/IR/Instrs.h"
#include "hermes/Utils/Dumper.h"

#include "gtest/gtest.h"

using llvh::errs;

using namespace hermes;

namespace {

// The verifier is only enabled if HERMES_SLOW_DEBUG is enabled
#ifdef HERMES_SLOW_DEBUG

TEST(IRVerifierTest, BasicBlockTest) {
  auto Ctx = std::make_shared<Context>();
  Module M{Ctx};
  IRBuilder Builder(&M);
  auto F = Builder.createFunction(
      "forEach", Function::DefinitionKind::ES5Function, true);
  auto Arg1 = Builder.createJSDynamicParam(F, "num");
  auto Arg2 = Builder.createJSDynamicParam(F, "value");

  auto Entry = Builder.createBasicBlock(F);
  auto Loop = Builder.createBasicBlock(F);
  auto Body = Builder.createBasicBlock(F);
  auto Exit = Builder.createBasicBlock(F);

  Builder.setInsertionBlock(Entry);
  Builder.createBranchInst(Loop);

  Builder.setInsertionBlock(Loop);
  Builder.createCondBranchInst(Arg1, Body, Exit);

  Builder.setInsertionBlock(Body);
  Builder.createBranchInst(Loop);

  Builder.setInsertionBlock(Exit);
  Builder.createReturnInst(Arg2);

  // So far so good, this will pass
  EXPECT_TRUE(verifyModule(M));

  auto Bad = Builder.createBasicBlock(F);
  Builder.setInsertionBlock(Bad);
  Builder.createReturnInst(Arg2);

  // A dead basic block was added, and hence will fail to verify
  EXPECT_FALSE(verifyModule(M, &errs(), VerificationMode::IR_OPTIMIZED));
}

TEST(IRVerifierTest, ReturnInstTest) {
  auto Ctx = std::make_shared<Context>();
  Module M{Ctx};
  IRBuilder Builder(&M);
  auto F = Builder.createFunction(
      "testReturn", Function::DefinitionKind::ES5Function, true);
  auto Arg1 = Builder.createJSDynamicParam(F, "num");
  Arg1->setType(Type::createNumber());

  auto Body = Builder.createBasicBlock(F);
  Builder.setInsertionBlock(Body);
  Builder.createReturnInst(Arg1);

  // Everything should pass so far
  EXPECT_TRUE(verifyModule(M));

  Builder.createReturnInst(Arg1);
  // This will also fail as there are now multiple return instrs in the BB
  EXPECT_FALSE(verifyModule(M));
}

TEST(IRVerifierTest, BranchInstTest) {
  auto Ctx = std::make_shared<Context>();
  Module M{Ctx};
  IRBuilder Builder(&M);
  auto F = Builder.createFunction(
      "testBranch", Function::DefinitionKind::ES5Function, true);

  auto BB1 = Builder.createBasicBlock(F);
  auto BB2 = Builder.createBasicBlock(F);
  auto BB3 = Builder.createBasicBlock(F);

  Builder.setInsertionBlock(BB1);
  Builder.createBranchInst(BB2);

  Builder.setInsertionBlock(BB2);
  Builder.createBranchInst(BB3);

  Builder.setInsertionBlock(BB3);
  Builder.createBranchInst(BB2);

  // Everything should pass
  EXPECT_TRUE(verifyModule(M));

  Builder.createBranchInst(BB2);

  // This will fail as there are now multple branch instrs in the same BB
  EXPECT_FALSE(verifyModule(M));
}

TEST(IRVerifierTest, DominanceTest) {
  auto Ctx = std::make_shared<Context>();
  Module M{Ctx};
  IRBuilder Builder(&M);
  auto F = Builder.createFunction(
      "testBranch", Function::DefinitionKind::ES5Function, true);
  auto Arg1 = Builder.createJSDynamicParam(F, "num");

  auto Body = Builder.createBasicBlock(F);

  Builder.setInsertionBlock(Body);
  auto AsString = Builder.createAddEmptyStringInst(Arg1);
  AsString->setType(Type::createString());
  Builder.createReturnInst(AsString);

  // This tries to verify that if an instruction A is an operand of another
  // instruction B, A should dominate B.
  EXPECT_TRUE(verifyModule(M, &errs()));
}

/// \return the diagnostic verifyModule produced, having first checked that
/// it rejected \p M at all.
static std::string expectRejected(Module &M) {
  std::string out;
  llvh::raw_string_ostream OS{out};
  EXPECT_FALSE(verifyModule(M, &OS));
  OS.flush();
  return out;
}

TEST(IRVerifierTest, TryStructureTest) {
  auto Ctx = std::make_shared<Context>();
  Module M{Ctx};
  IRBuilder Builder(&M);
  auto F = Builder.createFunction(
      "testBranch", Function::DefinitionKind::ES5Function, true);

  auto entry = Builder.createBasicBlock(F);
  // This BB will be reachable from both outside of a try and inside of a try.
  auto illegalBB = Builder.createBasicBlock(F);
  auto tryStartBB = Builder.createBasicBlock(F);
  auto tryBodyBB = Builder.createBasicBlock(F);
  auto catchBB = Builder.createBasicBlock(F);

  Builder.setInsertionBlock(entry);
  // Here we reach illegalBB from outside a try.
  Builder.createCondBranchInst(
      Builder.getLiteralBool(true), tryStartBB, illegalBB);

  Builder.setInsertionBlock(tryStartBB);
  Builder.createTryStartInst(tryBodyBB, catchBB);

  // Here we reach illegalBB from inside a try.
  Builder.setInsertionBlock(tryBodyBB);
  Builder.createBranchInst(illegalBB);

  // The CatchInst is load-bearing for what this test reaches. Without it the
  // verifier stops at "Catch Target of TryStartInst must begin with a
  // CatchInst", which is checked before the successors are walked, and the
  // conflicting-try check below is never reached at all.
  Builder.setInsertionBlock(catchBB);
  Builder.createCatchInst();
  Builder.createReturnInst(Builder.getLiteralUndefined());

  Builder.setInsertionBlock(illegalBB);
  Builder.createReturnInst(Builder.getLiteralUndefined());

  // One of the two enclosing tries here is "none": illegalBB is reachable
  // from the function entry, which is inside no try at all. Naming it must
  // not require an instruction to point at.
  EXPECT_NE(
      expectRejected(M).find(
          "is reachable from multiple different TryStartInsts: %2 and none"),
      std::string::npos);
}

TEST(IRVerifierTest, TryStructureNoTryReportedFirstTest) {
  auto Ctx = std::make_shared<Context>();
  Module M{Ctx};
  IRBuilder Builder(&M);
  auto F = Builder.createFunction(
      "testBranchMirrored", Function::DefinitionKind::ES5Function, true);

  auto entry = Builder.createBasicBlock(F);
  auto tryBodyBB = Builder.createBasicBlock(F);
  auto catchBB = Builder.createBasicBlock(F);
  auto illegalBB = Builder.createBasicBlock(F);

  Builder.setInsertionBlock(entry);
  Builder.createTryStartInst(tryBodyBB, catchBB);

  // The try body is walked before the catch target, so illegalBB is recorded
  // as being inside the try first and reached from outside it second. That
  // puts the null on the OTHER side of the message from TryStructureTest,
  // which is the side a fix that repaired only one of the two would miss.
  Builder.setInsertionBlock(tryBodyBB);
  Builder.createBranchInst(illegalBB);

  Builder.setInsertionBlock(catchBB);
  Builder.createCatchInst();
  Builder.createBranchInst(illegalBB);

  // Not a ReturnInst: illegalBB is visited while still inside the try, and a
  // return there is rejected first, before the successors are walked.
  Builder.setInsertionBlock(illegalBB);
  Builder.createUnreachableInst();

  EXPECT_NE(
      expectRejected(M).find(
          "is reachable from multiple different TryStartInsts: none and %0"),
      std::string::npos);
}

TEST(IRVerifierTest, TryStructureTwoDifferentTriesTest) {
  auto Ctx = std::make_shared<Context>();
  Module M{Ctx};
  IRBuilder Builder(&M);
  auto F = Builder.createFunction(
      "testTwoTries", Function::DefinitionKind::ES5Function, true);

  auto entry = Builder.createBasicBlock(F);
  auto tryStart1 = Builder.createBasicBlock(F);
  auto tryBody1 = Builder.createBasicBlock(F);
  auto catch1 = Builder.createBasicBlock(F);
  auto tryStart2 = Builder.createBasicBlock(F);
  auto tryBody2 = Builder.createBasicBlock(F);
  auto catch2 = Builder.createBasicBlock(F);
  auto illegalBB = Builder.createBasicBlock(F);

  Builder.setInsertionBlock(entry);
  Builder.createCondBranchInst(
      Builder.getLiteralBool(true), tryStart1, tryStart2);

  Builder.setInsertionBlock(tryStart1);
  Builder.createTryStartInst(tryBody1, catch1);
  Builder.setInsertionBlock(tryBody1);
  Builder.createBranchInst(illegalBB);
  Builder.setInsertionBlock(catch1);
  Builder.createCatchInst();
  Builder.createUnreachableInst();

  Builder.setInsertionBlock(tryStart2);
  Builder.createTryStartInst(tryBody2, catch2);
  Builder.setInsertionBlock(tryBody2);
  Builder.createBranchInst(illegalBB);
  Builder.setInsertionBlock(catch2);
  Builder.createCatchInst();
  Builder.createUnreachableInst();

  Builder.setInsertionBlock(illegalBB);
  Builder.createUnreachableInst();

  // Neither side is null here: the check is about the tries DIFFERING, not
  // about one of them being absent. A comparison that only asked whether
  // both sides were inside some try would accept this and still pass the two
  // tests above. Both labels are named, so the message has to identify each
  // try rather than only the first.
  EXPECT_NE(
      expectRejected(M).find(
          "is reachable from multiple different TryStartInsts: %1 and %5"),
      std::string::npos);
}

TEST(IRVerifierTest, TryStructureSameTryMergeTest) {
  auto Ctx = std::make_shared<Context>();
  Module M{Ctx};
  IRBuilder Builder(&M);
  auto F = Builder.createFunction(
      "testSameTryMerge", Function::DefinitionKind::ES5Function, true);

  auto entry = Builder.createBasicBlock(F);
  auto tryBody = Builder.createBasicBlock(F);
  auto arm1 = Builder.createBasicBlock(F);
  auto arm2 = Builder.createBasicBlock(F);
  auto merge = Builder.createBasicBlock(F);
  auto after = Builder.createBasicBlock(F);
  auto catchBB = Builder.createBasicBlock(F);

  Builder.setInsertionBlock(entry);
  Builder.createTryStartInst(tryBody, catchBB);

  // Two paths through the try body rejoin. Both reach merge with the SAME
  // enclosing try, which is legal and common -- any `if` inside a `try` is
  // this shape.
  Builder.setInsertionBlock(tryBody);
  Builder.createCondBranchInst(Builder.getLiteralBool(true), arm1, arm2);
  Builder.setInsertionBlock(arm1);
  Builder.createBranchInst(merge);
  Builder.setInsertionBlock(arm2);
  Builder.createBranchInst(merge);

  Builder.setInsertionBlock(merge);
  Builder.createTryEndInst(catchBB, after);
  Builder.setInsertionBlock(after);
  Builder.createReturnInst(Builder.getLiteralUndefined());

  Builder.setInsertionBlock(catchBB);
  Builder.createCatchInst();
  Builder.createReturnInst(Builder.getLiteralUndefined());

  // The positive direction, which the rejecting tests cannot supply: a check
  // that fired on any repeated visit, or only when both sides were null,
  // would refuse this.
  EXPECT_TRUE(verifyModule(M, &errs()));
}

TEST(IRVerifierTest, CatchTargetMustStartWithCatchTest) {
  auto Ctx = std::make_shared<Context>();
  Module M{Ctx};
  IRBuilder Builder(&M);
  auto F = Builder.createFunction(
      "testCatchTarget", Function::DefinitionKind::ES5Function, true);

  auto entry = Builder.createBasicBlock(F);
  auto tryBodyBB = Builder.createBasicBlock(F);
  auto catchBB = Builder.createBasicBlock(F);

  Builder.setInsertionBlock(entry);
  Builder.createTryStartInst(tryBodyBB, catchBB);

  Builder.setInsertionBlock(tryBodyBB);
  Builder.createTryEndInst(catchBB, catchBB);

  // No CatchInst, which is what this rejects.
  Builder.setInsertionBlock(catchBB);
  Builder.createReturnInst(Builder.getLiteralUndefined());

  EXPECT_NE(
      expectRejected(M).find(
          "Catch Target of TryStartInst must begin with a CatchInst"),
      std::string::npos);
}

#endif // HERMES_SLOW_DEBUG

} // end anonymous namespace
