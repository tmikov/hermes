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

/// Helper to create a Module.
/// Note: WasmIRGen::createFunctions() creates the top-level function.
struct TestModule {
  std::shared_ptr<Context> ctx;
  Module mod;

  TestModule() : ctx(std::make_shared<Context>()), mod(ctx) {}
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

// --- i32 trapping division tests (F.2) ---

TEST(WasmIRGenTest, I32DivS) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  std::vector<WasmValType> locals;
  irgen.beginFunction(0, locals);
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onI32DivS();
  irgen.endFunction();

  // Find the CallBuiltinInst in the entry block.
  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundCallBuiltin = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *cbi = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cbi->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmI32DivS) {
          foundCallBuiltin = true;
        }
      }
    }
  }
  EXPECT_TRUE(foundCallBuiltin);
}

TEST(WasmIRGenTest, I32DivU) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  std::vector<WasmValType> locals;
  irgen.beginFunction(0, locals);
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onI32DivU();
  irgen.endFunction();

  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundCallBuiltin = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *cbi = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cbi->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmI32DivU) {
          foundCallBuiltin = true;
        }
      }
    }
  }
  EXPECT_TRUE(foundCallBuiltin);
}

TEST(WasmIRGenTest, I32RemS) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  std::vector<WasmValType> locals;
  irgen.beginFunction(0, locals);
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onI32RemS();
  irgen.endFunction();

  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundCallBuiltin = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *cbi = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cbi->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmI32RemS) {
          foundCallBuiltin = true;
        }
      }
    }
  }
  EXPECT_TRUE(foundCallBuiltin);
}

TEST(WasmIRGenTest, I32RemU) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  std::vector<WasmValType> locals;
  irgen.beginFunction(0, locals);
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onI32RemU();
  irgen.endFunction();

  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundCallBuiltin = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *cbi = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cbi->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmI32RemU) {
          foundCallBuiltin = true;
        }
      }
    }
  }
  EXPECT_TRUE(foundCallBuiltin);
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
  // Should have 2 basic blocks: entry and exit block (dead block removed).
  EXPECT_EQ(func->getBasicBlockList().size(), 2u);

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

// --- If/else tests (D.8) ---

TEST(WasmIRGenTest, IfElseWithResult) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Function: (i32) -> (i32)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (if (result i32) (local.get 0)
  //   (then (i32.const 42))
  //   (else (i32.const 99)))
  irgen.onLocalGet(0);
  irgen.onIf({WasmValType::I32});
  irgen.onI32Const(42);
  irgen.onElse();
  irgen.onI32Const(99);
  irgen.onEnd(); // end if

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // Should have a CondBranchInst for the if condition.
  bool foundCondBranch = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (llvh::isa<CondBranchInst>(&inst))
        foundCondBranch = true;
    }
  }
  EXPECT_TRUE(foundCondBranch);

  // Should have a PhiInst in the merge block with entries for 42 and 99.
  bool foundPhi42 = false;
  bool foundPhi99 = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (auto *phi = llvh::dyn_cast<PhiInst>(&inst)) {
        for (unsigned i = 0; i < phi->getNumEntries(); ++i) {
          auto pair = phi->getEntry(i);
          if (auto *lit = llvh::dyn_cast<LiteralNumber>(pair.first)) {
            if (lit->getValue() == 42.0)
              foundPhi42 = true;
            if (lit->getValue() == 99.0)
              foundPhi99 = true;
          }
        }
      }
    }
  }
  EXPECT_TRUE(foundPhi42);
  EXPECT_TRUE(foundPhi99);
}

TEST(WasmIRGenTest, IfWithoutElseVoid) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Void function: (i32) -> ()
  moduleInfo.types.push_back(WasmFuncType{{WasmValType::I32}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {WasmValType::I32});

  // (if (local.get 0)
  //   (then (i32.const 1) (local.set 1)))
  irgen.onLocalGet(0);
  irgen.onIf({});
  irgen.onI32Const(1);
  irgen.onLocalSet(1);
  irgen.onEnd(); // end if (no else)

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // Should have a CondBranchInst for the if condition.
  bool foundCondBranch = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (llvh::isa<CondBranchInst>(&inst))
        foundCondBranch = true;
    }
  }
  EXPECT_TRUE(foundCondBranch);

  // Should return undefined (void function).
  auto *ret = findReturnInst(func);
  ASSERT_NE(ret, nullptr);
  EXPECT_TRUE(llvh::isa<LiteralUndefined>(ret->getOperand(0)));
}

TEST(WasmIRGenTest, NestedIfElse) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Function: (i32, i32) -> (i32)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (if (result i32) (local.get 0)
  //   (then
  //     (if (result i32) (local.get 1)
  //       (then (i32.const 1))
  //       (else (i32.const 2))))
  //   (else (i32.const 3)))
  irgen.onLocalGet(0);
  irgen.onIf({WasmValType::I32}); // outer if
  irgen.onLocalGet(1);
  irgen.onIf({WasmValType::I32}); // inner if
  irgen.onI32Const(1);
  irgen.onElse(); // inner else
  irgen.onI32Const(2);
  irgen.onEnd(); // end inner if
  irgen.onElse(); // outer else
  irgen.onI32Const(3);
  irgen.onEnd(); // end outer if

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // Count CondBranchInsts — should have 2 (outer if + inner if).
  unsigned condBranchCount = 0;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (llvh::isa<CondBranchInst>(&inst))
        ++condBranchCount;
    }
  }
  EXPECT_EQ(condBranchCount, 2u);

  // Should have PhiInsts with values 1, 2, and 3.
  bool found1 = false, found2 = false, found3 = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (auto *phi = llvh::dyn_cast<PhiInst>(&inst)) {
        for (unsigned i = 0; i < phi->getNumEntries(); ++i) {
          auto pair = phi->getEntry(i);
          if (auto *lit = llvh::dyn_cast<LiteralNumber>(pair.first)) {
            if (lit->getValue() == 1.0)
              found1 = true;
            if (lit->getValue() == 2.0)
              found2 = true;
            if (lit->getValue() == 3.0)
              found3 = true;
          }
        }
      }
    }
  }
  EXPECT_TRUE(found1);
  EXPECT_TRUE(found2);
  EXPECT_TRUE(found3);
}

TEST(WasmIRGenTest, IfWithBr) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Function: (i32) -> (i32)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (block (result i32)
  //   (if (local.get 0)
  //     (then (i32.const 42) (br 1))
  //   )
  //   (i32.const 99))
  irgen.onBlock({WasmValType::I32}); // outer block
  irgen.onLocalGet(0);
  irgen.onIf({}); // void if
  irgen.onI32Const(42);
  irgen.onBr(1); // br to outer block
  irgen.onEnd(); // end if
  irgen.onI32Const(99); // fallthrough path
  irgen.onEnd(); // end block

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // Should have both a CondBranchInst (for if) and PhiInst values.
  bool foundCondBranch = false;
  bool foundPhi42 = false;
  bool foundPhi99 = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (llvh::isa<CondBranchInst>(&inst))
        foundCondBranch = true;
      if (auto *phi = llvh::dyn_cast<PhiInst>(&inst)) {
        for (unsigned i = 0; i < phi->getNumEntries(); ++i) {
          auto pair = phi->getEntry(i);
          if (auto *lit = llvh::dyn_cast<LiteralNumber>(pair.first)) {
            if (lit->getValue() == 42.0)
              foundPhi42 = true;
            if (lit->getValue() == 99.0)
              foundPhi99 = true;
          }
        }
      }
    }
  }
  EXPECT_TRUE(foundCondBranch);
  EXPECT_TRUE(foundPhi42);
  EXPECT_TRUE(foundPhi99);
}

// --- br_table tests (D.9) ---

TEST(WasmIRGenTest, BrTableBasic) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Function: (i32) -> ()
  moduleInfo.types.push_back(WasmFuncType{{WasmValType::I32}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (block $b0
  //   (block $b1
  //     (block $b2
  //       (local.get 0)
  //       (br_table $b0 $b1 $b2)  ;; 0->$b0, 1->$b1, default->$b2
  //     )))
  irgen.onBlock({}); // $b0 (depth 2 from innermost)
  irgen.onBlock({}); // $b1 (depth 1)
  irgen.onBlock({}); // $b2 (depth 0)
  irgen.onLocalGet(0);
  uint32_t depths[] = {2, 1};
  irgen.onBrTable(depths, 2, 0);
  irgen.onEnd(); // end $b2
  irgen.onEnd(); // end $b1
  irgen.onEnd(); // end $b0

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // Should have a SwitchInst somewhere.
  bool foundSwitch = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (llvh::isa<SwitchInst>(&inst))
        foundSwitch = true;
    }
  }
  EXPECT_TRUE(foundSwitch);
}

TEST(WasmIRGenTest, BrTableWithResult) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Function: (i32) -> (i32)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (block (result i32)
  //   (i32.const 10)
  //   (local.get 0)
  //   (br_table 0 0)   ;; case 0 and default both go to the block end
  // )
  irgen.onBlock({WasmValType::I32});
  irgen.onI32Const(10);
  irgen.onLocalGet(0);
  uint32_t depths[] = {0};
  irgen.onBrTable(depths, 1, 0);
  irgen.onEnd(); // end block

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // Should have a SwitchInst and a PhiInst with value 10.
  bool foundSwitch = false;
  bool foundPhi10 = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (llvh::isa<SwitchInst>(&inst))
        foundSwitch = true;
      if (auto *phi = llvh::dyn_cast<PhiInst>(&inst)) {
        for (unsigned i = 0; i < phi->getNumEntries(); ++i) {
          auto pair = phi->getEntry(i);
          if (auto *lit = llvh::dyn_cast<LiteralNumber>(pair.first)) {
            if (lit->getValue() == 10.0)
              foundPhi10 = true;
          }
        }
      }
    }
  }
  EXPECT_TRUE(foundSwitch);
  EXPECT_TRUE(foundPhi10);
}

TEST(WasmIRGenTest, BrTableSameDepthMerge) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Function: (i32) -> ()
  moduleInfo.types.push_back(WasmFuncType{{WasmValType::I32}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (block $b0
  //   (block $b1
  //     (local.get 0)
  //     (br_table $b0 $b0 $b1)  ;; 0->$b0, 1->$b0, default->$b1
  //   ))
  irgen.onBlock({}); // $b0 (depth 1)
  irgen.onBlock({}); // $b1 (depth 0)
  irgen.onLocalGet(0);
  uint32_t depths[] = {1, 1};
  irgen.onBrTable(depths, 2, 0);
  irgen.onEnd(); // end $b1
  irgen.onEnd(); // end $b0

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // Should have a SwitchInst. Cases 0 and 1 should share the same
  // trampoline block (same depth).
  bool foundSwitch = false;
  const SwitchInst *switchInst = nullptr;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (auto *si = llvh::dyn_cast<SwitchInst>(&inst)) {
        foundSwitch = true;
        switchInst = si;
      }
    }
  }
  ASSERT_TRUE(foundSwitch);
  ASSERT_NE(switchInst, nullptr);

  // Two cases should share the same target block.
  ASSERT_EQ(switchInst->getNumCasePair(), 2u);
  auto case0 = switchInst->getCasePair(0);
  auto case1 = switchInst->getCasePair(1);
  EXPECT_EQ(case0.second, case1.second);
}

// --- Select tests (D.10) ---

TEST(WasmIRGenTest, SelectTrueCondition) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Function: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (select (i32.const 42) (i32.const 99) (i32.const 1))
  // Stack order: val1=42, val2=99, cond=1 → result should be 42.
  irgen.onI32Const(42);
  irgen.onI32Const(99);
  irgen.onI32Const(1);
  irgen.onSelect();

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // Should have a CondBranchInst from the select.
  bool foundCondBranch = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (llvh::isa<CondBranchInst>(&inst))
        foundCondBranch = true;
    }
  }
  EXPECT_TRUE(foundCondBranch);

  // Should have a PhiInst with entries for 42 and 99.
  bool foundPhi42 = false;
  bool foundPhi99 = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (auto *phi = llvh::dyn_cast<PhiInst>(&inst)) {
        for (unsigned i = 0; i < phi->getNumEntries(); ++i) {
          auto pair = phi->getEntry(i);
          if (auto *lit = llvh::dyn_cast<LiteralNumber>(pair.first)) {
            if (lit->getValue() == 42.0)
              foundPhi42 = true;
            if (lit->getValue() == 99.0)
              foundPhi99 = true;
          }
        }
      }
    }
  }
  EXPECT_TRUE(foundPhi42);
  EXPECT_TRUE(foundPhi99);
}

TEST(WasmIRGenTest, SelectWithParams) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Function: (i32, i32, i32) -> (i32)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32, WasmValType::I32},
      {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (select (local.get 0) (local.get 1) (local.get 2))
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onLocalGet(2);
  irgen.onSelect();

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // Should have a CondBranchInst and a PhiInst.
  bool foundCondBranch = false;
  bool foundPhi = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (llvh::isa<CondBranchInst>(&inst))
        foundCondBranch = true;
      if (llvh::isa<PhiInst>(&inst))
        foundPhi = true;
    }
  }
  EXPECT_TRUE(foundCondBranch);
  EXPECT_TRUE(foundPhi);
}

// --- unreachable and nop tests (D.11) ---

TEST(WasmIRGenTest, UnreachableBasic) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Void function: () -> ()
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (unreachable)
  irgen.onUnreachable();

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // Should have an UnreachableInst in the entry block.
  bool foundUnreachable = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (llvh::isa<UnreachableInst>(&inst))
        foundUnreachable = true;
    }
  }
  EXPECT_TRUE(foundUnreachable);
}

TEST(WasmIRGenTest, UnreachableDeadCode) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Function: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (unreachable) (i32.const 99) — the const should be dead code.
  irgen.onUnreachable();
  irgen.onI32Const(99); // should be a no-op (unreachable)

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // Should have an UnreachableInst.
  bool foundUnreachable = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (llvh::isa<UnreachableInst>(&inst))
        foundUnreachable = true;
    }
  }
  EXPECT_TRUE(foundUnreachable);

  // The constant 99 should NOT appear in the IR since it's dead code.
  // The LiteralNumber 99 may exist in the module's literal pool, but
  // it should not be used by any instruction.
  bool found99InInst = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      for (unsigned i = 0; i < inst.getNumOperands(); ++i) {
        if (auto *lit = llvh::dyn_cast<LiteralNumber>(inst.getOperand(i))) {
          if (lit->getValue() == 99.0)
            found99InInst = true;
        }
      }
    }
  }
  EXPECT_FALSE(found99InInst);
}

TEST(WasmIRGenTest, NopDoesNothing) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Void function: () -> ()
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (nop)
  irgen.onNop();

  irgen.onEnd(); // function end

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // nop should not add any instructions beyond the standard control flow.
  // The function should have just: entry block with BranchInst to exit block,
  // exit block with ReturnInst.
  auto *ret = findReturnInst(func);
  ASSERT_NE(ret, nullptr);
  EXPECT_TRUE(llvh::isa<LiteralUndefined>(ret->getOperand(0)));
}

// --- Function call tests (D.12) ---

TEST(WasmIRGenTest, CallSimple) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type 0: () -> (i32) — callee returns a constant
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  // Type 1: () -> (i32) — caller
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});

  moduleInfo.functions.push_back(WasmFunction{0}); // func 0
  moduleInfo.functions.push_back(WasmFunction{1}); // func 1

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // Define function 0: return 42
  irgen.beginFunction(0, {});
  irgen.onI32Const(42);
  irgen.onEnd();
  irgen.endFunction();

  // Define function 1: call func 0, return its result
  irgen.beginFunction(1, {});
  irgen.onCall(0);
  irgen.onEnd();
  irgen.endFunction();

  // Verify function 1 has a CallInst loading function 0's closure.
  auto *caller = irgen.getIRFunctions()[1];
  bool foundCall = false;
  for (auto &bb : caller->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (auto *call = llvh::dyn_cast<CallInst>(&inst)) {
        // The callee is a LoadFrameInst loading the pre-created closure.
        auto *lfi =
            llvh::dyn_cast<LoadFrameInst>(call->getCallee());
        ASSERT_NE(lfi, nullptr);
        // The variable name should be "closure_0" for function 0.
        EXPECT_EQ(
            lfi->getLoadVariable()->getName().str(), "closure_0");
        foundCall = true;
      }
    }
  }
  EXPECT_TRUE(foundCall);
}

TEST(WasmIRGenTest, CallWithArgs) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type 0: (i32, i32) -> (i32) — add function
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  // Type 1: () -> (i32) — caller
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});

  moduleInfo.functions.push_back(WasmFunction{0}); // func 0
  moduleInfo.functions.push_back(WasmFunction{1}); // func 1

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // Define function 0: return param0 + param1
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onI32Add();
  irgen.onEnd();
  irgen.endFunction();

  // Define function 1: call func 0 with (10, 20)
  irgen.beginFunction(1, {});
  irgen.onI32Const(10);
  irgen.onI32Const(20);
  irgen.onCall(0);
  irgen.onEnd();
  irgen.endFunction();

  // Verify function 1 has a CallInst with 2 arguments.
  auto *caller = irgen.getIRFunctions()[1];
  bool foundCall = false;
  for (auto &bb : caller->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (auto *call = llvh::dyn_cast<CallInst>(&inst)) {
        // The callee is a LoadFrameInst loading the pre-created closure.
        auto *lfi =
            llvh::dyn_cast<LoadFrameInst>(call->getCallee());
        ASSERT_NE(lfi, nullptr);
        EXPECT_EQ(
            lfi->getLoadVariable()->getName().str(), "closure_0");
        // getNumArguments() includes "this" as argument 0, so for
        // 2 Wasm args: this + 2 = 3 total arguments.
        EXPECT_EQ(call->getNumArguments(), 3u);
        foundCall = true;
      }
    }
  }
  EXPECT_TRUE(foundCall);
}

TEST(WasmIRGenTest, CallVoidFunction) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type 0: () -> () — void function
  moduleInfo.types.push_back(WasmFuncType{{}, {}});

  moduleInfo.functions.push_back(WasmFunction{0}); // func 0
  moduleInfo.functions.push_back(WasmFunction{0}); // func 1 (same type)

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // Define function 0: nop
  irgen.beginFunction(0, {});
  irgen.onNop();
  irgen.onEnd();
  irgen.endFunction();

  // Define function 1: call void func 0
  irgen.beginFunction(1, {});
  irgen.onCall(0);
  irgen.onEnd();
  irgen.endFunction();

  // Verify CallInst exists and function 1 returns undefined.
  auto *caller = irgen.getIRFunctions()[1];
  bool foundCall = false;
  for (auto &bb : caller->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (llvh::isa<CallInst>(&inst))
        foundCall = true;
    }
  }
  EXPECT_TRUE(foundCall);

  auto *ret = findReturnInst(caller);
  ASSERT_NE(ret, nullptr);
  EXPECT_TRUE(llvh::isa<LiteralUndefined>(ret->getOperand(0)));
}

TEST(WasmIRGenTest, CallInUnreachable) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type 0: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});

  moduleInfo.functions.push_back(WasmFunction{0}); // func 0
  moduleInfo.functions.push_back(WasmFunction{0}); // func 1

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // Define function 0: return 1
  irgen.beginFunction(0, {});
  irgen.onI32Const(1);
  irgen.onEnd();
  irgen.endFunction();

  // Define function 1: return, then call (dead code)
  irgen.beginFunction(1, {});
  irgen.onI32Const(99);
  irgen.onReturn();
  // This call should be a no-op because we're in unreachable code.
  irgen.onCall(0);
  irgen.onEnd();
  irgen.endFunction();

  // Verify no CallInst in function 1 — the call was in unreachable code.
  auto *caller = irgen.getIRFunctions()[1];
  bool foundCall = false;
  for (auto &bb : caller->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (llvh::isa<CallInst>(&inst))
        foundCall = true;
    }
  }
  EXPECT_FALSE(foundCall);
}

// --- f64 arithmetic tests (E.1) ---

TEST(WasmIRGenTest, F64AddSubMulDiv) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::F64, WasmValType::F64}, {WasmValType::F64}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // f64.add
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF64Add();
  irgen.onDrop();

  // f64.sub
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF64Sub();
  irgen.onDrop();

  // f64.mul
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF64Mul();
  irgen.onDrop();

  // f64.div
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF64Div();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  bool foundAdd = false, foundSub = false;
  bool foundMul = false, foundDiv = false;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::BinaryAddInstKind)
      foundAdd = true;
    if (inst.getKind() == ValueKind::BinarySubtractInstKind)
      foundSub = true;
    if (inst.getKind() == ValueKind::BinaryMultiplyInstKind)
      foundMul = true;
    if (inst.getKind() == ValueKind::BinaryDivideInstKind)
      foundDiv = true;
  }
  EXPECT_TRUE(foundAdd);
  EXPECT_TRUE(foundSub);
  EXPECT_TRUE(foundMul);
  EXPECT_TRUE(foundDiv);
}

TEST(WasmIRGenTest, F64NegAbsSqrt) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::F64}, {WasmValType::F64}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // f64.neg
  irgen.onLocalGet(0);
  irgen.onF64Neg();
  irgen.onDrop();

  // f64.abs
  irgen.onLocalGet(0);
  irgen.onF64Abs();
  irgen.onDrop();

  // f64.sqrt
  irgen.onLocalGet(0);
  irgen.onF64Sqrt();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  bool foundNeg = false;
  unsigned builtinCount = 0;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::UnaryMinusInstKind)
      foundNeg = true;
    if (llvh::isa<CallBuiltinInst>(&inst))
      ++builtinCount;
  }
  EXPECT_TRUE(foundNeg);
  // abs + sqrt = 2 CallBuiltinInst
  EXPECT_EQ(builtinCount, 2u);
}

TEST(WasmIRGenTest, F64RoundingOps) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::F64}, {WasmValType::F64}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // ceil, floor, trunc, nearest — 4 CallBuiltinInst
  irgen.onLocalGet(0);
  irgen.onF64Ceil();
  irgen.onDrop();

  irgen.onLocalGet(0);
  irgen.onF64Floor();
  irgen.onDrop();

  irgen.onLocalGet(0);
  irgen.onF64Trunc();
  irgen.onDrop();

  irgen.onLocalGet(0);
  irgen.onF64Nearest();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  unsigned builtinCount = 0;
  for (auto &inst : bb) {
    if (llvh::isa<CallBuiltinInst>(&inst))
      ++builtinCount;
  }
  EXPECT_EQ(builtinCount, 4u);
}

TEST(WasmIRGenTest, F64MinMax) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::F64, WasmValType::F64}, {WasmValType::F64}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // f64.min
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF64Min();
  irgen.onDrop();

  // f64.max
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF64Max();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  unsigned builtinCount = 0;
  for (auto &inst : bb) {
    if (llvh::isa<CallBuiltinInst>(&inst))
      ++builtinCount;
  }
  // min + max = 2 CallBuiltinInst
  EXPECT_EQ(builtinCount, 2u);
}

TEST(WasmIRGenTest, F64Comparisons) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::F64, WasmValType::F64}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // f64.eq → BinaryStrictlyEqual + BinaryOr
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF64Eq();
  irgen.onDrop();

  // f64.lt → BinaryLessThan + BinaryOr
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF64Lt();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  bool foundStrictEq = false;
  bool foundLt = false;
  unsigned orCount = 0;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::BinaryStrictlyEqualInstKind)
      foundStrictEq = true;
    if (inst.getKind() == ValueKind::BinaryLessThanInstKind)
      foundLt = true;
    if (inst.getKind() == ValueKind::BinaryOrInstKind)
      ++orCount;
  }
  EXPECT_TRUE(foundStrictEq);
  EXPECT_TRUE(foundLt);
  // Two comparisons, each followed by BinaryOr → 2 BinaryOrInsts
  EXPECT_EQ(orCount, 2u);
}

TEST(WasmIRGenTest, F64PromoteF32) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // f32 → f64 promotion
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::F32}, {WasmValType::F64}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // f64.promote_f32 is a no-op (value already on stack as double)
  irgen.onLocalGet(0);
  irgen.onF64PromoteF32();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  // The only instruction in the entry block should be the setup
  // (GetParentScope, CreateScope, AllocStack, LoadParam, StoreStack,
  //  LoadStack for local.get, BranchInst to exit).
  // There should be NO arithmetic or conversion instruction.
  for (auto &inst : bb) {
    EXPECT_FALSE(llvh::isa<CallBuiltinInst>(&inst));
    EXPECT_FALSE(llvh::isa<BinaryOperatorInst>(&inst));
    EXPECT_FALSE(llvh::isa<UnaryOperatorInst>(&inst));
  }
}

// --- f32 arithmetic tests (E.2) ---

TEST(WasmIRGenTest, F32AddSubMulDiv) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::F32, WasmValType::F32}, {WasmValType::F32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // f32.add
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF32Add();
  irgen.onDrop();

  // f32.sub
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF32Sub();
  irgen.onDrop();

  // f32.mul
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF32Mul();
  irgen.onDrop();

  // f32.div
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF32Div();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  bool foundAdd = false, foundSub = false;
  bool foundMul = false, foundDiv = false;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::BinaryAddInstKind)
      foundAdd = true;
    if (inst.getKind() == ValueKind::BinarySubtractInstKind)
      foundSub = true;
    if (inst.getKind() == ValueKind::BinaryMultiplyInstKind)
      foundMul = true;
    if (inst.getKind() == ValueKind::BinaryDivideInstKind)
      foundDiv = true;
  }
  EXPECT_TRUE(foundAdd);
  EXPECT_TRUE(foundSub);
  EXPECT_TRUE(foundMul);
  EXPECT_TRUE(foundDiv);
}

TEST(WasmIRGenTest, F32NegAbsSqrt) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::F32}, {WasmValType::F32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // f32.neg
  irgen.onLocalGet(0);
  irgen.onF32Neg();
  irgen.onDrop();

  // f32.abs
  irgen.onLocalGet(0);
  irgen.onF32Abs();
  irgen.onDrop();

  // f32.sqrt
  irgen.onLocalGet(0);
  irgen.onF32Sqrt();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  bool foundNeg = false;
  unsigned builtinCount = 0;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::UnaryMinusInstKind)
      foundNeg = true;
    if (llvh::isa<CallBuiltinInst>(&inst))
      ++builtinCount;
  }
  EXPECT_TRUE(foundNeg);
  // abs + sqrt = 2 CallBuiltinInst
  EXPECT_EQ(builtinCount, 2u);
}

TEST(WasmIRGenTest, F32RoundingOps) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::F32}, {WasmValType::F32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // ceil, floor, trunc, nearest — 4 CallBuiltinInst
  irgen.onLocalGet(0);
  irgen.onF32Ceil();
  irgen.onDrop();

  irgen.onLocalGet(0);
  irgen.onF32Floor();
  irgen.onDrop();

  irgen.onLocalGet(0);
  irgen.onF32Trunc();
  irgen.onDrop();

  irgen.onLocalGet(0);
  irgen.onF32Nearest();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  unsigned builtinCount = 0;
  for (auto &inst : bb) {
    if (llvh::isa<CallBuiltinInst>(&inst))
      ++builtinCount;
  }
  EXPECT_EQ(builtinCount, 4u);
}

TEST(WasmIRGenTest, F32MinMax) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::F32, WasmValType::F32}, {WasmValType::F32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // f32.min
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF32Min();
  irgen.onDrop();

  // f32.max
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF32Max();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  unsigned builtinCount = 0;
  for (auto &inst : bb) {
    if (llvh::isa<CallBuiltinInst>(&inst))
      ++builtinCount;
  }
  // min + max = 2 CallBuiltinInst
  EXPECT_EQ(builtinCount, 2u);
}

// --- f32 comparison tests (E.3) ---

TEST(WasmIRGenTest, F32Comparisons) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::F32, WasmValType::F32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // f32.eq → BinaryStrictlyEqual + BinaryOr
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF32Eq();
  irgen.onDrop();

  // f32.ne → BinaryStrictlyNotEqual + BinaryOr
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF32Ne();
  irgen.onDrop();

  // f32.lt → BinaryLessThan + BinaryOr
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF32Lt();
  irgen.onDrop();

  // f32.gt → BinaryGreaterThan + BinaryOr
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF32Gt();
  irgen.onDrop();

  // f32.le → BinaryLessThanOrEqual + BinaryOr
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF32Le();
  irgen.onDrop();

  // f32.ge → BinaryGreaterThanOrEqual + BinaryOr
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF32Ge();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  bool foundStrictEq = false, foundStrictNe = false;
  bool foundLt = false, foundGt = false;
  bool foundLe = false, foundGe = false;
  unsigned orCount = 0;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::BinaryStrictlyEqualInstKind)
      foundStrictEq = true;
    if (inst.getKind() == ValueKind::BinaryStrictlyNotEqualInstKind)
      foundStrictNe = true;
    if (inst.getKind() == ValueKind::BinaryLessThanInstKind)
      foundLt = true;
    if (inst.getKind() == ValueKind::BinaryGreaterThanInstKind)
      foundGt = true;
    if (inst.getKind() == ValueKind::BinaryLessThanOrEqualInstKind)
      foundLe = true;
    if (inst.getKind() == ValueKind::BinaryGreaterThanOrEqualInstKind)
      foundGe = true;
    if (inst.getKind() == ValueKind::BinaryOrInstKind)
      ++orCount;
  }
  EXPECT_TRUE(foundStrictEq);
  EXPECT_TRUE(foundStrictNe);
  EXPECT_TRUE(foundLt);
  EXPECT_TRUE(foundGt);
  EXPECT_TRUE(foundLe);
  EXPECT_TRUE(foundGe);
  // 6 comparisons, each with BinaryOr → 6 BinaryOrInsts
  EXPECT_EQ(orCount, 6u);
}

TEST(WasmIRGenTest, F32DemoteF64) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // f64 → f32 demotion
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::F64}, {WasmValType::F32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // f32.demote_f64 is a no-op in Phase 1 (no rounding)
  irgen.onLocalGet(0);
  irgen.onF32DemoteF64();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  // There should be NO arithmetic, conversion, or builtin instruction.
  for (auto &inst : bb) {
    EXPECT_FALSE(llvh::isa<CallBuiltinInst>(&inst));
    EXPECT_FALSE(llvh::isa<BinaryOperatorInst>(&inst));
    EXPECT_FALSE(llvh::isa<UnaryOperatorInst>(&inst));
  }
}

// --- WasmHelpers tests (F.1) ---

TEST(WasmIRGenTest, UnreachableCallsWasmTrap) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  // Void function: () -> ()
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // (unreachable) should emit CallBuiltinInst(wasmTrap) + UnreachableInst.
  irgen.onUnreachable();

  irgen.onEnd(); // function end
  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];

  // Should have both a CallBuiltinInst (for wasmTrap) and an UnreachableInst.
  bool foundTrapCall = false;
  bool foundUnreachable = false;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (auto *cbi = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cbi->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmTrap) {
          foundTrapCall = true;
        }
      }
      if (llvh::isa<UnreachableInst>(&inst))
        foundUnreachable = true;
    }
  }
  EXPECT_TRUE(foundTrapCall);
  EXPECT_TRUE(foundUnreachable);
}

TEST(WasmIRGenTest, WasmHelpersEmitTrap) {
  // Test the WasmHelpers class directly.
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  // Use WasmHelpers directly to emit a trap call.
  // We access it indirectly through onUnreachable which calls helpers_.
  // Already tested above — this test verifies the infrastructure exists.
  irgen.onUnreachable();

  irgen.onEnd(); // function end
  irgen.endFunction();

  // Count CallBuiltinInst instructions in the function.
  auto *func = irgen.getIRFunctions()[0];
  int trapCallCount = 0;
  for (auto &bb : func->getBasicBlockList()) {
    for (auto &inst : bb) {
      if (auto *cbi = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cbi->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmTrap) {
          trapCallCount++;
        }
      }
    }
  }
  EXPECT_EQ(trapCallCount, 1);
}

// --- F.3: i32 bit manipulation ---

TEST(WasmIRGenTest, I32Clz) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  std::vector<WasmValType> locals;
  irgen.beginFunction(0, locals);
  irgen.onLocalGet(0);
  irgen.onI32Clz();
  irgen.endFunction();

  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundCallBuiltin = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *cbi = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cbi->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmI32Clz) {
          foundCallBuiltin = true;
        }
      }
    }
  }
  EXPECT_TRUE(foundCallBuiltin);
}

TEST(WasmIRGenTest, I32Ctz) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  std::vector<WasmValType> locals;
  irgen.beginFunction(0, locals);
  irgen.onLocalGet(0);
  irgen.onI32Ctz();
  irgen.endFunction();

  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundCallBuiltin = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *cbi = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cbi->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmI32Ctz) {
          foundCallBuiltin = true;
        }
      }
    }
  }
  EXPECT_TRUE(foundCallBuiltin);
}

TEST(WasmIRGenTest, I32Popcnt) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  std::vector<WasmValType> locals;
  irgen.beginFunction(0, locals);
  irgen.onLocalGet(0);
  irgen.onI32Popcnt();
  irgen.endFunction();

  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundCallBuiltin = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *cbi = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cbi->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmI32Popcnt) {
          foundCallBuiltin = true;
        }
      }
    }
  }
  EXPECT_TRUE(foundCallBuiltin);
}

TEST(WasmIRGenTest, I32RotlRotr) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // Test rotl
  {
    std::vector<WasmValType> locals;
    irgen.beginFunction(0, locals);
    irgen.onLocalGet(0);
    irgen.onLocalGet(1);
    irgen.onI32Rotl();
    irgen.endFunction();

    auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
    bool foundRotl = false;
    for (auto &bb : blocks) {
      for (auto &inst : bb) {
        if (auto *cbi = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
          if (cbi->getBuiltinIndex() ==
              BuiltinMethod::HermesBuiltin_wasmI32Rotl) {
            foundRotl = true;
          }
        }
      }
    }
    EXPECT_TRUE(foundRotl);
  }

  // Test rotr
  {
    std::vector<WasmValType> locals;
    irgen.beginFunction(1, locals);
    irgen.onLocalGet(0);
    irgen.onLocalGet(1);
    irgen.onI32Rotr();
    irgen.endFunction();

    auto &blocks = irgen.getIRFunctions()[1]->getBasicBlockList();
    bool foundRotr = false;
    for (auto &bb : blocks) {
      for (auto &inst : bb) {
        if (auto *cbi = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
          if (cbi->getBuiltinIndex() ==
              BuiltinMethod::HermesBuiltin_wasmI32Rotr) {
            foundRotr = true;
          }
        }
      }
    }
    EXPECT_TRUE(foundRotr);
  }
}

TEST(WasmIRGenTest, I32Extend8S) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  std::vector<WasmValType> locals;
  irgen.beginFunction(0, locals);
  irgen.onLocalGet(0);
  irgen.onI32Extend8S();
  irgen.endFunction();

  // Expect: (a << 24) >> 24
  // Find BinaryLeftShiftInst with operand 24, followed by
  // BinaryRightShiftInst with operand 24.
  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundLeftShift = false;
  bool foundRightShift = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *bop = llvh::dyn_cast<BinaryOperatorInst>(&inst)) {
        if (bop->getKind() == ValueKind::BinaryLeftShiftInstKind) {
          if (auto *lit = llvh::dyn_cast<LiteralNumber>(bop->getRightHandSide())) {
            if (lit->getValue() == 24)
              foundLeftShift = true;
          }
        }
        if (bop->getKind() == ValueKind::BinaryRightShiftInstKind) {
          if (auto *lit = llvh::dyn_cast<LiteralNumber>(bop->getRightHandSide())) {
            if (lit->getValue() == 24)
              foundRightShift = true;
          }
        }
      }
    }
  }
  EXPECT_TRUE(foundLeftShift);
  EXPECT_TRUE(foundRightShift);
}

TEST(WasmIRGenTest, I32Extend16S) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  std::vector<WasmValType> locals;
  irgen.beginFunction(0, locals);
  irgen.onLocalGet(0);
  irgen.onI32Extend16S();
  irgen.endFunction();

  // Expect: (a << 16) >> 16
  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundLeftShift = false;
  bool foundRightShift = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *bop = llvh::dyn_cast<BinaryOperatorInst>(&inst)) {
        if (bop->getKind() == ValueKind::BinaryLeftShiftInstKind) {
          if (auto *lit = llvh::dyn_cast<LiteralNumber>(bop->getRightHandSide())) {
            if (lit->getValue() == 16)
              foundLeftShift = true;
          }
        }
        if (bop->getKind() == ValueKind::BinaryRightShiftInstKind) {
          if (auto *lit = llvh::dyn_cast<LiteralNumber>(bop->getRightHandSide())) {
            if (lit->getValue() == 16)
              foundRightShift = true;
          }
        }
      }
    }
  }
  EXPECT_TRUE(foundLeftShift);
  EXPECT_TRUE(foundRightShift);
}

// --- F.4: Type conversion tests ---

TEST(WasmIRGenTest, I32TruncF64S) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::F64}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onI32TruncF64S();
  irgen.endFunction();

  // Should produce a CallBuiltinInst for wasmI32TruncF64S.
  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundCallBuiltin = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *cb = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cb->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmI32TruncF64S) {
          foundCallBuiltin = true;
        }
      }
    }
  }
  EXPECT_TRUE(foundCallBuiltin);
}

TEST(WasmIRGenTest, I32TruncSatF64U) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::F64}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onI32TruncSatF64U();
  irgen.endFunction();

  // Should produce a CallBuiltinInst for wasmI32TruncSatF64U.
  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundCallBuiltin = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *cb = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cb->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmI32TruncSatF64U) {
          foundCallBuiltin = true;
        }
      }
    }
  }
  EXPECT_TRUE(foundCallBuiltin);
}

TEST(WasmIRGenTest, F64ConvertI32S) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::I32}, {WasmValType::F64}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onF64ConvertI32S();
  irgen.endFunction();

  // Should produce an AsInt32Inst.
  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundAsInt32 = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (llvh::isa<AsInt32Inst>(&inst)) {
        foundAsInt32 = true;
      }
    }
  }
  EXPECT_TRUE(foundAsInt32);
}

TEST(WasmIRGenTest, F64ConvertI32U) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::I32}, {WasmValType::F64}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onF64ConvertI32U();
  irgen.endFunction();

  // Should produce an AsUint32Inst.
  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundAsUint32 = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (llvh::isa<AsUint32Inst>(&inst)) {
        foundAsUint32 = true;
      }
    }
  }
  EXPECT_TRUE(foundAsUint32);
}

TEST(WasmIRGenTest, I32ReinterpretF32) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::F32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onI32ReinterpretF32();
  irgen.endFunction();

  // Should produce a CallBuiltinInst for wasmI32ReinterpretF32.
  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundCallBuiltin = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *cb = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cb->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmI32ReinterpretF32) {
          foundCallBuiltin = true;
        }
      }
    }
  }
  EXPECT_TRUE(foundCallBuiltin);
}

TEST(WasmIRGenTest, F32ReinterpretI32) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::I32}, {WasmValType::F32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onF32ReinterpretI32();
  irgen.endFunction();

  // Should produce a CallBuiltinInst for wasmF32ReinterpretI32.
  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundCallBuiltin = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *cb = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cb->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmF32ReinterpretI32) {
          foundCallBuiltin = true;
        }
      }
    }
  }
  EXPECT_TRUE(foundCallBuiltin);
}

TEST(WasmIRGenTest, CreateFunctionsExportsObject) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type 0: (i32, i32) -> (i32)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  // Type 1: () -> ()
  moduleInfo.types.push_back(WasmFuncType{{}, {}});

  // Two defined functions.
  moduleInfo.functions.push_back(WasmFunction{0}); // func 0: exported
  moduleInfo.functions.push_back(WasmFunction{1}); // func 1: internal

  // Export only function 0 as "add".
  WasmExport exp;
  exp.name = "add";
  exp.kind = WasmExternalKind::Function;
  exp.index = 0;
  moduleInfo.exports.push_back(exp);

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // The top-level function should contain:
  //   AllocObjectLiteralInst (exports object)
  //   LoadFrameInst (load closure for exported func)
  //   StorePropertyStrictInst (store closure on exports object)
  //   ReturnInst (return the exports object, not undefined)
  auto *topLevel = tm.mod.getTopLevelFunction();
  ASSERT_NE(topLevel, nullptr);
  ASSERT_EQ(topLevel->getBasicBlockList().size(), 1u);

  auto &bb = topLevel->getBasicBlockList().front();

  AllocObjectLiteralInst *allocObj = nullptr;
  StorePropertyStrictInst *storeProp = nullptr;
  ReturnInst *ret = nullptr;
  unsigned loadFrameCount = 0;

  for (auto &inst : bb) {
    if (auto *a = llvh::dyn_cast<AllocObjectLiteralInst>(&inst))
      allocObj = a;
    if (auto *s = llvh::dyn_cast<StorePropertyStrictInst>(&inst))
      storeProp = s;
    if (llvh::isa<LoadFrameInst>(&inst))
      ++loadFrameCount;
    if (auto *r = llvh::dyn_cast<ReturnInst>(&inst))
      ret = r;
  }

  // AllocObjectLiteralInst should exist.
  ASSERT_NE(allocObj, nullptr);

  // StorePropertyStrictInst should exist (one for the one export).
  ASSERT_NE(storeProp, nullptr);
  // The store target should be the alloc'd object.
  EXPECT_EQ(storeProp->getObject(), allocObj);
  // The property name should be "add".
  auto *propLit = llvh::dyn_cast<LiteralString>(storeProp->getProperty());
  ASSERT_NE(propLit, nullptr);
  EXPECT_EQ(propLit->getValue().str(), "add");

  // There should be a LoadFrameInst for the exported closure
  // (in addition to the StoreFrameInst/CreateFunctionInst for all funcs).
  EXPECT_GE(loadFrameCount, 1u);

  // ReturnInst should return the exports object, not undefined.
  ASSERT_NE(ret, nullptr);
  EXPECT_EQ(ret->getOperand(0), allocObj);
}

TEST(WasmIRGenTest, CreateFunctionsNoExports) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // One function, no exports.
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // Even with no exports, the top-level function should return an
  // (empty) exports object, not undefined.
  auto *topLevel = tm.mod.getTopLevelFunction();
  auto &bb = topLevel->getBasicBlockList().front();

  AllocObjectLiteralInst *allocObj = nullptr;
  ReturnInst *ret = nullptr;
  unsigned storePropCount = 0;

  for (auto &inst : bb) {
    if (auto *a = llvh::dyn_cast<AllocObjectLiteralInst>(&inst))
      allocObj = a;
    if (llvh::isa<StorePropertyStrictInst>(&inst))
      ++storePropCount;
    if (auto *r = llvh::dyn_cast<ReturnInst>(&inst))
      ret = r;
  }

  ASSERT_NE(allocObj, nullptr);
  // No exports means no StorePropertyStrictInst.
  EXPECT_EQ(storePropCount, 0u);
  // ReturnInst returns the empty object.
  ASSERT_NE(ret, nullptr);
  EXPECT_EQ(ret->getOperand(0), allocObj);
}

TEST(WasmIRGenTest, CreateFunctionsSkipsNonFunctionExports) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // One function type and one function.
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  // Export the function.
  WasmExport funcExp;
  funcExp.name = "myFunc";
  funcExp.kind = WasmExternalKind::Function;
  funcExp.index = 0;
  moduleInfo.exports.push_back(funcExp);

  // Also add a memory export (should be skipped).
  WasmExport memExp;
  memExp.name = "memory";
  memExp.kind = WasmExternalKind::Memory;
  memExp.index = 0;
  moduleInfo.exports.push_back(memExp);

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  auto *topLevel = tm.mod.getTopLevelFunction();
  auto &bb = topLevel->getBasicBlockList().front();

  unsigned storePropCount = 0;
  for (auto &inst : bb) {
    if (llvh::isa<StorePropertyStrictInst>(&inst))
      ++storePropCount;
  }

  // Only the function export should produce a StorePropertyStrictInst.
  EXPECT_EQ(storePropCount, 1u);
}

} // namespace
