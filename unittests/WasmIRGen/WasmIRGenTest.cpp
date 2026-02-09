/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/WasmIRGen/WasmIRGen.h"

#include "hermes/AST/Context.h"
#include "hermes/IR/IR.h"
#include "hermes/IR/IRBuilder.h"
#include "hermes/IR/Instrs.h"
#include "hermes/WasmFrontend/WasmModuleInfo.h"
#include "hermes/WasmFrontend/WasmTypes.h"

#include "gtest/gtest.h"

using namespace hermes;
using namespace hermes::wasm;

namespace {

/// Helper to create a Module with a top-level function.
struct TestModule {
  std::shared_ptr<Context> ctx;
  Module mod;

  TestModule() : ctx(std::make_shared<Context>()), mod(ctx) {
    IRBuilder builder(&mod);
    builder.createTopLevelFunction("global", true);
  }
};

// --- createFunctions tests ---

TEST(WasmIRGenTest, CreateFunctionsSingleNoParams) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // One function type: () -> ()
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  // One defined function using type 0.
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  auto funcs = irgen.getIRFunctions();
  ASSERT_EQ(funcs.size(), 1u);
  ASSERT_NE(funcs[0], nullptr);

  // The function should have "this" param only (no Wasm params).
  // getJSDynamicParams() includes "this" at index 0.
  EXPECT_EQ(funcs[0]->getJSDynamicParams().size(), 1u);

  // The function should have one basic block.
  EXPECT_EQ(funcs[0]->getBasicBlockList().size(), 1u);

  // The basic block should end with a ReturnInst.
  auto &bb = funcs[0]->getBasicBlockList().front();
  ASSERT_FALSE(bb.empty());
  EXPECT_TRUE(llvh::isa<ReturnInst>(&bb.back()));
}

TEST(WasmIRGenTest, CreateFunctionsSingleWithParams) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // One function type: (i32, i32) -> (i32)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32},
      {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  auto funcs = irgen.getIRFunctions();
  ASSERT_EQ(funcs.size(), 1u);

  // "this" + 2 Wasm params = 3 JSDynamicParams.
  EXPECT_EQ(funcs[0]->getJSDynamicParams().size(), 3u);
}

TEST(WasmIRGenTest, CreateFunctionsMultiple) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type 0: () -> ()
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  // Type 1: (i32) -> (i32)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  // Type 2: (i32, f64, i32) -> (f64)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::F64, WasmValType::I32},
      {WasmValType::F64}});

  moduleInfo.functions.push_back(WasmFunction{0});
  moduleInfo.functions.push_back(WasmFunction{1});
  moduleInfo.functions.push_back(WasmFunction{2});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  auto funcs = irgen.getIRFunctions();
  ASSERT_EQ(funcs.size(), 3u);

  // Function 0: () -> () — "this" only.
  EXPECT_EQ(funcs[0]->getJSDynamicParams().size(), 1u);
  // Function 1: (i32) -> (i32) — "this" + 1.
  EXPECT_EQ(funcs[1]->getJSDynamicParams().size(), 2u);
  // Function 2: (i32, f64, i32) -> (f64) — "this" + 3.
  EXPECT_EQ(funcs[2]->getJSDynamicParams().size(), 4u);
}

TEST(WasmIRGenTest, CreateFunctionsWithImports) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type 0: (i32) -> (i32)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  // Type 1: (i32, i32) -> (i32)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});

  // One imported function (type 0).
  WasmImport imp;
  imp.moduleName = "env";
  imp.fieldName = "log";
  imp.kind = WasmExternalKind::Function;
  imp.typeIndex = 0;
  moduleInfo.imports.push_back(imp);

  // One defined function (type 1).
  moduleInfo.functions.push_back(WasmFunction{1});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  auto funcs = irgen.getIRFunctions();
  // 1 imported + 1 defined = 2 total.
  ASSERT_EQ(funcs.size(), 2u);

  // Imported function 0: (i32) -> (i32) — "this" + 1.
  EXPECT_EQ(funcs[0]->getJSDynamicParams().size(), 2u);
  // Defined function 1: (i32, i32) -> (i32) — "this" + 2.
  EXPECT_EQ(funcs[1]->getJSDynamicParams().size(), 3u);
}

TEST(WasmIRGenTest, CreateFunctionsWithNames) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type 0: () -> ()
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});
  moduleInfo.functions.push_back(WasmFunction{0});

  // Set names for the functions.
  moduleInfo.names.functionNames = {"myFunc", ""};

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  auto funcs = irgen.getIRFunctions();
  ASSERT_EQ(funcs.size(), 2u);

  // First function should use the name from the name section.
  EXPECT_EQ(funcs[0]->getOriginalOrInferredName().str(), "myFunc");
  // Second function has empty name, should get a generated name.
  EXPECT_EQ(funcs[1]->getOriginalOrInferredName().str(), "wasm_func_1");
}

// --- beginFunction / endFunction tests ---

TEST(WasmIRGenTest, BeginEndFunctionNoLocals) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: (i32) -> (i32)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // Begin function with no extra locals.
  irgen.beginFunction(0, {});

  // Push a constant and end (implicit return).
  irgen.onI32Const(42);
  irgen.endFunction();

  // The function should have blocks: entry + exit (from implicit func block).
  auto *func = irgen.getIRFunctions()[0];
  EXPECT_GE(func->getBasicBlockList().size(), 2u);
  // The exit block (last block) should end with ReturnInst.
  auto &exitBB = func->getBasicBlockList().back();
  EXPECT_FALSE(exitBB.empty());
  EXPECT_TRUE(llvh::isa<ReturnInst>(&exitBB.back()));
}

TEST(WasmIRGenTest, BeginFunctionWithLocals) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: (i32) -> (i32)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // Begin with 2 extra i32 locals.
  irgen.beginFunction(0, {WasmValType::I32, WasmValType::I32});

  // Push a constant and end.
  irgen.onI32Const(0);
  irgen.endFunction();

  // The function should have AllocStackInst instructions:
  // 1 for the parameter + 2 for locals = 3.
  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();
  unsigned allocCount = 0;
  for (auto &inst : bb) {
    if (llvh::isa<AllocStackInst>(&inst))
      ++allocCount;
  }
  EXPECT_EQ(allocCount, 3u);
}

// --- Instruction callback tests ---

TEST(WasmIRGenTest, LocalGetSet) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: (i32) -> (i32)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // Begin: 1 param + 1 extra local.
  irgen.beginFunction(0, {WasmValType::I32});

  // local.get 0 (the parameter)
  irgen.onLocalGet(0);
  // local.set 1 (store into the extra local)
  irgen.onLocalSet(1);
  // local.get 1 (read back)
  irgen.onLocalGet(1);

  irgen.endFunction();

  // Verify instructions were generated: there should be LoadStackInst
  // and StoreStackInst instructions.
  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();
  unsigned loadCount = 0;
  unsigned storeCount = 0;
  for (auto &inst : bb) {
    if (llvh::isa<LoadStackInst>(&inst))
      ++loadCount;
    if (llvh::isa<StoreStackInst>(&inst))
      ++storeCount;
  }
  // StoreStack: 1 (param init) + 1 (local init) + 1 (local.set 1) = 3
  // LoadStack: 1 (local.get 0) + 1 (local.get 1) = 2
  EXPECT_EQ(loadCount, 2u);
  EXPECT_EQ(storeCount, 3u);
}

TEST(WasmIRGenTest, LocalTee) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: (i32) -> (i32)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  irgen.beginFunction(0, {});

  // i32.const 42
  irgen.onI32Const(42);
  // local.tee 0 — stores and keeps the value on stack.
  irgen.onLocalTee(0);

  irgen.endFunction();

  // The function should return 42 (the tee'd value).
  // The return is in the exit block (last block).
  auto *func = irgen.getIRFunctions()[0];
  auto &exitBB = func->getBasicBlockList().back();
  ASSERT_TRUE(llvh::isa<ReturnInst>(&exitBB.back()));
  auto *ret = llvh::cast<ReturnInst>(&exitBB.back());
  // The return operand should be a PhiInst (from the implicit function block).
  EXPECT_TRUE(llvh::isa<PhiInst>(ret->getOperand(0)));
}

TEST(WasmIRGenTest, VoidFunctionReturn) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: () -> () (void function)
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  irgen.beginFunction(0, {});
  irgen.endFunction();

  // Should return undefined. The return is in the exit block.
  auto *func = irgen.getIRFunctions()[0];
  auto &exitBB = func->getBasicBlockList().back();
  ASSERT_TRUE(llvh::isa<ReturnInst>(&exitBB.back()));
  auto *ret = llvh::cast<ReturnInst>(&exitBB.back());
  EXPECT_TRUE(llvh::isa<LiteralUndefined>(ret->getOperand(0)));
}

// --- i32 arithmetic tests (D.3) ---

TEST(WasmIRGenTest, I32Add) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onI32Add();
  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  // Find BinaryAddInst and AsInt32Inst.
  bool foundAdd = false;
  bool foundAsInt32 = false;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::BinaryAddInstKind)
      foundAdd = true;
    if (llvh::isa<AsInt32Inst>(&inst))
      foundAsInt32 = true;
  }
  EXPECT_TRUE(foundAdd);
  EXPECT_TRUE(foundAsInt32);
}

TEST(WasmIRGenTest, I32Mul) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onI32Mul();
  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  // Find CallBuiltinInst (Math.imul).
  bool foundCallBuiltin = false;
  for (auto &inst : bb) {
    if (llvh::isa<CallBuiltinInst>(&inst))
      foundCallBuiltin = true;
  }
  EXPECT_TRUE(foundCallBuiltin);
}

TEST(WasmIRGenTest, I32BitwiseOps) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // Test i32.and
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onI32And();

  // Test i32.or
  irgen.onLocalGet(0);
  irgen.onI32Or();

  // Test i32.xor
  irgen.onLocalGet(0);
  irgen.onI32Xor();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  bool foundAnd = false;
  bool foundOr = false;
  bool foundXor = false;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::BinaryAndInstKind)
      foundAnd = true;
    if (inst.getKind() == ValueKind::BinaryOrInstKind)
      foundOr = true;
    if (inst.getKind() == ValueKind::BinaryXorInstKind)
      foundXor = true;
  }
  EXPECT_TRUE(foundAnd);
  EXPECT_TRUE(foundOr);
  EXPECT_TRUE(foundXor);
}

TEST(WasmIRGenTest, I32ShiftOps) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // Test i32.shl
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onI32Shl();

  // Test i32.shr_s
  irgen.onLocalGet(0);
  irgen.onI32ShrS();

  // Test i32.shr_u
  irgen.onLocalGet(0);
  irgen.onI32ShrU();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  bool foundShl = false;
  bool foundShrS = false;
  bool foundShrU = false;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::BinaryLeftShiftInstKind)
      foundShl = true;
    if (inst.getKind() == ValueKind::BinaryRightShiftInstKind)
      foundShrS = true;
    if (inst.getKind() == ValueKind::BinaryUnsignedRightShiftInstKind)
      foundShrU = true;
  }
  EXPECT_TRUE(foundShl);
  EXPECT_TRUE(foundShrS);
  EXPECT_TRUE(foundShrU);
}

// --- i32 comparison tests (D.4) ---

TEST(WasmIRGenTest, I32EqNe) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onI32Eq();
  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  // Should have BinaryStrictlyEqualInst followed by BinaryOrInst.
  bool foundStrictlyEqual = false;
  bool foundBitOr = false;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::BinaryStrictlyEqualInstKind)
      foundStrictlyEqual = true;
    if (inst.getKind() == ValueKind::BinaryOrInstKind)
      foundBitOr = true;
  }
  EXPECT_TRUE(foundStrictlyEqual);
  EXPECT_TRUE(foundBitOr);
}

TEST(WasmIRGenTest, I32SignedComparisons) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onI32LtS();
  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  // Should have AsInt32Inst (x2), BinaryLessThanInst, BinaryOrInst.
  unsigned asInt32Count = 0;
  bool foundLessThan = false;
  bool foundBitOr = false;
  for (auto &inst : bb) {
    if (llvh::isa<AsInt32Inst>(&inst))
      ++asInt32Count;
    if (inst.getKind() == ValueKind::BinaryLessThanInstKind)
      foundLessThan = true;
    if (inst.getKind() == ValueKind::BinaryOrInstKind)
      foundBitOr = true;
  }
  EXPECT_EQ(asInt32Count, 2u);
  EXPECT_TRUE(foundLessThan);
  EXPECT_TRUE(foundBitOr);
}

TEST(WasmIRGenTest, I32UnsignedComparisons) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onI32LtU();
  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  // Should have AsUint32Inst (x2), BinaryLessThanInst, BinaryOrInst.
  unsigned asUint32Count = 0;
  bool foundLessThan = false;
  bool foundBitOr = false;
  for (auto &inst : bb) {
    if (llvh::isa<AsUint32Inst>(&inst))
      ++asUint32Count;
    if (inst.getKind() == ValueKind::BinaryLessThanInstKind)
      foundLessThan = true;
    if (inst.getKind() == ValueKind::BinaryOrInstKind)
      foundBitOr = true;
  }
  EXPECT_EQ(asUint32Count, 2u);
  EXPECT_TRUE(foundLessThan);
  EXPECT_TRUE(foundBitOr);
}

TEST(WasmIRGenTest, I32Eqz) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onI32Eqz();
  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  // Should have BinaryStrictlyEqualInst(val, 0) and BinaryOrInst.
  bool foundStrictlyEqual = false;
  bool foundBitOr = false;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::BinaryStrictlyEqualInstKind)
      foundStrictlyEqual = true;
    if (inst.getKind() == ValueKind::BinaryOrInstKind)
      foundBitOr = true;
  }
  EXPECT_TRUE(foundStrictlyEqual);
  EXPECT_TRUE(foundBitOr);
}

// --- Return and drop tests (D.5) ---

TEST(WasmIRGenTest, ExplicitReturn) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // i32.const 42; return
  irgen.onI32Const(42);
  irgen.onReturn();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  // Should have 3 basic blocks: entry, dead block after return, exit block.
  EXPECT_EQ(func->getBasicBlockList().size(), 3u);

  // Entry block should end with ReturnInst returning 42.
  auto &bb = func->getBasicBlockList().front();
  ASSERT_TRUE(llvh::isa<ReturnInst>(&bb.back()));
  auto *ret = llvh::cast<ReturnInst>(&bb.back());
  auto *lit = llvh::dyn_cast<LiteralNumber>(ret->getOperand(0));
  ASSERT_NE(lit, nullptr);
  EXPECT_EQ(lit->getValue(), 42.0);
}

TEST(WasmIRGenTest, ExplicitReturnVoid) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Void function: () -> ()
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // return (no value)
  irgen.onReturn();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  // Entry block should end with ReturnInst returning undefined.
  auto &bb = func->getBasicBlockList().front();
  ASSERT_TRUE(llvh::isa<ReturnInst>(&bb.back()));
  auto *ret = llvh::cast<ReturnInst>(&bb.back());
  EXPECT_TRUE(llvh::isa<LiteralUndefined>(ret->getOperand(0)));
}

TEST(WasmIRGenTest, Drop) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Void function: () -> ()
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // i32.const 42; drop
  irgen.onI32Const(42);
  irgen.onDrop();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  // Should return undefined (void function, value was dropped).
  // The return is in the exit block (last block).
  auto &exitBB = func->getBasicBlockList().back();
  ASSERT_TRUE(llvh::isa<ReturnInst>(&exitBB.back()));
  auto *ret = llvh::cast<ReturnInst>(&exitBB.back());
  EXPECT_TRUE(llvh::isa<LiteralUndefined>(ret->getOperand(0)));
}

// --- Block/br/br_if tests (D.6) ---

/// Helper: find the first ReturnInst in a function.
static ReturnInst *findReturnInst(Function *func) {
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (auto *ret = llvh::dyn_cast<ReturnInst>(&inst))
        return ret;
    }
  }
  return nullptr;
}

TEST(WasmIRGenTest, BlockWithResult) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Function: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (block (result i32) (i32.const 42) (end))
  irgen.onBlock({WasmValType::I32});
  irgen.onI32Const(42);
  irgen.onEnd(); // end of block
  // At this point, the block result (42) should be on the value stack.
  irgen.onEnd(); // end of function

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  // Find the ReturnInst.
  auto *ret = findReturnInst(func);
  ASSERT_NE(ret, nullptr);

  // There should be a PhiInst feeding the return (from the function exit).
  EXPECT_TRUE(llvh::isa<PhiInst>(ret->getOperand(0)));
}

TEST(WasmIRGenTest, BlockBrWithResult) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Function: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (block (result i32) (i32.const 42) (br 0) (end))
  irgen.onBlock({WasmValType::I32});
  irgen.onI32Const(42);
  irgen.onBr(0);
  irgen.onEnd();

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // Entry block should have a BranchInst (from the br 0).
  auto &entryBB = func->getBasicBlockList().front();
  EXPECT_TRUE(llvh::isa<BranchInst>(&entryBB.back()));

  // There should be a PhiInst in the block's continuation block
  // that receives 42 from the br.
  bool foundPhi = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (auto *phi = llvh::dyn_cast<PhiInst>(&inst)) {
        if (phi->getNumEntries() > 0) {
          auto pair = phi->getEntry(0);
          if (auto *lit = llvh::dyn_cast<LiteralNumber>(pair.first)) {
            if (lit->getValue() == 42.0)
              foundPhi = true;
          }
        }
      }
    }
  }
  EXPECT_TRUE(foundPhi);
}

TEST(WasmIRGenTest, BlockVoid) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Void function: () -> ()
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (block (nop) (end))
  irgen.onBlock({});
  // No operations inside the block.
  irgen.onEnd();

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  // Should have multiple basic blocks (entry, block cont, exit).
  EXPECT_GE(func->getBasicBlockList().size(), 3u);

  // Should return undefined.
  auto *ret = findReturnInst(func);
  ASSERT_NE(ret, nullptr);
  EXPECT_TRUE(llvh::isa<LiteralUndefined>(ret->getOperand(0)));
}

TEST(WasmIRGenTest, BrIfTaken) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Void function: () -> ()
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (block (i32.const 1) (br_if 0) (end))
  irgen.onBlock({});
  irgen.onI32Const(1);
  irgen.onBrIf(0);
  irgen.onEnd();

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  // Should have a CondBranchInst somewhere.
  bool foundCondBranch = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (llvh::isa<CondBranchInst>(&inst))
        foundCondBranch = true;
    }
  }
  EXPECT_TRUE(foundCondBranch);
}

TEST(WasmIRGenTest, NestedBlocksBr) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Function: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (block (result i32)
  //   (block
  //     (i32.const 55)
  //     (br 1)
  //   )
  //   (i32.const 0)
  // )
  irgen.onBlock({WasmValType::I32}); // outer block
  irgen.onBlock({});                  // inner block
  irgen.onI32Const(55);
  irgen.onBr(1); // br to outer block (depth 1)
  irgen.onEnd(); // end inner block
  irgen.onI32Const(0); // unreachable code
  irgen.onEnd(); // end outer block

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // The function should have a PhiInst receiving 55 from the br 1.
  bool foundPhi55 = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (auto *phi = llvh::dyn_cast<PhiInst>(&inst)) {
        if (phi->getNumEntries() > 0) {
          auto pair = phi->getEntry(0);
          if (auto *lit = llvh::dyn_cast<LiteralNumber>(pair.first)) {
            if (lit->getValue() == 55.0)
              foundPhi55 = true;
          }
        }
      }
    }
  }
  EXPECT_TRUE(foundPhi55);
}

// --- Loop tests (D.7) ---

TEST(WasmIRGenTest, LoopFallthrough) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Void function: () -> ()
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (loop (nop) (end))
  irgen.onLoop({});
  // No operations inside the loop — just falls through.
  irgen.onEnd();

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  // Should have: entry, exit(func), loop header, loop end = 4 blocks min.
  EXPECT_GE(func->getBasicBlockList().size(), 4u);

  // Should return undefined.
  auto *ret = findReturnInst(func);
  ASSERT_NE(ret, nullptr);
  EXPECT_TRUE(llvh::isa<LiteralUndefined>(ret->getOperand(0)));
}

TEST(WasmIRGenTest, LoopBrBack) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Void function: () -> ()
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (loop (br 0) (end))
  // The br 0 targets the loop header (infinite loop).
  irgen.onLoop({});
  irgen.onBr(0); // br to loop header
  irgen.onEnd();

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  // The loop header should have a BranchInst targeting itself.
  // Find a block with a BranchInst whose target is the block itself
  // or another block that is the loop header.
  bool foundLoopBack = false;
  for (auto &bb : func->getBasicBlockList()) {
    if (bb.empty())
      continue;
    if (auto *br = llvh::dyn_cast<BranchInst>(&bb.back())) {
      // Check if we find a BranchInst targeting a block that has
      // incoming branches from the loop body.
      (void)br;
    }
  }
  // Just verify it compiles and doesn't crash.
  // The end block is unreachable (since br 0 always loops).
  // The function should still have a return instruction somewhere.
  auto *ret = findReturnInst(func);
  ASSERT_NE(ret, nullptr);
  (void)foundLoopBack;
}

TEST(WasmIRGenTest, LoopWithBrIf) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Function: (i32) -> ()
  moduleInfo.types.push_back(WasmFuncType{{WasmValType::I32}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (loop (local.get 0) (br_if 0) (end))
  // Conditional loop back: loop while param 0 is non-zero.
  irgen.onLoop({});
  irgen.onLocalGet(0);
  irgen.onBrIf(0); // br_if to loop header
  irgen.onEnd();

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  // Should have a CondBranchInst targeting the loop header.
  bool foundCondBranch = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (llvh::isa<CondBranchInst>(&inst))
        foundCondBranch = true;
    }
  }
  EXPECT_TRUE(foundCondBranch);
}

TEST(WasmIRGenTest, LoopWithResult) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Function: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (loop (result i32) (i32.const 42) (end))
  // Loop with result type — the result is the fallthrough value.
  irgen.onLoop({WasmValType::I32});
  irgen.onI32Const(42);
  irgen.onEnd(); // loop end — 42 falls through as the result

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  // The loop's end block should have a PhiInst for the result.
  bool foundPhi42 = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (auto *phi = llvh::dyn_cast<PhiInst>(&inst)) {
        if (phi->getNumEntries() > 0) {
          auto pair = phi->getEntry(0);
          if (auto *lit = llvh::dyn_cast<LiteralNumber>(pair.first)) {
            if (lit->getValue() == 42.0)
              foundPhi42 = true;
          }
        }
      }
    }
  }
  EXPECT_TRUE(foundPhi42);
}

} // namespace
