/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/WasmIRGen/WasmIRGen.h"

#include "hermes/FrontEndDefs/Builtins.h"
#include "hermes/IR/IR.h"
#include "hermes/IR/IRBuilder.h"
#include "hermes/IR/Instrs.h"
#include "hermes/WasmFrontend/WasmModuleInfo.h"

#include "llvh/ADT/Twine.h"

namespace hermes {
namespace wasm {

WasmIRGen::WasmIRGen(Module &M, WasmModuleInfo &moduleInfo)
    : moduleInfo_(moduleInfo), builder_(&M) {}

void WasmIRGen::createFunctions() {
  uint32_t totalFuncs = moduleInfo_.totalFunctionCount();
  irFunctions_.resize(totalFuncs, nullptr);

  for (uint32_t i = 0; i < totalFuncs; ++i) {
    const WasmFuncType &funcType = moduleInfo_.getFunctionType(i);

    // Derive a name for the function.
    std::string name;
    if (i < moduleInfo_.names.functionNames.size() &&
        !moduleInfo_.names.functionNames[i].empty()) {
      name = moduleInfo_.names.functionNames[i];
    } else {
      name = ("wasm_func_" + llvh::Twine(i)).str();
    }

    // Create the IR function.
    auto *func = builder_.createFunction(
        name,
        Function::DefinitionKind::ES5Function,
        true /* strictMode */);

    // Add a "this" parameter (required by Hermes calling convention).
    builder_.createJSThisParam(func);

    // Add one JSDynamicParam per Wasm parameter.
    for (uint32_t p = 0; p < funcType.params.size(); ++p) {
      builder_.createJSDynamicParam(
          func, ("p" + llvh::Twine(p)).str());
    }

    // Create a single entry basic block with ReturnInst(undefined).
    auto *entry = builder_.createBasicBlock(func);
    builder_.setInsertionBlock(entry);
    builder_.createReturnInst(builder_.getLiteralUndefined());

    irFunctions_[i] = func;
  }
}

void WasmIRGen::beginFunction(
    uint32_t funcIndex,
    const std::vector<WasmValType> &localTypes) {
  assert(
      funcIndex < irFunctions_.size() &&
      "funcIndex out of range");
  currentFuncIndex_ = funcIndex;
  currentFunc_ = irFunctions_[funcIndex];
  assert(currentFunc_ && "IR function not created");

  valueStack_.clear();
  locals_.clear();
  controlStack_.clear();

  const WasmFuncType &funcType = moduleInfo_.getFunctionType(funcIndex);

  // Remove the placeholder return instruction and entry block content.
  // The entry block was created with a single ReturnInst(undefined) by
  // createFunctions(). We clear it and reuse the block.
  auto &entryBB = currentFunc_->getBasicBlockList().front();
  // Remove all instructions from the entry block.
  while (!entryBB.empty()) {
    entryBB.back().eraseFromParent();
  }
  builder_.setInsertionBlock(&entryBB);

  // Create AllocStackInst for each parameter.
  uint32_t numParams = funcType.params.size();
  for (uint32_t i = 0; i < numParams; ++i) {
    auto *alloc = builder_.createAllocStackInst(
        ("local_" + llvh::Twine(i)).str(),
        Type::createAnyType());
    locals_.push_back(alloc);

    // Initialize from function parameter. JSDynamicParams are indexed
    // starting after "this" (index 0), so param i is at index i+1.
    auto *param = currentFunc_->getJSDynamicParam(i + 1);
    auto *loadParam = builder_.createLoadParamInst(param);
    builder_.createStoreStackInst(loadParam, alloc);
  }

  // Create AllocStackInst for each declared local, initialized to zero.
  for (uint32_t i = 0; i < localTypes.size(); ++i) {
    auto *alloc = builder_.createAllocStackInst(
        ("local_" + llvh::Twine(numParams + i)).str(),
        Type::createAnyType());
    locals_.push_back(alloc);

    // Initialize locals to their zero value.
    Value *zeroVal;
    switch (localTypes[i]) {
      case WasmValType::I32:
      case WasmValType::I64:
      case WasmValType::F32:
      case WasmValType::F64:
        zeroVal = builder_.getLiteralNumber(0);
        break;
      case WasmValType::FuncRef:
      case WasmValType::ExternRef:
        zeroVal = builder_.getLiteralNull();
        break;
      default:
        zeroVal = builder_.getLiteralNumber(0);
        break;
    }
    builder_.createStoreStackInst(zeroVal, alloc);
  }
}

void WasmIRGen::endFunction() {
  const WasmFuncType &funcType =
      moduleInfo_.getFunctionType(currentFuncIndex_);

  // If the function has a result type and there's a value on the stack,
  // return it. Otherwise return undefined.
  if (!funcType.results.empty() && !valueStack_.empty()) {
    Value *result = pop();
    builder_.createReturnInst(result);
  } else {
    builder_.createReturnInst(builder_.getLiteralUndefined());
  }

  currentFunc_ = nullptr;
  valueStack_.clear();
  locals_.clear();
  controlStack_.clear();
}

void WasmIRGen::onI32Const(int32_t value) {
  push(builder_.getLiteralNumber(static_cast<double>(value)));
}

void WasmIRGen::onI64Const(int64_t value) {
  // Split i64 into lo32 and hi32 (Phase 1 representation).
  auto lo = static_cast<int32_t>(value & 0xFFFFFFFF);
  auto hi = static_cast<int32_t>((static_cast<uint64_t>(value) >> 32) & 0xFFFFFFFF);
  push(builder_.getLiteralNumber(static_cast<double>(lo)));
  push(builder_.getLiteralNumber(static_cast<double>(hi)));
}

void WasmIRGen::onF32Const(float value) {
  push(builder_.getLiteralNumber(static_cast<double>(value)));
}

void WasmIRGen::onF64Const(double value) {
  push(builder_.getLiteralNumber(value));
}

void WasmIRGen::onLocalGet(uint32_t localIndex) {
  assert(localIndex < locals_.size() && "localIndex out of range");
  push(builder_.createLoadStackInst(locals_[localIndex]));
}

void WasmIRGen::onLocalSet(uint32_t localIndex) {
  assert(localIndex < locals_.size() && "localIndex out of range");
  Value *val = pop();
  builder_.createStoreStackInst(val, locals_[localIndex]);
}

void WasmIRGen::onLocalTee(uint32_t localIndex) {
  assert(localIndex < locals_.size() && "localIndex out of range");
  Value *val = pop();
  builder_.createStoreStackInst(val, locals_[localIndex]);
  push(val);
}

// --- i32 arithmetic (D.3) ---

void WasmIRGen::onI32Add() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *add = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryAddInstKind);
  push(builder_.createAsInt32Inst(add));
}

void WasmIRGen::onI32Sub() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *sub = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinarySubtractInstKind);
  push(builder_.createAsInt32Inst(sub));
}

void WasmIRGen::onI32Mul() {
  // Use Math.imul for correctness: double multiplication loses precision
  // for large int32 products (e.g., 65536 * 65536 overflows 53-bit mantissa).
  Value *rhs = pop();
  Value *lhs = pop();
  auto *imul = builder_.createCallBuiltinInst(
      BuiltinMethod::Math_imul, {lhs, rhs});
  push(imul);
}

void WasmIRGen::onI32And() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryAndInstKind));
}

void WasmIRGen::onI32Or() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32Xor() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryXorInstKind));
}

void WasmIRGen::onI32Shl() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryLeftShiftInstKind));
}

void WasmIRGen::onI32ShrS() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryRightShiftInstKind));
}

void WasmIRGen::onI32ShrU() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryUnsignedRightShiftInstKind));
}

// --- i32 comparisons (D.4) ---

void WasmIRGen::onI32Eq() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryStrictlyEqualInstKind);
  // Convert boolean to i32 (true→1, false→0) via BitOr with 0.
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32Ne() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryStrictlyNotEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32LtS() {
  Value *rhs = pop();
  Value *lhs = pop();
  // Signed: cast both operands to int32 before comparing.
  auto *lhsI32 = builder_.createAsInt32Inst(lhs);
  auto *rhsI32 = builder_.createAsInt32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsI32, rhsI32, ValueKind::BinaryLessThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32GtS() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsI32 = builder_.createAsInt32Inst(lhs);
  auto *rhsI32 = builder_.createAsInt32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsI32, rhsI32, ValueKind::BinaryGreaterThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32LeS() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsI32 = builder_.createAsInt32Inst(lhs);
  auto *rhsI32 = builder_.createAsInt32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsI32, rhsI32, ValueKind::BinaryLessThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32GeS() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsI32 = builder_.createAsInt32Inst(lhs);
  auto *rhsI32 = builder_.createAsInt32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsI32, rhsI32, ValueKind::BinaryGreaterThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32LtU() {
  Value *rhs = pop();
  Value *lhs = pop();
  // Unsigned: cast both operands to uint32 before comparing.
  auto *lhsU32 = builder_.createAsUint32Inst(lhs);
  auto *rhsU32 = builder_.createAsUint32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsU32, rhsU32, ValueKind::BinaryLessThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32GtU() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsU32 = builder_.createAsUint32Inst(lhs);
  auto *rhsU32 = builder_.createAsUint32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsU32, rhsU32, ValueKind::BinaryGreaterThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32LeU() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsU32 = builder_.createAsUint32Inst(lhs);
  auto *rhsU32 = builder_.createAsUint32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsU32, rhsU32, ValueKind::BinaryLessThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32GeU() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsU32 = builder_.createAsUint32Inst(lhs);
  auto *rhsU32 = builder_.createAsUint32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsU32, rhsU32, ValueKind::BinaryGreaterThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32Eqz() {
  Value *val = pop();
  // eqz(x) == (x === 0) → boolean → i32.
  auto *cmp = builder_.createBinaryOperatorInst(
      val,
      builder_.getLiteralNumber(0),
      ValueKind::BinaryStrictlyEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

Value *WasmIRGen::pop() {
  assert(!valueStack_.empty() && "value stack underflow");
  Value *v = valueStack_.back();
  valueStack_.pop_back();
  return v;
}

void WasmIRGen::push(Value *v) {
  valueStack_.push_back(v);
}

} // namespace wasm
} // namespace hermes
