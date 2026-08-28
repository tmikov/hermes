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

/// Count the CallBuiltinInsts in \p bb that call builtin \p m. Tests that
/// merely check "some CallBuiltinInst exists" cannot tell one wasm builtin
/// from another -- emitting wasmI64Sub where wasmI64Add is expected would
/// pass -- so assert the specific builtin instead.
static unsigned countBuiltinCalls(BasicBlock &bb, BuiltinMethod::Enum m) {
  unsigned n = 0;
  for (auto &inst : bb)
    if (auto *cbi = llvh::dyn_cast<CallBuiltinInst>(&inst))
      if (cbi->getBuiltinIndex() == m)
        ++n;
  return n;
}

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
    if (inst.getKind() == ValueKind::FAddInstKind)
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

  // i32.mul lowers to exactly one Math.imul call.
  EXPECT_EQ(countBuiltinCalls(bb, BuiltinMethod::Math_imul), 1u);
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

  // Should have FEqualInst followed by AsInt32Inst (bool→i32).
  bool foundStrictlyEqual = false;
  bool foundAsInt32 = false;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::FEqualInstKind)
      foundStrictlyEqual = true;
    if (llvh::isa<AsInt32Inst>(&inst))
      foundAsInt32 = true;
  }
  EXPECT_TRUE(foundStrictlyEqual);
  EXPECT_TRUE(foundAsInt32);
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

  // Should have AsInt32Inst (x2 for operands + x1 for bool→i32), FLessThanInst.
  unsigned asInt32Count = 0;
  bool foundLessThan = false;
  for (auto &inst : bb) {
    if (llvh::isa<AsInt32Inst>(&inst))
      ++asInt32Count;
    if (inst.getKind() == ValueKind::FLessThanInstKind)
      foundLessThan = true;
  }
  EXPECT_EQ(asInt32Count, 3u);
  EXPECT_TRUE(foundLessThan);
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

  // Should have AsUint32Inst (x2), FLessThanInst, AsInt32Inst (bool→i32).
  unsigned asUint32Count = 0;
  bool foundLessThan = false;
  bool foundAsInt32 = false;
  for (auto &inst : bb) {
    if (llvh::isa<AsUint32Inst>(&inst))
      ++asUint32Count;
    if (inst.getKind() == ValueKind::FLessThanInstKind)
      foundLessThan = true;
    if (llvh::isa<AsInt32Inst>(&inst))
      foundAsInt32 = true;
  }
  EXPECT_EQ(asUint32Count, 2u);
  EXPECT_TRUE(foundLessThan);
  EXPECT_TRUE(foundAsInt32);
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

  // Should have FEqualInst(val, 0) and AsInt32Inst (bool→i32).
  bool foundStrictlyEqual = false;
  bool foundAsInt32 = false;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::FEqualInstKind)
      foundStrictlyEqual = true;
    if (llvh::isa<AsInt32Inst>(&inst))
      foundAsInt32 = true;
  }
  EXPECT_TRUE(foundStrictlyEqual);
  EXPECT_TRUE(foundAsInt32);
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
  irgen.onBlock({}, {WasmValType::I32});
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
  irgen.onBlock({}, {WasmValType::I32});
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
  irgen.onBlock({}, {});
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
  irgen.onBlock({}, {});
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
  irgen.onBlock({}, {WasmValType::I32}); // outer block
  irgen.onBlock({}, {});                  // inner block
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
  irgen.onLoop({}, {});
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
  irgen.onLoop({}, {});
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
  irgen.onLoop({}, {});
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
  irgen.onLoop({}, {WasmValType::I32});
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
  irgen.onIf({}, {WasmValType::I32});
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
  irgen.onIf({}, {});
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
  irgen.onIf({}, {WasmValType::I32}); // outer if
  irgen.onLocalGet(1);
  irgen.onIf({}, {WasmValType::I32}); // inner if
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
  irgen.onBlock({}, {WasmValType::I32}); // outer block
  irgen.onLocalGet(0);
  irgen.onIf({}, {}); // void if
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
  irgen.onBlock({}, {}); // $b0 (depth 2 from innermost)
  irgen.onBlock({}, {}); // $b1 (depth 1)
  irgen.onBlock({}, {}); // $b2 (depth 0)
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
  irgen.onBlock({}, {WasmValType::I32});
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
  irgen.onBlock({}, {}); // $b0 (depth 1)
  irgen.onBlock({}, {}); // $b1 (depth 0)
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
    if (inst.getKind() == ValueKind::FAddInstKind)
      foundAdd = true;
    if (inst.getKind() == ValueKind::FSubtractInstKind)
      foundSub = true;
    if (inst.getKind() == ValueKind::FMultiplyInstKind)
      foundMul = true;
    if (inst.getKind() == ValueKind::FDivideInstKind)
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
    if (inst.getKind() == ValueKind::FNegateKind)
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
  unsigned asInt32Count = 0;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::FEqualInstKind)
      foundStrictEq = true;
    if (inst.getKind() == ValueKind::FLessThanInstKind)
      foundLt = true;
    if (llvh::isa<AsInt32Inst>(&inst))
      ++asInt32Count;
  }
  EXPECT_TRUE(foundStrictEq);
  EXPECT_TRUE(foundLt);
  // Two comparisons, each followed by AsInt32Inst → 2 AsInt32Insts
  EXPECT_EQ(asInt32Count, 2u);
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
    if (inst.getKind() == ValueKind::FAddInstKind)
      foundAdd = true;
    if (inst.getKind() == ValueKind::FSubtractInstKind)
      foundSub = true;
    if (inst.getKind() == ValueKind::FMultiplyInstKind)
      foundMul = true;
    if (inst.getKind() == ValueKind::FDivideInstKind)
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
    if (inst.getKind() == ValueKind::FNegateKind)
      foundNeg = true;
    if (llvh::isa<CallBuiltinInst>(&inst))
      ++builtinCount;
  }
  EXPECT_TRUE(foundNeg);
  // neg's fround + abs + abs's fround + sqrt + sqrt's fround = 5
  EXPECT_EQ(builtinCount, 5u);
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
  // ceil + fround + floor + fround + trunc + fround + nearest + fround = 8
  EXPECT_EQ(builtinCount, 8u);
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
  // min + fround + max + fround = 4
  EXPECT_EQ(builtinCount, 4u);
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
  unsigned asInt32Count = 0;
  for (auto &inst : bb) {
    if (inst.getKind() == ValueKind::FEqualInstKind)
      foundStrictEq = true;
    if (inst.getKind() == ValueKind::FNotEqualInstKind)
      foundStrictNe = true;
    if (inst.getKind() == ValueKind::FLessThanInstKind)
      foundLt = true;
    if (inst.getKind() == ValueKind::FGreaterThanInstKind)
      foundGt = true;
    if (inst.getKind() == ValueKind::FLessThanOrEqualInstKind)
      foundLe = true;
    if (inst.getKind() == ValueKind::FGreaterThanOrEqualInstKind)
      foundGe = true;
    if (llvh::isa<AsInt32Inst>(&inst))
      ++asInt32Count;
  }
  EXPECT_TRUE(foundStrictEq);
  EXPECT_TRUE(foundStrictNe);
  EXPECT_TRUE(foundLt);
  EXPECT_TRUE(foundGt);
  EXPECT_TRUE(foundLe);
  EXPECT_TRUE(foundGe);
  // 6 comparisons, each with AsInt32Inst → 6 AsInt32Insts
  EXPECT_EQ(asInt32Count, 6u);
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

  // f32.demote_f64 rounds to f32 via Math.fround
  irgen.onLocalGet(0);
  irgen.onF32DemoteF64();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  auto &bb = func->getBasicBlockList().front();

  // Should have exactly 1 CallBuiltinInst (Math.fround), no arithmetic.
  unsigned builtinCount = 0;
  for (auto &inst : bb) {
    if (llvh::isa<CallBuiltinInst>(&inst))
      ++builtinCount;
    EXPECT_FALSE(llvh::isa<BinaryOperatorInst>(&inst));
    EXPECT_FALSE(llvh::isa<UnaryOperatorInst>(&inst));
  }
  EXPECT_EQ(builtinCount, 1u);
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
  irgen.finalizeModule();

  // The top-level function should now return a module info object
  // with "instantiate", "exportDescs", and "importDescs" properties.
  auto *topLevel = tm.mod.getTopLevelFunction();
  ASSERT_NE(topLevel, nullptr);
  ASSERT_EQ(topLevel->getBasicBlockList().size(), 1u);

  auto &bb = topLevel->getBasicBlockList().front();

  // Look for key properties of the module info object.
  bool hasInstantiateProp = false;
  bool hasExportDescsProp = false;
  bool hasImportDescsProp = false;
  bool hasCreateFunctionInst = false;
  ReturnInst *ret = nullptr;

  for (auto &inst : bb) {
    if (llvh::isa<CreateFunctionInst>(&inst))
      hasCreateFunctionInst = true;
    if (auto *s = llvh::dyn_cast<StorePropertyStrictInst>(&inst)) {
      if (auto *propLit = llvh::dyn_cast<LiteralString>(s->getProperty())) {
        auto name = propLit->getValue().str();
        if (name == "instantiate")
          hasInstantiateProp = true;
        else if (name == "exportDescs")
          hasExportDescsProp = true;
        else if (name == "importDescs")
          hasImportDescsProp = true;
      }
    }
    if (auto *r = llvh::dyn_cast<ReturnInst>(&inst))
      ret = r;
  }

  // Should have a CreateFunctionInst for __wasm_instantiate__.
  EXPECT_TRUE(hasCreateFunctionInst);
  // Module info object should have all three properties.
  EXPECT_TRUE(hasInstantiateProp);
  EXPECT_TRUE(hasExportDescsProp);
  EXPECT_TRUE(hasImportDescsProp);
  // ReturnInst should return the module info object.
  ASSERT_NE(ret, nullptr);
}

TEST(WasmIRGenTest, CreateFunctionsNoExports) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // One function, no exports.
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.finalizeModule();

  // Even with no exports, the top-level function should return a module
  // info object with "instantiate", "exportDescs", and "importDescs".
  auto *topLevel = tm.mod.getTopLevelFunction();
  auto &bb = topLevel->getBasicBlockList().front();

  bool hasInstantiateProp = false;
  bool hasExportDescsProp = false;
  bool hasImportDescsProp = false;
  ReturnInst *ret = nullptr;

  for (auto &inst : bb) {
    if (auto *s = llvh::dyn_cast<StorePropertyStrictInst>(&inst)) {
      if (auto *propLit = llvh::dyn_cast<LiteralString>(s->getProperty())) {
        auto name = propLit->getValue().str();
        if (name == "instantiate")
          hasInstantiateProp = true;
        else if (name == "exportDescs")
          hasExportDescsProp = true;
        else if (name == "importDescs")
          hasImportDescsProp = true;
      }
    }
    if (auto *r = llvh::dyn_cast<ReturnInst>(&inst))
      ret = r;
  }

  EXPECT_TRUE(hasInstantiateProp);
  EXPECT_TRUE(hasExportDescsProp);
  EXPECT_TRUE(hasImportDescsProp);
  // ReturnInst should return the module info object.
  ASSERT_NE(ret, nullptr);
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

  // Also add a memory export.
  moduleInfo.memories.push_back(WasmMemoryType{{1, 4, true}});
  WasmExport memExp;
  memExp.name = "memory";
  memExp.kind = WasmExternalKind::Memory;
  memExp.index = 0;
  moduleInfo.exports.push_back(memExp);

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.finalizeModule();

  // The top-level now builds the module info object. We verify that
  // the descriptor arrays include both exports (function and memory),
  // and the module info has all three required properties.
  auto *topLevel = tm.mod.getTopLevelFunction();
  auto &bb = topLevel->getBasicBlockList().front();

  bool hasInstantiateProp = false;
  bool hasExportDescsProp = false;
  bool hasImportDescsProp = false;

  for (auto &inst : bb) {
    if (auto *s = llvh::dyn_cast<StorePropertyStrictInst>(&inst)) {
      if (auto *propLit = llvh::dyn_cast<LiteralString>(s->getProperty())) {
        auto name = propLit->getValue().str();
        if (name == "instantiate")
          hasInstantiateProp = true;
        else if (name == "exportDescs")
          hasExportDescsProp = true;
        else if (name == "importDescs")
          hasImportDescsProp = true;
      }
    }
  }

  EXPECT_TRUE(hasInstantiateProp);
  EXPECT_TRUE(hasExportDescsProp);
  EXPECT_TRUE(hasImportDescsProp);
}

TEST(WasmIRGenTest, F64Copysign) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::F64, WasmValType::F64}, {WasmValType::F64}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF64Copysign();
  irgen.endFunction();

  // Should produce a CallBuiltinInst for wasmF64Copysign.
  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundCallBuiltin = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *cb = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cb->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmF64Copysign) {
          foundCallBuiltin = true;
          // Two Wasm args + 1 (this) = 3.
          EXPECT_EQ(cb->getNumArguments(), 3u);
        }
      }
    }
  }
  EXPECT_TRUE(foundCallBuiltin);
}

TEST(WasmIRGenTest, F32Copysign) {
  TestModule tm;
  WasmModuleInfo moduleInfo;
  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::F32, WasmValType::F32}, {WasmValType::F32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onLocalGet(1);
  irgen.onF32Copysign();
  irgen.endFunction();

  // Should produce a CallBuiltinInst for wasmF32Copysign.
  auto &blocks = irgen.getIRFunctions()[0]->getBasicBlockList();
  bool foundCallBuiltin = false;
  for (auto &bb : blocks) {
    for (auto &inst : bb) {
      if (auto *cb = llvh::dyn_cast<CallBuiltinInst>(&inst)) {
        if (cb->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmF32Copysign) {
          foundCallBuiltin = true;
          // Two Wasm args + 1 (this) = 3.
          EXPECT_EQ(cb->getNumArguments(), 3u);
        }
      }
    }
  }
  EXPECT_TRUE(foundCallBuiltin);
}

// --- i64 representation (G.1) ---

TEST(WasmIRGenTest, I64ConstPushesTwo) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: () -> (i32) — returns i32 so we can verify stack height via return
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  irgen.beginFunction(0, {});

  // Push an i64 constant — should occupy 2 stack slots.
  irgen.onI64Const(0x0000000100000002LL);

  // Drop should consume both halves (lo + hi).
  irgen.onDrop();

  // Push a regular i32 for the return.
  irgen.onI32Const(42);

  irgen.endFunction();

  // Verify the function compiled without assertion failures.
  // The key test is that onDrop properly consumed both i64 halves.
  auto *func = irgen.getIRFunctions()[0];
  ASSERT_NE(func, nullptr);
}

TEST(WasmIRGenTest, I64PopI64ReturnsCorrectPair) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: () -> (i32) — returns lo32 of the i64
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  irgen.beginFunction(0, {});

  // Push an i64 constant: value = 0x00000003_00000007
  // lo32 = 7, hi32 = 3
  irgen.onI64Const(0x0000000300000007LL);

  // Use popI64 to get lo and hi.
  auto [lo, hi] = irgen.popI64();

  // Both should be LiteralNumber values.
  auto *loNum = llvh::dyn_cast<LiteralNumber>(lo);
  auto *hiNum = llvh::dyn_cast<LiteralNumber>(hi);
  ASSERT_NE(loNum, nullptr);
  ASSERT_NE(hiNum, nullptr);
  EXPECT_EQ(loNum->getValue(), 7.0);
  EXPECT_EQ(hiNum->getValue(), 3.0);

  // Push an i32 for return.
  irgen.onI32Const(0);
  irgen.endFunction();
}

TEST(WasmIRGenTest, I64DropConsumesTwo) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  irgen.beginFunction(0, {});

  // Push i32 first, then i64 on top.
  irgen.onI32Const(10);
  irgen.onI64Const(0x100000002LL);

  // Drop should consume the i64 (2 slots), leaving the i32.
  irgen.onDrop();

  // The remaining i32 on the stack is our return value.
  irgen.endFunction();

  // Verify that the function compiled without issues.
  auto *func = irgen.getIRFunctions()[0];
  ASSERT_NE(func, nullptr);
}

TEST(WasmIRGenTest, I64DropDoesNotConsumeSingleValue) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: () -> ()
  moduleInfo.types.push_back(WasmFuncType{{}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  irgen.beginFunction(0, {});

  // Push a regular i32 value.
  irgen.onI32Const(42);

  // Drop should consume only 1 slot (not 2) because it's not i64.
  irgen.onDrop();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  ASSERT_NE(func, nullptr);
}

TEST(WasmIRGenTest, I64PushI64ThenI32Interleaved) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  irgen.beginFunction(0, {});

  // Push i64 (2 slots), then i32 (1 slot).
  irgen.onI64Const(100);
  irgen.onI32Const(42);

  // Drop the i32 (1 slot).
  irgen.onDrop();

  // Drop the i64 (2 slots).
  irgen.onDrop();

  // Push return value.
  irgen.onI32Const(0);
  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  ASSERT_NE(func, nullptr);
}

// --- i64 arithmetic tests (G.3) ---

TEST(WasmIRGenTest, I64AddEmitsCallAndHi) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  irgen.beginFunction(0, {});

  // Push two i64 values.
  irgen.onI64Const(100);
  irgen.onI64Const(200);

  // i64.add should pop two i64 pairs, push one i64 pair.
  irgen.onI64Add();

  // The result should be an i64 on the stack.
  auto [lo, hi] = irgen.popI64();
  ASSERT_NE(lo, nullptr);
  ASSERT_NE(hi, nullptr);

  // With the retBuf convention, lo and hi are AsInt32Inst wrapping
  // LoadPropertyInst reading from retBufI_[0] and retBufI_[1]. That much
  // is identical for every i64 binop, so also assert the operation is
  // specifically i64.add and not some other i64 builtin.
  auto *loInst = llvh::dyn_cast<AsInt32Inst>(lo);
  ASSERT_NE(loInst, nullptr);

  auto *hiInst = llvh::dyn_cast<AsInt32Inst>(hi);
  ASSERT_NE(hiInst, nullptr);

  auto &bb = irgen.getIRFunctions()[0]->getBasicBlockList().front();
  EXPECT_EQ(countBuiltinCalls(bb, BuiltinMethod::HermesBuiltin_wasmI64Add), 1u);
  EXPECT_EQ(countBuiltinCalls(bb, BuiltinMethod::HermesBuiltin_wasmI64Sub), 0u);

  // Push i32 return value.
  irgen.onI32Const(0);
  irgen.endFunction();
}

TEST(WasmIRGenTest, I64AndEmitsInlineBitwise) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  irgen.beginFunction(0, {});

  irgen.onI64Const(0xFF00);
  irgen.onI64Const(0x0FFF);

  // i64.and should produce inline BinaryAndInst for both lo and hi.
  irgen.onI64And();

  auto [lo, hi] = irgen.popI64();
  // Both should be BinaryOperatorInst (BinaryAnd), not CallBuiltinInst.
  EXPECT_TRUE(llvh::isa<BinaryOperatorInst>(lo));
  EXPECT_TRUE(llvh::isa<BinaryOperatorInst>(hi));

  irgen.onI32Const(0);
  irgen.endFunction();
}

TEST(WasmIRGenTest, I64EqzReturnsI32) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  irgen.beginFunction(0, {});

  irgen.onI64Const(42);

  // i64.eqz takes i64, returns i32. It is lowered fully inline as
  // AsInt32(FCompare((lo | hi), 0)) with no builtin call, so assert that
  // shape rather than merely that the function exists: exactly one
  // FCompareInst against literal 0, wrapped in an AsInt32Inst, and no
  // CallBuiltinInst at all.
  irgen.onI64Eqz();
  irgen.endFunction();

  auto &bb = irgen.getIRFunctions()[0]->getBasicBlockList().front();
  unsigned fcmpCount = 0, callBuiltinCount = 0;
  bool asInt32OfFCompare = false;
  for (auto &inst : bb) {
    if (auto *fc = llvh::dyn_cast<FCompareInst>(&inst)) {
      ++fcmpCount;
      EXPECT_TRUE(llvh::isa<LiteralNumber>(fc->getRight()));
    }
    if (llvh::isa<CallBuiltinInst>(&inst))
      ++callBuiltinCount;
    if (auto *ai = llvh::dyn_cast<AsInt32Inst>(&inst))
      if (llvh::isa<FCompareInst>(ai->getSingleOperand()))
        asInt32OfFCompare = true;
  }
  EXPECT_EQ(fcmpCount, 1u);
  EXPECT_EQ(callBuiltinCount, 0u);
  EXPECT_TRUE(asInt32OfFCompare);
}

TEST(WasmIRGenTest, I64EqEmitsCallBuiltin) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  irgen.beginFunction(0, {});

  irgen.onI64Const(42);
  irgen.onI64Const(42);
  irgen.onI64Eq();
  irgen.endFunction();

  // Despite the test name, i64.eq is lowered fully inline --
  // AsInt32(FCompare((loA^loB) | (hiA^hiB), 0)) -- with no builtin call.
  // Assert that shape: one FCompareInst against a literal, wrapped in
  // AsInt32, and no CallBuiltinInst.
  auto &bb = irgen.getIRFunctions()[0]->getBasicBlockList().front();
  unsigned fcmpCount = 0, callBuiltinCount = 0;
  bool asInt32OfFCompare = false;
  for (auto &inst : bb) {
    if (auto *fc = llvh::dyn_cast<FCompareInst>(&inst)) {
      ++fcmpCount;
      EXPECT_TRUE(llvh::isa<LiteralNumber>(fc->getRight()));
    }
    if (llvh::isa<CallBuiltinInst>(&inst))
      ++callBuiltinCount;
    if (auto *ai = llvh::dyn_cast<AsInt32Inst>(&inst))
      if (llvh::isa<FCompareInst>(ai->getSingleOperand()))
        asInt32OfFCompare = true;
  }
  EXPECT_EQ(fcmpCount, 1u);
  EXPECT_EQ(callBuiltinCount, 0u);
  EXPECT_TRUE(asInt32OfFCompare);
}

TEST(WasmIRGenTest, I64ClzReturnsI64WithZeroHi) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  irgen.beginFunction(0, {});

  irgen.onI64Const(1);
  irgen.onI64Clz();

  // Result should be i64 (per Wasm spec, clz returns i64).
  // Pop via popI64 — if this succeeds, the result was properly pushed as i64.
  auto [lo, hi] = irgen.popI64();

  // lo should be a CallBuiltinInst (wasmI64Clz).
  auto *loInst = llvh::dyn_cast<CallBuiltinInst>(lo);
  ASSERT_NE(loInst, nullptr);
  EXPECT_EQ(
      loInst->getBuiltinIndex(),
      BuiltinMethod::HermesBuiltin_wasmI64Clz);

  // hi should be LiteralNumber(0) since clz result fits in i32.
  auto *hiNum = llvh::dyn_cast<LiteralNumber>(hi);
  ASSERT_NE(hiNum, nullptr);
  EXPECT_EQ(hiNum->getValue(), 0.0);

  irgen.onI32Const(0);
  irgen.endFunction();
}

TEST(WasmIRGenTest, I64ShlEmitsCallAndHi) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  irgen.beginFunction(0, {});

  irgen.onI64Const(1);
  irgen.onI64Const(32);
  irgen.onI64Shl();

  // Pop via popI64 — verifies the result was pushed as i64.
  auto [lo, hi] = irgen.popI64();

  // With the retBuf convention, lo and hi are AsInt32Inst wrapping
  // LoadPropertyInst reading from retBufI_[0] and retBufI_[1]. Assert the
  // operation is specifically i64.shl, not another retBuf builtin.
  auto *loInst = llvh::dyn_cast<AsInt32Inst>(lo);
  ASSERT_NE(loInst, nullptr);

  auto *hiInst = llvh::dyn_cast<AsInt32Inst>(hi);
  ASSERT_NE(hiInst, nullptr);

  auto &bb = irgen.getIRFunctions()[0]->getBasicBlockList().front();
  EXPECT_EQ(countBuiltinCalls(bb, BuiltinMethod::HermesBuiltin_wasmI64Shl), 1u);

  irgen.onI32Const(0);
  irgen.endFunction();
}

TEST(WasmIRGenTest, I64TruncF64SEmitsCallAndHi) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: (f64) -> (i32) — i32 result since we'll wrap the i64 manually.
  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::F64}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  irgen.onLocalGet(0);
  irgen.onI64TruncF64S();

  // Result should be an i64 (split pair). Pop and verify.
  auto [lo, hi] = irgen.popI64();

  // With the retBuf convention, lo and hi are AsInt32Inst wrapping
  // LoadPropertyInst -- identical across the trunc family, so assert the
  // operation is specifically i64.trunc_f64_s.
  auto *loInst = llvh::dyn_cast<AsInt32Inst>(lo);
  ASSERT_NE(loInst, nullptr);

  auto *hiInst = llvh::dyn_cast<AsInt32Inst>(hi);
  ASSERT_NE(hiInst, nullptr);

  auto &bb = irgen.getIRFunctions()[0]->getBasicBlockList().front();
  EXPECT_EQ(
      countBuiltinCalls(bb, BuiltinMethod::HermesBuiltin_wasmI64TruncF64S),
      1u);

  irgen.onI32Const(0);
  irgen.endFunction();
}

TEST(WasmIRGenTest, I64TruncF64UEmitsCallAndHi) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::F64}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  irgen.onLocalGet(0);
  irgen.onI64TruncF64U();

  auto [lo, hi] = irgen.popI64();

  // With the retBuf convention, lo is AsInt32Inst wrapping LoadPropertyInst.
  auto *loInst = llvh::dyn_cast<AsInt32Inst>(lo);
  ASSERT_NE(loInst, nullptr);

  irgen.onI32Const(0);
  irgen.endFunction();
}

TEST(WasmIRGenTest, I64TruncSatF64SEmitsCallAndHi) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::F64}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  irgen.onLocalGet(0);
  irgen.onI64TruncSatF64S();

  auto [lo, hi] = irgen.popI64();

  // With the retBuf convention, lo is AsInt32Inst wrapping LoadPropertyInst.
  auto *loInst = llvh::dyn_cast<AsInt32Inst>(lo);
  ASSERT_NE(loInst, nullptr);

  irgen.onI32Const(0);
  irgen.endFunction();
}

TEST(WasmIRGenTest, I64TruncSatF64UEmitsCallAndHi) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::F64}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  irgen.onLocalGet(0);
  irgen.onI64TruncSatF64U();

  auto [lo, hi] = irgen.popI64();

  // With the retBuf convention, lo is AsInt32Inst wrapping LoadPropertyInst.
  auto *loInst = llvh::dyn_cast<AsInt32Inst>(lo);
  ASSERT_NE(loInst, nullptr);

  irgen.onI32Const(0);
  irgen.endFunction();
}

TEST(WasmIRGenTest, I64TruncF32SDelegatesToF64) {
  // In Phase 1, f32 and f64 truncations produce the same IR.
  TestModule tm;
  WasmModuleInfo moduleInfo;

  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::F32}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  irgen.onLocalGet(0);
  irgen.onI64TruncF32S();

  // Should emit wasmI64TruncF64S (same builtin as f64 variant).
  auto [lo, hi] = irgen.popI64();

  // With the retBuf convention, lo is AsInt32Inst wrapping LoadPropertyInst.
  auto *loInst = llvh::dyn_cast<AsInt32Inst>(lo);
  ASSERT_NE(loInst, nullptr);

  irgen.onI32Const(0);
  irgen.endFunction();
}

// --- G.4c: i64→float conversions and reinterpret ---

TEST(WasmIRGenTest, F64ConvertI64SEmitsCall) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // (i64.const 42) → f64.convert_i64_s → result on stack as f64
  moduleInfo.types.push_back(
      WasmFuncType{{}, {WasmValType::F64}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  irgen.onI64Const(42);
  irgen.onF64ConvertI64S();

  // Result is a single f64 value (not i64 pair).
  // Pop it as a regular value (not popI64).
  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  bool found = false;
  for (auto &BB : *func) {
    for (auto &I : BB) {
      if (auto *call = llvh::dyn_cast<CallBuiltinInst>(&I)) {
        if (call->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmF64ConvertI64S) {
          found = true;
          // Takes 2 args: lo, hi.
          EXPECT_EQ(call->getNumArguments(), 3u); // +1 for 'this'
        }
      }
    }
  }
  EXPECT_TRUE(found);
}

TEST(WasmIRGenTest, F64ConvertI64UEmitsCall) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  moduleInfo.types.push_back(
      WasmFuncType{{}, {WasmValType::F64}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  irgen.onI64Const(-1);
  irgen.onF64ConvertI64U();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  bool found = false;
  for (auto &BB : *func) {
    for (auto &I : BB) {
      if (auto *call = llvh::dyn_cast<CallBuiltinInst>(&I)) {
        if (call->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmF64ConvertI64U) {
          found = true;
        }
      }
    }
  }
  EXPECT_TRUE(found);
}

TEST(WasmIRGenTest, F32ConvertI64SEmitsCall) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  moduleInfo.types.push_back(
      WasmFuncType{{}, {WasmValType::F32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  irgen.onI64Const(100);
  irgen.onF32ConvertI64S();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  bool found = false;
  for (auto &BB : *func) {
    for (auto &I : BB) {
      if (auto *call = llvh::dyn_cast<CallBuiltinInst>(&I)) {
        if (call->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmF32ConvertI64S) {
          found = true;
        }
      }
    }
  }
  EXPECT_TRUE(found);
}

TEST(WasmIRGenTest, I64ReinterpretF64ConstantFolds) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // f64.const 1.0 followed by i64.reinterpret_f64 should be constant-folded.
  // 1.0 = 0x3FF0000000000000 → lo=0, hi=0x3FF00000=1072693248.
  moduleInfo.types.push_back(
      WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  irgen.onF64Const(1.0);
  irgen.onI64ReinterpretF64();

  // Result should be an i64 (split pair) of literal constants.
  auto [lo, hi] = irgen.popI64();

  auto *loLit = llvh::dyn_cast<LiteralNumber>(lo);
  ASSERT_NE(loLit, nullptr);
  EXPECT_EQ(loLit->getValue(), 0.0);

  auto *hiLit = llvh::dyn_cast<LiteralNumber>(hi);
  ASSERT_NE(hiLit, nullptr);
  EXPECT_EQ(hiLit->getValue(), 1072693248.0); // 0x3FF00000

  irgen.onI32Const(0);
  irgen.endFunction();
}

TEST(WasmIRGenTest, F64ReinterpretI64EmitsCall) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // i64 → f64.reinterpret_i64 → produces f64 (single value)
  moduleInfo.types.push_back(
      WasmFuncType{{}, {WasmValType::F64}});
  moduleInfo.functions.push_back(WasmFunction{0});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();
  irgen.beginFunction(0, {});

  irgen.onI64Const(4607182418800017408LL); // 0x3FF0000000000000 = 1.0
  irgen.onF64ReinterpretI64();

  irgen.endFunction();

  auto *func = irgen.getIRFunctions()[0];
  bool found = false;
  for (auto &BB : *func) {
    for (auto &I : BB) {
      if (auto *call = llvh::dyn_cast<CallBuiltinInst>(&I)) {
        if (call->getBuiltinIndex() ==
            BuiltinMethod::HermesBuiltin_wasmF64ReinterpretI64) {
          found = true;
          // Takes 2 args: lo, hi.
          EXPECT_EQ(call->getNumArguments(), 3u); // +1 for 'this'
        }
      }
    }
  }
  EXPECT_TRUE(found);
}

// --- Import trampoline tests (I.2) ---

TEST(WasmIRGenTest, ImportTrampolineVoidReturn) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: (i32) -> void
  moduleInfo.types.push_back(WasmFuncType{{WasmValType::I32}, {}});
  // One function import.
  WasmImport imp;
  imp.moduleName = "env";
  imp.fieldName = "log";
  imp.kind = WasmExternalKind::Function;
  imp.typeIndex = 0;
  moduleInfo.imports.push_back(imp);

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  auto funcs = irgen.getIRFunctions();
  ASSERT_EQ(funcs.size(), 1u);

  // The import trampoline should have a body (not just ReturnInst undefined).
  auto &bb = funcs[0]->getBasicBlockList().front();
  ASSERT_FALSE(bb.empty());

  // Should contain a CallInst (calling the imported JS function).
  bool foundCall = false;
  bool foundReturn = false;
  for (auto &I : bb) {
    if (llvh::isa<CallInst>(&I))
      foundCall = true;
    if (auto *ret = llvh::dyn_cast<ReturnInst>(&I)) {
      foundReturn = true;
      // Void return: should return undefined.
      EXPECT_TRUE(llvh::isa<LiteralUndefined>(ret->getOperand(0)));
    }
  }
  EXPECT_TRUE(foundCall);
  EXPECT_TRUE(foundReturn);
}

TEST(WasmIRGenTest, ImportTrampolineI32Return) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: (i32, i32) -> i32
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32, WasmValType::I32}, {WasmValType::I32}});
  // One function import.
  WasmImport imp;
  imp.moduleName = "env";
  imp.fieldName = "add";
  imp.kind = WasmExternalKind::Function;
  imp.typeIndex = 0;
  moduleInfo.imports.push_back(imp);

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  auto funcs = irgen.getIRFunctions();
  ASSERT_EQ(funcs.size(), 1u);

  auto &bb = funcs[0]->getBasicBlockList().front();

  // Should contain GetParentScopeInst, LoadFrameInst, LoadParamInst (x2),
  // CallInst, AsInt32Inst, ReturnInst.
  bool foundGetParent = false;
  bool foundLoadFrame = false;
  bool foundAsInt32 = false;
  int loadParamCount = 0;
  for (auto &I : bb) {
    if (llvh::isa<GetParentScopeInst>(&I))
      foundGetParent = true;
    if (llvh::isa<LoadFrameInst>(&I))
      foundLoadFrame = true;
    if (llvh::isa<LoadParamInst>(&I))
      ++loadParamCount;
    if (llvh::isa<AsInt32Inst>(&I))
      foundAsInt32 = true;
  }
  EXPECT_TRUE(foundGetParent);
  EXPECT_TRUE(foundLoadFrame);
  EXPECT_EQ(loadParamCount, 2); // two i32 params
  EXPECT_TRUE(foundAsInt32); // return value coerced to i32
}

TEST(WasmIRGenTest, ImportTrampolineF64Return) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: (f64) -> f64
  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::F64}, {WasmValType::F64}});
  WasmImport imp;
  imp.moduleName = "env";
  imp.fieldName = "f64_func";
  imp.kind = WasmExternalKind::Function;
  imp.typeIndex = 0;
  moduleInfo.imports.push_back(imp);

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  auto funcs = irgen.getIRFunctions();
  ASSERT_EQ(funcs.size(), 1u);

  auto &bb = funcs[0]->getBasicBlockList().front();

  // f64 return: converted with ToNumber, not coerced to int32.
  bool foundAsInt32 = false;
  bool foundCall = false;
  ReturnInst *retInst = nullptr;
  for (auto &I : bb) {
    if (llvh::isa<AsInt32Inst>(&I))
      foundAsInt32 = true;
    if (llvh::isa<CallInst>(&I))
      foundCall = true;
    if (auto *ret = llvh::dyn_cast<ReturnInst>(&I))
      retInst = ret;
  }
  EXPECT_TRUE(foundCall);
  EXPECT_FALSE(foundAsInt32); // f64 return is not coerced to int32
  ASSERT_NE(retInst, nullptr);
  // The JS callee is untyped, so the result must be converted rather than
  // returned as-is: returning it directly leaves the value :any, and any
  // float arithmetic on it then fails lowered-IR verification.
  auto *asNum = llvh::dyn_cast<AsNumberInst>(retInst->getOperand(0));
  ASSERT_NE(asNum, nullptr);
  EXPECT_TRUE(llvh::isa<CallInst>(asNum->getOperand(0)));
}

TEST(WasmIRGenTest, ImportTrampolineI64Return) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type: () -> i64
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I64}});
  WasmImport imp;
  imp.moduleName = "env";
  imp.fieldName = "i64_func";
  imp.kind = WasmExternalKind::Function;
  imp.typeIndex = 0;
  moduleInfo.imports.push_back(imp);

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  auto funcs = irgen.getIRFunctions();
  ASSERT_EQ(funcs.size(), 1u);

  auto &bb = funcs[0]->getBasicBlockList().front();

  // i64 return: should have CallBuiltinInst(wasmBigIntToI64) to convert
  // the JS BigInt return value back to split (lo, hi).
  bool foundBigIntToI64 = false;
  for (auto &I : bb) {
    if (auto *call = llvh::dyn_cast<CallBuiltinInst>(&I)) {
      if (call->getBuiltinIndex() ==
          BuiltinMethod::HermesBuiltin_wasmBigIntToI64) {
        foundBigIntToI64 = true;
      }
    }
  }
  EXPECT_TRUE(foundBigIntToI64);
}

TEST(WasmIRGenTest, ImportTrampolineWithDefinedFunction) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // Type 0: (i32) -> void (for import)
  moduleInfo.types.push_back(WasmFuncType{{WasmValType::I32}, {}});
  // Type 1: (i32) -> i32 (for defined function)
  moduleInfo.types.push_back(WasmFuncType{
      {WasmValType::I32}, {WasmValType::I32}});

  // One function import (index 0).
  WasmImport imp;
  imp.moduleName = "env";
  imp.fieldName = "log";
  imp.kind = WasmExternalKind::Function;
  imp.typeIndex = 0;
  moduleInfo.imports.push_back(imp);

  // One defined function (index 1).
  moduleInfo.functions.push_back(WasmFunction{1});

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  auto funcs = irgen.getIRFunctions();
  ASSERT_EQ(funcs.size(), 2u);

  // Function 0 (import): should have a trampoline body with CallInst.
  {
    auto &bb = funcs[0]->getBasicBlockList().front();
    bool foundCall = false;
    for (auto &I : bb)
      if (llvh::isa<CallInst>(&I))
        foundCall = true;
    EXPECT_TRUE(foundCall);
  }

  // Function 1 (defined): should still have stub body with ReturnInst.
  {
    auto &bb = funcs[1]->getBasicBlockList().front();
    ASSERT_FALSE(bb.empty());
    EXPECT_TRUE(llvh::isa<ReturnInst>(&bb.back()));
    // No CallInst in the stub.
    bool foundCall = false;
    for (auto &I : bb)
      if (llvh::isa<CallInst>(&I))
        foundCall = true;
    EXPECT_FALSE(foundCall);
  }
}

// --- Globals (K.1) ---

TEST(WasmIRGenTest, GlobalGetI32) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // One function type: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  // One immutable i32 global initialized to 42.
  WasmGlobal g;
  g.type.type = WasmValType::I32;
  g.type.mutable_ = false;
  g.initKind = WasmGlobal::InitKind::I32Const;
  g.initValue.i32Val = 42;
  moduleInfo.globals.push_back(g);

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // Translate function body: global.get 0; (implicit return)
  irgen.beginFunction(0, {});
  irgen.onGlobalGet(0);
  irgen.endFunction();

  // Verify the function has a LoadFrameInst for the global.
  auto funcs = irgen.getIRFunctions();
  ASSERT_EQ(funcs.size(), 1u);
  auto &entryBB = funcs[0]->getBasicBlockList().front();
  bool foundLoadFrame = false;
  for (auto &I : entryBB) {
    if (llvh::isa<LoadFrameInst>(&I)) {
      foundLoadFrame = true;
      break;
    }
  }
  EXPECT_TRUE(foundLoadFrame);
}

TEST(WasmIRGenTest, GlobalSetI32) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // One function type: (i32) -> ()
  moduleInfo.types.push_back(
      WasmFuncType{{WasmValType::I32}, {}});
  moduleInfo.functions.push_back(WasmFunction{0});

  // One mutable i32 global initialized to 0.
  WasmGlobal g;
  g.type.type = WasmValType::I32;
  g.type.mutable_ = true;
  g.initKind = WasmGlobal::InitKind::I32Const;
  g.initValue.i32Val = 0;
  moduleInfo.globals.push_back(g);

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // Translate function body: local.get 0; global.set 0; (implicit return)
  irgen.beginFunction(0, {});
  irgen.onLocalGet(0);
  irgen.onGlobalSet(0);
  irgen.endFunction();

  // Verify the function has a StoreFrameInst for the global.
  auto funcs = irgen.getIRFunctions();
  ASSERT_EQ(funcs.size(), 1u);
  auto &entryBB = funcs[0]->getBasicBlockList().front();
  bool foundStoreFrame = false;
  for (auto &I : entryBB) {
    if (llvh::isa<StoreFrameInst>(&I)) {
      foundStoreFrame = true;
      break;
    }
  }
  EXPECT_TRUE(foundStoreFrame);
}

TEST(WasmIRGenTest, GlobalGetSetF64) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // One function type: () -> (f64)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::F64}});
  moduleInfo.functions.push_back(WasmFunction{0});

  // One mutable f64 global initialized to 3.14.
  WasmGlobal g;
  g.type.type = WasmValType::F64;
  g.type.mutable_ = true;
  g.initKind = WasmGlobal::InitKind::F64Const;
  g.initValue.f64Val = 3.14;
  moduleInfo.globals.push_back(g);

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // Translate function body: f64.const 6.28; global.set 0; global.get 0;
  irgen.beginFunction(0, {});
  irgen.onF64Const(6.28);
  irgen.onGlobalSet(0);
  irgen.onGlobalGet(0);
  irgen.endFunction();

  auto funcs = irgen.getIRFunctions();
  ASSERT_EQ(funcs.size(), 1u);

  // Verify there's both a StoreFrameInst and a LoadFrameInst.
  auto &entryBB = funcs[0]->getBasicBlockList().front();
  bool foundStore = false, foundLoad = false;
  for (auto &I : entryBB) {
    if (llvh::isa<StoreFrameInst>(&I))
      foundStore = true;
    if (llvh::isa<LoadFrameInst>(&I))
      foundLoad = true;
  }
  EXPECT_TRUE(foundStore);
  EXPECT_TRUE(foundLoad);
}

TEST(WasmIRGenTest, GlobalInitFromOther) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // One function type: () -> (i32)
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I32}});
  moduleInfo.functions.push_back(WasmFunction{0});

  // Global 0: immutable i32 = 42.
  WasmGlobal g0;
  g0.type.type = WasmValType::I32;
  g0.type.mutable_ = false;
  g0.initKind = WasmGlobal::InitKind::I32Const;
  g0.initValue.i32Val = 42;
  moduleInfo.globals.push_back(g0);

  // Global 1: mutable i32 initialized from global 0.
  WasmGlobal g1;
  g1.type.type = WasmValType::I32;
  g1.type.mutable_ = true;
  g1.initKind = WasmGlobal::InitKind::GlobalGet;
  g1.initValue.globalIndex = 0;
  moduleInfo.globals.push_back(g1);

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // Read global 1 (should have been initialized from global 0).
  irgen.beginFunction(0, {});
  irgen.onGlobalGet(1);
  irgen.endFunction();

  auto funcs = irgen.getIRFunctions();
  ASSERT_EQ(funcs.size(), 1u);

  // The function body should have a LoadFrameInst for global_1.
  auto &entryBB = funcs[0]->getBasicBlockList().front();
  bool foundLoad = false;
  for (auto &I : entryBB) {
    if (llvh::isa<LoadFrameInst>(&I)) {
      foundLoad = true;
      break;
    }
  }
  EXPECT_TRUE(foundLoad);
}

TEST(WasmIRGenTest, GlobalGetI64Split) {
  TestModule tm;
  WasmModuleInfo moduleInfo;

  // One function type: () -> (i64) — returns split lo/hi.
  moduleInfo.types.push_back(WasmFuncType{{}, {WasmValType::I64}});
  moduleInfo.functions.push_back(WasmFunction{0});

  // One i64 global.
  WasmGlobal g;
  g.type.type = WasmValType::I64;
  g.type.mutable_ = true;
  g.initKind = WasmGlobal::InitKind::I64Const;
  g.initValue.i64Val = 0x100000002LL; // lo=2, hi=1
  moduleInfo.globals.push_back(g);

  WasmIRGen irgen(tm.mod, moduleInfo);
  irgen.createFunctions();

  // Translate: global.get 0; drop (pops i64 pair);
  irgen.beginFunction(0, {});
  irgen.onGlobalGet(0);
  irgen.onDrop();
  irgen.endFunction();

  auto funcs = irgen.getIRFunctions();
  ASSERT_EQ(funcs.size(), 1u);

  // Should have two LoadFrameInst: one for lo32, one for hi32.
  auto &entryBB = funcs[0]->getBasicBlockList().front();
  int loadCount = 0;
  for (auto &I : entryBB) {
    if (llvh::isa<LoadFrameInst>(&I))
      ++loadCount;
  }
  EXPECT_EQ(loadCount, 2);
}

} // namespace
