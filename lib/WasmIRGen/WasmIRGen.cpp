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

#include "llvh/ADT/DenseMap.h"
#include "llvh/ADT/SmallVector.h"
#include "llvh/ADT/Twine.h"
#include "llvh/Support/raw_ostream.h"

namespace hermes {
namespace wasm {

WasmIRGen::WasmIRGen(Module &M, WasmModuleInfo &moduleInfo)
    : moduleInfo_(moduleInfo), builder_(&M), helpers_(builder_) {}

void WasmIRGen::createFunctions() {
  // Create the top-level function first (must be before other functions).
  auto *topLevel = builder_.createTopLevelFunction(
      "global", true /* strictMode */);
  topLevel->setExpectedParamCountIncludingThis(1); // just "this"

  // Create a VariableScope for the top-level function (no parent).
  topLevelVS_ = builder_.createVariableScope(nullptr);

  // If the module has memory, create Variables for the 8 typed array views.
  bool hasMemory = moduleInfo_.totalMemoryCount() > 0;
  if (hasMemory) {
    static const char *viewNames[NUM_MEM_VIEWS] = {
        "HEAP8",
        "HEAPU8",
        "HEAP16",
        "HEAPU16",
        "HEAP32",
        "HEAPU32",
        "HEAPF32",
        "HEAPF64",
    };
    for (uint8_t i = 0; i < NUM_MEM_VIEWS; ++i) {
      memViewVars_[i] = builder_.createVariable(
          topLevelVS_,
          viewNames[i],
          Type::createAnyType(),
          /* hidden */ true);
    }
  }

  // Create all Wasm functions and a Variable in the top-level scope for each.
  uint32_t totalFuncs = moduleInfo_.totalFunctionCount();
  irFunctions_.resize(totalFuncs, nullptr);
  closureVars_.resize(totalFuncs, nullptr);

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

    // Create a variable in the top-level scope to hold the pre-created closure.
    closureVars_[i] = builder_.createVariable(
        topLevelVS_,
        ("closure_" + llvh::Twine(i)),
        Type::createAnyType(),
        /* hidden */ true);

    // Add a "this" parameter (required by Hermes calling convention).
    builder_.createJSThisParam(func);

    // Add JSDynamicParams per Wasm parameter. i64 params need two slots
    // (lo32, hi32) for the split representation.
    uint32_t jsParamCount = 0;
    for (uint32_t p = 0; p < funcType.params.size(); ++p) {
      if (funcType.params[p] == WasmValType::I64) {
        builder_.createJSDynamicParam(
            func, ("p" + llvh::Twine(p) + "_lo").str());
        builder_.createJSDynamicParam(
            func, ("p" + llvh::Twine(p) + "_hi").str());
        jsParamCount += 2;
      } else {
        builder_.createJSDynamicParam(
            func, ("p" + llvh::Twine(p)).str());
        jsParamCount += 1;
      }
    }

    // Set the expected param count (including "this").
    func->setExpectedParamCountIncludingThis(jsParamCount + 1);

    // Create a single entry basic block with ReturnInst(undefined).
    auto *entry = builder_.createBasicBlock(func);
    builder_.setInsertionBlock(entry);
    builder_.createReturnInst(builder_.getLiteralUndefined());

    irFunctions_[i] = func;
  }

  // Populate the top-level function body.
  // Create all closures once and store them in the top-level scope.
  auto *tlEntry = builder_.createBasicBlock(topLevel);
  builder_.setInsertionBlock(tlEntry);

  // Create a scope for the top-level function.
  auto *tlScope = builder_.createCreateScopeInst(
      topLevelVS_, builder_.getEmptySentinel());

  // Pre-create closures for all Wasm functions and store in the environment.
  for (uint32_t i = 0; i < totalFuncs; ++i) {
    auto *closure = builder_.createCreateFunctionInst(
        tlScope, irFunctions_[i]);
    builder_.createStoreFrameInst(tlScope, closure, closureVars_[i]);
  }

  // Create typed array views for the linear memory if present.
  if (hasMemory) {
    createMemoryViews(tlScope);
  }

  // Call the start function if specified (load its pre-created closure).
  if (moduleInfo_.startFunction.has_value()) {
    uint32_t startIdx = *moduleInfo_.startFunction;
    if (startIdx < irFunctions_.size()) {
      auto *closure = builder_.createLoadFrameInst(
          tlScope, closureVars_[startIdx]);
      builder_.createCallInst(
          closure,
          /* newTarget */ builder_.getLiteralUndefined(),
          /* thisValue */ builder_.getLiteralUndefined(),
          {});
    }
  }

  // Build the exports object: a plain JS object mapping export names to
  // their pre-created closures. Only function exports are handled; other
  // export kinds (memory, table, global) are silently skipped for now.
  auto *exportsObj = builder_.createAllocObjectLiteralInst({});
  for (const auto &exp : moduleInfo_.exports) {
    if (exp.kind != WasmExternalKind::Function)
      continue;
    assert(
        exp.index < closureVars_.size() &&
        "export function index out of range");
    auto *closure = builder_.createLoadFrameInst(
        tlScope, closureVars_[exp.index]);
    builder_.createStorePropertyStrictInst(
        closure, exportsObj, builder_.getLiteralString(exp.name));
  }

  builder_.createReturnInst(exportsObj);
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
  valueStackIsI64Hi_.clear();
  locals_.clear();
  localSlotIndex_.clear();
  localTypes_.clear();
  controlStack_.clear();
  unreachable_ = false;

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

  // Get the parent (top-level) scope. Used to load pre-created closures
  // from the environment at call sites.
  parentScopeInst_ = builder_.createGetParentScopeInst(
      topLevelVS_, currentFunc_->getParentScopeParam());

  // Build local type map (params + declared locals).
  uint32_t numParams = funcType.params.size();
  for (uint32_t i = 0; i < numParams; ++i) {
    localTypes_.push_back(funcType.params[i]);
  }
  for (uint32_t i = 0; i < localTypes.size(); ++i) {
    localTypes_.push_back(localTypes[i]);
  }

  // Create AllocStackInst for each parameter. i64 params use 2 slots.
  // JSDynamicParam index tracks the expanding JS param list.
  uint32_t jsParamIdx = 1; // 0 = "this"
  for (uint32_t i = 0; i < numParams; ++i) {
    localSlotIndex_.push_back(locals_.size());
    if (funcType.params[i] == WasmValType::I64) {
      // i64 param: allocate lo and hi stack slots.
      auto *allocLo = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(i) + "_lo").str(),
          Type::createAnyType());
      auto *allocHi = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(i) + "_hi").str(),
          Type::createAnyType());
      locals_.push_back(allocLo);
      locals_.push_back(allocHi);

      auto *paramLo = currentFunc_->getJSDynamicParam(jsParamIdx);
      auto *paramHi = currentFunc_->getJSDynamicParam(jsParamIdx + 1);
      builder_.createStoreStackInst(
          builder_.createLoadParamInst(paramLo), allocLo);
      builder_.createStoreStackInst(
          builder_.createLoadParamInst(paramHi), allocHi);
      jsParamIdx += 2;
    } else {
      auto *alloc = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(i)).str(),
          Type::createAnyType());
      locals_.push_back(alloc);

      auto *param = currentFunc_->getJSDynamicParam(jsParamIdx);
      builder_.createStoreStackInst(
          builder_.createLoadParamInst(param), alloc);
      jsParamIdx += 1;
    }
  }

  // Create AllocStackInst for each declared local, initialized to zero.
  for (uint32_t i = 0; i < localTypes.size(); ++i) {
    localSlotIndex_.push_back(locals_.size());
    if (localTypes[i] == WasmValType::I64) {
      // i64 local: allocate lo and hi stack slots, both initialized to 0.
      auto *allocLo = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(numParams + i) + "_lo").str(),
          Type::createAnyType());
      auto *allocHi = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(numParams + i) + "_hi").str(),
          Type::createAnyType());
      locals_.push_back(allocLo);
      locals_.push_back(allocHi);
      builder_.createStoreStackInst(builder_.getLiteralNumber(0), allocLo);
      builder_.createStoreStackInst(builder_.getLiteralNumber(0), allocHi);
    } else {
      auto *alloc = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(numParams + i)).str(),
          Type::createAnyType());
      locals_.push_back(alloc);

      // Initialize locals to their zero value.
      Value *zeroVal;
      switch (localTypes[i]) {
        case WasmValType::I32:
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

  // Push an implicit function-level control entry. The function body acts
  // as an implicit block — wabt calls OnEndExpr for the function body's
  // final "end", which pops this entry via onEnd().
  auto *exitBlock = builder_.createBasicBlock(currentFunc_);

  ControlEntry funcEntry;
  funcEntry.kind = ControlEntry::Block;
  funcEntry.contBlock = exitBlock;
  funcEntry.elseBlock = nullptr;
  funcEntry.resultTypes = funcType.results;
  funcEntry.stackHeight = 0;

  if (!funcType.results.empty()) {
    auto *savedBlock = builder_.getInsertionBlock();
    builder_.setInsertionBlock(exitBlock);
    for (size_t i = 0; i < funcType.results.size(); ++i) {
      funcEntry.resultPhis.push_back(builder_.createPhiInst());
      // i64 results need a second phi for the hi32 part.
      if (funcType.results[i] == WasmValType::I64) {
        funcEntry.resultPhis.push_back(builder_.createPhiInst());
      }
    }
    builder_.setInsertionBlock(savedBlock);
  }

  controlStack_.push_back(std::move(funcEntry));
}

void WasmIRGen::endFunction() {
  // If the control stack still has the implicit function-level entry (e.g.,
  // in unit tests that skip onEnd), pop remaining entries.
  while (!controlStack_.empty()) {
    onEnd();
  }

  // Emit a return if the current block is not terminated.
  if (!isCurrentBlockTerminated()) {
    const WasmFuncType &funcType =
        moduleInfo_.getFunctionType(currentFuncIndex_);
    if (!funcType.results.empty() && !valueStack_.empty()) {
      if (funcType.results[0] == WasmValType::I64 && isTopI64()) {
        auto [lo, hi] = popI64();
        helpers_.emitI64HiStash(hi);
        builder_.createReturnInst(lo);
      } else {
        Value *result = pop();
        builder_.createReturnInst(result);
      }
    } else {
      builder_.createReturnInst(builder_.getLiteralUndefined());
    }
  }

  // Remove dead blocks that were created after unconditional branches/returns
  // but never received instructions. The BCGen verifier requires every block
  // to be reachable and have a terminator.
  llvh::SmallVector<BasicBlock *, 4> deadBlocks;
  for (auto &BB : currentFunc_->getBasicBlockList()) {
    if (BB.empty() || !llvh::isa<TerminatorInst>(&BB.back())) {
      deadBlocks.push_back(&BB);
    }
  }
  for (auto *BB : deadBlocks) {
    BB->eraseFromParent();
  }

  currentFunc_ = nullptr;
  parentScopeInst_ = nullptr;
  valueStack_.clear();
  valueStackIsI64Hi_.clear();
  locals_.clear();
  localSlotIndex_.clear();
  localTypes_.clear();
  controlStack_.clear();
}

void WasmIRGen::onI32Const(int32_t value) {
  push(builder_.getLiteralNumber(static_cast<double>(value)));
}

void WasmIRGen::onI64Const(int64_t value) {
  // Split i64 into lo32 and hi32 (Phase 1 representation).
  auto lo = static_cast<int32_t>(value & 0xFFFFFFFF);
  auto hi = static_cast<int32_t>(
      (static_cast<uint64_t>(value) >> 32) & 0xFFFFFFFF);
  pushI64(
      builder_.getLiteralNumber(static_cast<double>(lo)),
      builder_.getLiteralNumber(static_cast<double>(hi)));
}

void WasmIRGen::onF32Const(float value) {
  push(builder_.getLiteralNumber(static_cast<double>(value)));
}

void WasmIRGen::onF64Const(double value) {
  push(builder_.getLiteralNumber(value));
}

void WasmIRGen::onLocalGet(uint32_t localIndex) {
  assert(localIndex < localSlotIndex_.size() && "localIndex out of range");
  uint32_t slot = localSlotIndex_[localIndex];
  if (localTypes_[localIndex] == WasmValType::I64) {
    auto *lo = builder_.createLoadStackInst(locals_[slot]);
    auto *hi = builder_.createLoadStackInst(locals_[slot + 1]);
    pushI64(lo, hi);
  } else {
    push(builder_.createLoadStackInst(locals_[slot]));
  }
}

void WasmIRGen::onLocalSet(uint32_t localIndex) {
  assert(localIndex < localSlotIndex_.size() && "localIndex out of range");
  uint32_t slot = localSlotIndex_[localIndex];
  if (localTypes_[localIndex] == WasmValType::I64) {
    auto [lo, hi] = popI64();
    builder_.createStoreStackInst(lo, locals_[slot]);
    builder_.createStoreStackInst(hi, locals_[slot + 1]);
  } else {
    Value *val = pop();
    builder_.createStoreStackInst(val, locals_[slot]);
  }
}

void WasmIRGen::onLocalTee(uint32_t localIndex) {
  assert(localIndex < localSlotIndex_.size() && "localIndex out of range");
  uint32_t slot = localSlotIndex_[localIndex];
  if (localTypes_[localIndex] == WasmValType::I64) {
    auto [lo, hi] = popI64();
    builder_.createStoreStackInst(lo, locals_[slot]);
    builder_.createStoreStackInst(hi, locals_[slot + 1]);
    pushI64(lo, hi);
  } else {
    Value *val = pop();
    builder_.createStoreStackInst(val, locals_[slot]);
    push(val);
  }
}

// --- Return and drop (D.5) ---

void WasmIRGen::onReturn() {
  if (unreachable_)
    return;

  const WasmFuncType &funcType =
      moduleInfo_.getFunctionType(currentFuncIndex_);

  if (!funcType.results.empty()) {
    if (funcType.results[0] == WasmValType::I64) {
      auto [lo, hi] = popI64();
      helpers_.emitI64HiStash(hi);
      builder_.createReturnInst(lo);
    } else {
      Value *result = pop();
      builder_.createReturnInst(result);
    }
  } else {
    builder_.createReturnInst(builder_.getLiteralUndefined());
  }

  // After an unconditional return, code is unreachable.
  unreachable_ = true;

  // Create a new dead basic block for any dead code that follows.
  auto *deadBlock = builder_.createBasicBlock(currentFunc_);
  builder_.setInsertionBlock(deadBlock);
}

void WasmIRGen::onDrop() {
  if (isTopI64()) {
    popI64();
  } else {
    pop();
  }
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

// --- i32 trapping division (F.2) ---

void WasmIRGen::onI32DivS() {
  if (unreachable_)
    return;
  Value *rhs = pop();
  Value *lhs = pop();
  push(helpers_.emitI32DivS(lhs, rhs));
}

void WasmIRGen::onI32DivU() {
  if (unreachable_)
    return;
  Value *rhs = pop();
  Value *lhs = pop();
  push(helpers_.emitI32DivU(lhs, rhs));
}

void WasmIRGen::onI32RemS() {
  if (unreachable_)
    return;
  Value *rhs = pop();
  Value *lhs = pop();
  push(helpers_.emitI32RemS(lhs, rhs));
}

void WasmIRGen::onI32RemU() {
  if (unreachable_)
    return;
  Value *rhs = pop();
  Value *lhs = pop();
  push(helpers_.emitI32RemU(lhs, rhs));
}

// --- i32 bit manipulation (F.3) ---

void WasmIRGen::onI32Clz() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32Clz(a));
}

void WasmIRGen::onI32Ctz() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32Ctz(a));
}

void WasmIRGen::onI32Popcnt() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32Popcnt(a));
}

void WasmIRGen::onI32Rotl() {
  if (unreachable_)
    return;
  Value *rhs = pop();
  Value *lhs = pop();
  push(helpers_.emitI32Rotl(lhs, rhs));
}

void WasmIRGen::onI32Rotr() {
  if (unreachable_)
    return;
  Value *rhs = pop();
  Value *lhs = pop();
  push(helpers_.emitI32Rotr(lhs, rhs));
}

void WasmIRGen::onI32Extend8S() {
  if (unreachable_)
    return;
  Value *a = pop();
  // Sign-extend from 8 bits: (a << 24) >> 24
  auto *shifted = builder_.createBinaryOperatorInst(
      a, builder_.getLiteralNumber(24), ValueKind::BinaryLeftShiftInstKind);
  push(builder_.createBinaryOperatorInst(
      shifted,
      builder_.getLiteralNumber(24),
      ValueKind::BinaryRightShiftInstKind));
}

void WasmIRGen::onI32Extend16S() {
  if (unreachable_)
    return;
  Value *a = pop();
  // Sign-extend from 16 bits: (a << 16) >> 16
  auto *shifted = builder_.createBinaryOperatorInst(
      a, builder_.getLiteralNumber(16), ValueKind::BinaryLeftShiftInstKind);
  push(builder_.createBinaryOperatorInst(
      shifted,
      builder_.getLiteralNumber(16),
      ValueKind::BinaryRightShiftInstKind));
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

// --- Control flow (D.6, D.7) ---

void WasmIRGen::onBlock(const std::vector<WasmValType> &resultTypes) {
  // Create a continuation basic block (target of br 0 = after end).
  auto *contBlock = builder_.createBasicBlock(currentFunc_);

  ControlEntry entry;
  entry.kind = ControlEntry::Block;
  entry.contBlock = contBlock;
  entry.resultTypes = resultTypes;
  entry.stackHeight = valueStack_.size();
  entry.outerUnreachable = unreachable_;

  // Create phi nodes in the continuation block (i64 results get 2 phis).
  if (!resultTypes.empty()) {
    entry.resultPhis = createResultPhis(contBlock, resultTypes);
  }

  controlStack_.push_back(std::move(entry));
}

void WasmIRGen::onLoop(const std::vector<WasmValType> &resultTypes) {
  // Create the loop header block. br targeting this loop jumps here.
  auto *headerBlock = builder_.createBasicBlock(currentFunc_);

  // Create the end block (after the loop's end, where fallthrough goes).
  auto *endBlock = builder_.createBasicBlock(currentFunc_);

  ControlEntry entry;
  entry.kind = ControlEntry::Loop;
  entry.contBlock = headerBlock; // br targets the loop header
  entry.endBlock = endBlock; // fallthrough after end goes here
  entry.resultTypes = resultTypes;
  entry.stackHeight = valueStack_.size();
  entry.outerUnreachable = unreachable_;

  // Result phis go in the end block (for fallthrough values).
  // i64 results get 2 phis each.
  if (!resultTypes.empty()) {
    entry.resultPhis = createResultPhis(endBlock, resultTypes);
  }

  // Branch from the current block to the loop header.
  if (!unreachable_ && !isCurrentBlockTerminated()) {
    builder_.createBranchInst(headerBlock);
  }

  // Set insertion point to the loop header.
  builder_.setInsertionBlock(headerBlock);

  controlStack_.push_back(std::move(entry));
}

void WasmIRGen::onIf(const std::vector<WasmValType> &resultTypes) {
  if (unreachable_) {
    // Push a dummy If entry so onEnd/onElse can pop it.
    ControlEntry entry;
    entry.kind = ControlEntry::If;
    entry.contBlock = nullptr;
    entry.elseBlock = nullptr;
    entry.resultTypes = resultTypes;
    entry.stackHeight = valueStack_.size();
    entry.outerUnreachable = true;
    controlStack_.push_back(std::move(entry));
    return;
  }

  // Pop the condition.
  Value *cond = pop();

  // Create thenBlock, elseBlock, mergeBlock.
  auto *thenBlock = builder_.createBasicBlock(currentFunc_);
  auto *elseBlock = builder_.createBasicBlock(currentFunc_);
  auto *mergeBlock = builder_.createBasicBlock(currentFunc_);

  // Emit CondBranchInst: non-zero → thenBlock, zero → elseBlock.
  builder_.createCondBranchInst(cond, thenBlock, elseBlock);

  ControlEntry entry;
  entry.kind = ControlEntry::If;
  entry.contBlock = mergeBlock; // br target and end continuation
  entry.elseBlock = elseBlock;
  entry.resultTypes = resultTypes;
  entry.stackHeight = valueStack_.size();
  entry.outerUnreachable = unreachable_;

  // Create phi nodes in the merge block for results (i64 results get 2 phis).
  if (!resultTypes.empty()) {
    entry.resultPhis = createResultPhis(mergeBlock, resultTypes);
  }

  controlStack_.push_back(std::move(entry));

  // Set insertion point to the thenBlock.
  builder_.setInsertionBlock(thenBlock);
}

void WasmIRGen::onElse() {
  assert(!controlStack_.empty() && "control stack underflow");
  ControlEntry &entry = controlStack_.back();
  assert(entry.kind == ControlEntry::If && "onElse without matching if");

  if (!entry.outerUnreachable) {
    bool fallsThrough = !unreachable_ && !isCurrentBlockTerminated();

    if (fallsThrough) {
      // The then-block falls through to mergeBlock.
      addBranchPhiOperands(entry);
      builder_.createBranchInst(entry.contBlock);
    }

    // Set insertion point to the elseBlock.
    builder_.setInsertionBlock(entry.elseBlock);
  }

  // Restore the value stack to the entry height (discard then-block values).
  valueStack_.resize(entry.stackHeight);
  valueStackIsI64Hi_.resize(entry.stackHeight);

  // Reset unreachable for the else arm.
  unreachable_ = entry.outerUnreachable;

  // Mark that we've consumed the else block (so onEnd knows).
  entry.elseBlock = nullptr;
}

void WasmIRGen::onEnd() {
  assert(!controlStack_.empty() && "control stack underflow");
  ControlEntry entry = std::move(controlStack_.back());
  controlStack_.pop_back();

  if (entry.kind == ControlEntry::Block || entry.kind == ControlEntry::If) {
    bool fallsThrough = !unreachable_ && !isCurrentBlockTerminated();

    if (fallsThrough) {
      // Add phi operands from the fallthrough path.
      addBranchPhiOperands(entry);
      builder_.createBranchInst(entry.contBlock);
    }

    // For If without else: the else block branches directly to merge.
    if (entry.kind == ControlEntry::If && entry.elseBlock != nullptr &&
        !entry.outerUnreachable) {
      auto *savedBlock = builder_.getInsertionBlock();
      builder_.setInsertionBlock(entry.elseBlock);
      builder_.createBranchInst(entry.contBlock);
      builder_.setInsertionBlock(savedBlock);
    }

    // Set insertion point to the continuation block.
    builder_.setInsertionBlock(entry.contBlock);

    // The continuation block is reachable if we fell through, if any
    // branch (br/br_if) targeted this block, or if there was an if
    // without else (the else path always reaches merge).
    bool ifWithoutElse =
        entry.kind == ControlEntry::If && entry.elseBlock != nullptr;
    unreachable_ = !fallsThrough && !entry.branchTargeted && !ifWithoutElse;

    // Restore unreachable if the outer context was unreachable.
    if (entry.outerUnreachable)
      unreachable_ = true;

    // Push phi results onto the value stack (i64 results push 2 values).
    pushResultPhis(entry);
  } else if (entry.kind == ControlEntry::Loop) {
    bool fallsThrough = !unreachable_ && !isCurrentBlockTerminated();

    if (fallsThrough) {
      // Add phi operands to the end block from the fallthrough path.
      // We handle this directly here rather than via addBranchPhiOperands,
      // because addBranchPhiOperands skips Loop entries (since br to a
      // loop targets the header, not the end block).
      if (!entry.resultPhis.empty()) {
        auto *currentBlock = builder_.getInsertionBlock();
        size_t numPhis = entry.resultPhis.size();
        size_t available = valueStack_.size();
        if (available >= numPhis) {
          for (size_t i = 0; i < numPhis; ++i) {
            Value *val = valueStack_[available - numPhis + i];
            entry.resultPhis[i]->addEntry(val, currentBlock);
          }
          valueStack_.resize(available - numPhis);
          valueStackIsI64Hi_.resize(available - numPhis);
        } else {
          // Stack underflow — use undefined as placeholder.
          for (size_t i = 0; i < numPhis; ++i) {
            Value *val = (i >= numPhis - available)
                ? valueStack_[i - (numPhis - available)]
                : builder_.getLiteralUndefined();
            entry.resultPhis[i]->addEntry(val, currentBlock);
          }
          valueStack_.clear();
          valueStackIsI64Hi_.clear();
        }
      }
      builder_.createBranchInst(entry.endBlock);
    }

    // Set insertion point to the end block (after the loop).
    builder_.setInsertionBlock(entry.endBlock);

    // The end block is reachable if we fell through.
    // Note: branchTargeted only tracks br to the loop header; it does NOT
    // make the end block reachable. Only fallthrough makes it reachable.
    unreachable_ = !fallsThrough;

    // Push phi results onto the value stack (i64 results push 2 values).
    pushResultPhis(entry);
  }
}

void WasmIRGen::onBr(uint32_t depth) {
  if (unreachable_)
    return;

  ControlEntry &entry = getControlEntry(depth);
  entry.branchTargeted = true;

  // Add phi operands for the branch target.
  addBranchPhiOperands(entry);

  // Branch to the target.
  builder_.createBranchInst(entry.contBlock);

  // After an unconditional branch, code is unreachable.
  unreachable_ = true;

  // Create a new dead basic block for any dead code that follows.
  auto *deadBlock = builder_.createBasicBlock(currentFunc_);
  builder_.setInsertionBlock(deadBlock);
}

void WasmIRGen::onBrIf(uint32_t depth) {
  if (unreachable_)
    return;

  ControlEntry &entry = getControlEntry(depth);
  entry.branchTargeted = true;

  // Pop the condition.
  Value *cond = pop();

  // Create a fallthrough block for when the condition is false.
  auto *fallthroughBlock = builder_.createBasicBlock(currentFunc_);

  // If the block has results, peek at the value stack (don't pop) and add
  // phi operands from the branch-taken path. Values stay for fallthrough.
  peekBranchPhiOperands(entry);

  // Emit conditional branch: non-zero condition branches to target.
  builder_.createCondBranchInst(cond, entry.contBlock, fallthroughBlock);

  // Continue generating code in the fallthrough block.
  builder_.setInsertionBlock(fallthroughBlock);
}

void WasmIRGen::onBrTable(
    const uint32_t *depths,
    uint32_t numTargets,
    uint32_t defaultDepth) {
  if (unreachable_)
    return;

  // Pop the index value.
  Value *index = pop();

  // For each target (including the default), we need to create a trampoline
  // block that adds phi operands to the target's continuation block and then
  // branches there. We use a SwitchInst to dispatch to these trampolines.

  // Collect all unique depths and create trampoline blocks.
  // Multiple case values may share the same depth. We can reuse the same
  // trampoline block for cases with the same depth.
  llvh::DenseMap<uint32_t, BasicBlock *> depthToTrampoline;

  auto getOrCreateTrampoline = [&](uint32_t depth) -> BasicBlock * {
    auto it = depthToTrampoline.find(depth);
    if (it != depthToTrampoline.end())
      return it->second;
    auto *trampoline = builder_.createBasicBlock(currentFunc_);
    depthToTrampoline[depth] = trampoline;
    return trampoline;
  };

  // Create trampoline blocks for all targets.
  llvh::SmallVector<Literal *, 8> caseValues;
  llvh::SmallVector<BasicBlock *, 8> caseBlocks;
  for (uint32_t i = 0; i < numTargets; ++i) {
    caseValues.push_back(builder_.getLiteralNumber(static_cast<double>(i)));
    caseBlocks.push_back(getOrCreateTrampoline(depths[i]));
  }

  BasicBlock *defaultTrampoline = getOrCreateTrampoline(defaultDepth);

  // Emit the SwitchInst.
  builder_.createSwitchInst(index, defaultTrampoline, caseValues, caseBlocks);

  // Now populate each trampoline block with phi operands and branch.
  for (auto &pair : depthToTrampoline) {
    uint32_t depth = pair.first;
    BasicBlock *trampoline = pair.second;

    ControlEntry &entry = getControlEntry(depth);
    entry.branchTargeted = true;

    builder_.setInsertionBlock(trampoline);

    // Add phi operands. For Block/If entries, peek at the value stack and
    // add values as phi incoming edges (the values were on the stack before
    // the index was popped, so they're still there). For Loop entries,
    // no phi operands (br to loop targets the header).
    if ((entry.kind == ControlEntry::Block ||
         entry.kind == ControlEntry::If) &&
        !entry.resultPhis.empty()) {
      size_t numPhis = entry.resultPhis.size();
      size_t available = valueStack_.size();
      for (size_t i = 0; i < numPhis; ++i) {
        Value *val;
        if (available >= numPhis) {
          val = valueStack_[available - numPhis + i];
        } else {
          val = builder_.getLiteralUndefined();
        }
        entry.resultPhis[i]->addEntry(val, trampoline);
      }
    }

    builder_.createBranchInst(entry.contBlock);
  }

  // After br_table, code is unreachable.
  unreachable_ = true;

  // Create a new dead basic block for any dead code that follows.
  auto *deadBlock = builder_.createBasicBlock(currentFunc_);
  builder_.setInsertionBlock(deadBlock);
}

// --- Parametric instructions (D.10) ---

void WasmIRGen::onSelect() {
  if (unreachable_)
    return;

  Value *cond = pop();

  // Check if the values are i64 (val2 is on top, then val1 below).
  bool isI64 = isTopI64();

  if (isI64) {
    auto [lo2, hi2] = popI64(); // value if cond == 0 (false)
    auto [lo1, hi1] = popI64(); // value if cond != 0 (true)

    auto *trueBlock = builder_.createBasicBlock(currentFunc_);
    auto *falseBlock = builder_.createBasicBlock(currentFunc_);
    auto *mergeBlock = builder_.createBasicBlock(currentFunc_);

    builder_.createCondBranchInst(cond, trueBlock, falseBlock);

    builder_.setInsertionBlock(trueBlock);
    builder_.createBranchInst(mergeBlock);

    builder_.setInsertionBlock(falseBlock);
    builder_.createBranchInst(mergeBlock);

    builder_.setInsertionBlock(mergeBlock);
    auto *phiLo = builder_.createPhiInst();
    phiLo->addEntry(lo1, trueBlock);
    phiLo->addEntry(lo2, falseBlock);
    auto *phiHi = builder_.createPhiInst();
    phiHi->addEntry(hi1, trueBlock);
    phiHi->addEntry(hi2, falseBlock);

    pushI64(phiLo, phiHi);
  } else {
    Value *val2 = pop(); // value if cond == 0 (false)
    Value *val1 = pop(); // value if cond != 0 (true)

    auto *trueBlock = builder_.createBasicBlock(currentFunc_);
    auto *falseBlock = builder_.createBasicBlock(currentFunc_);
    auto *mergeBlock = builder_.createBasicBlock(currentFunc_);

    builder_.createCondBranchInst(cond, trueBlock, falseBlock);

    builder_.setInsertionBlock(trueBlock);
    builder_.createBranchInst(mergeBlock);

    builder_.setInsertionBlock(falseBlock);
    builder_.createBranchInst(mergeBlock);

    builder_.setInsertionBlock(mergeBlock);
    auto *phi = builder_.createPhiInst();
    phi->addEntry(val1, trueBlock);
    phi->addEntry(val2, falseBlock);

    push(phi);
  }
}

// --- Function calls (D.12) ---

void WasmIRGen::onCall(uint32_t funcIndex) {
  if (unreachable_)
    return;

  assert(
      funcIndex < irFunctions_.size() &&
      "call funcIndex out of range");

  // Look up the called function's type signature.
  const WasmFuncType &funcType = moduleInfo_.getFunctionType(funcIndex);

  // Pop arguments from the value stack in reverse order.
  // Wasm pushes args left-to-right, so the last arg is on top.
  // i64 params occupy 2 stack slots (lo, hi) and become 2 JS args.
  // First, collect the Wasm-level values in reverse order.
  llvh::SmallVector<Value *, 8> args;
  // Temporary storage for popped values in reverse Wasm param order.
  llvh::SmallVector<std::pair<Value *, Value *>, 8> wasmArgs(
      funcType.params.size());
  for (uint32_t i = funcType.params.size(); i > 0; --i) {
    if (funcType.params[i - 1] == WasmValType::I64) {
      wasmArgs[i - 1] = popI64();
    } else {
      wasmArgs[i - 1] = {pop(), nullptr};
    }
  }
  // Build the JS arg list in forward order.
  for (uint32_t i = 0; i < funcType.params.size(); ++i) {
    if (funcType.params[i] == WasmValType::I64) {
      args.push_back(wasmArgs[i].first); // lo
      args.push_back(wasmArgs[i].second); // hi
    } else {
      args.push_back(wasmArgs[i].first);
    }
  }

  // Load the pre-created closure from the top-level environment.
  auto *closure = builder_.createLoadFrameInst(
      parentScopeInst_, closureVars_[funcIndex]);
  auto *call = builder_.createCallInst(
      closure,
      /* newTarget */ builder_.getLiteralUndefined(),
      /* thisValue */ builder_.getLiteralUndefined(),
      args);

  // Push the return value if the function has a result type.
  if (!funcType.results.empty()) {
    if (funcType.results[0] == WasmValType::I64) {
      // i64 result: lo32 is the call return value, hi32 is in the stash.
      auto *hi = helpers_.emitI64HiResult();
      pushI64(call, hi);
    } else {
      push(call);
    }
  }
}

// --- i64 arithmetic (G.3) ---
// i64 values are represented as two i32 values on the stack [lo, hi].
// Binary operations pop two i64 pairs and push one i64 pair.
// For operations that need a native helper: the helper returns lo32 and
// stashes hi32, which is retrieved via a second call to emitI64HiResult().

void WasmIRGen::onI64Add() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64Add(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64Sub() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64Sub(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64Mul() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64Mul(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64DivS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64DivS(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64DivU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64DivU(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64RemS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64RemS(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64RemU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64RemU(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64And() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = builder_.createBinaryOperatorInst(
      loA, loB, ValueKind::BinaryAndInstKind);
  auto *hi = builder_.createBinaryOperatorInst(
      hiA, hiB, ValueKind::BinaryAndInstKind);
  pushI64(lo, hi);
}

void WasmIRGen::onI64Or() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = builder_.createBinaryOperatorInst(
      loA, loB, ValueKind::BinaryOrInstKind);
  auto *hi = builder_.createBinaryOperatorInst(
      hiA, hiB, ValueKind::BinaryOrInstKind);
  pushI64(lo, hi);
}

void WasmIRGen::onI64Xor() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = builder_.createBinaryOperatorInst(
      loA, loB, ValueKind::BinaryXorInstKind);
  auto *hi = builder_.createBinaryOperatorInst(
      hiA, hiB, ValueKind::BinaryXorInstKind);
  pushI64(lo, hi);
}

void WasmIRGen::onI64Shl() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64Shl(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64ShrS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64ShrS(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64ShrU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64ShrU(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64Rotl() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64Rotl(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64Rotr() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64Rotr(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

// --- i64 unary (G.3) ---
// clz/ctz/popcnt return i64 per the Wasm spec, but the result always fits
// in [0, 64]. The native helper returns a single i32 value. We push it as
// an i64 with hi = 0.

void WasmIRGen::onI64Clz() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  auto *result = helpers_.emitI64Clz(lo, hi);
  pushI64(result, builder_.getLiteralNumber(0));
}

void WasmIRGen::onI64Ctz() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  auto *result = helpers_.emitI64Ctz(lo, hi);
  pushI64(result, builder_.getLiteralNumber(0));
}

void WasmIRGen::onI64Popcnt() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  auto *result = helpers_.emitI64Popcnt(lo, hi);
  pushI64(result, builder_.getLiteralNumber(0));
}

// --- i64 comparisons (G.3) ---
// These take i64 operands and return i32 (0 or 1). The result is pushed
// as a single i32 value (not an i64 pair).

void WasmIRGen::onI64Eqz() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  push(helpers_.emitI64Eqz(lo, hi));
}

void WasmIRGen::onI64Eq() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64Eq(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64Ne() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64Ne(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64LtS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64LtS(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64GtS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64GtS(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64LeS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64LeS(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64GeS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64GeS(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64LtU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64LtU(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64GtU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64GtU(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64LeU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64LeU(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64GeU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64GeU(loA, hiA, loB, hiB));
}

// --- i64 conversions: inline IR (G.4a) ---

void WasmIRGen::onI32WrapI64() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  // i32.wrap_i64: take lo32, discard hi32.
  (void)hi;
  push(lo);
}

void WasmIRGen::onI64ExtendI32S() {
  if (unreachable_)
    return;
  Value *val = pop();
  // Sign-extend i32 to i64: lo = val, hi = (val >> 31).
  auto *hi = builder_.createBinaryOperatorInst(
      builder_.createAsInt32Inst(val),
      builder_.getLiteralNumber(31),
      ValueKind::BinaryRightShiftInstKind);
  pushI64(val, hi);
}

void WasmIRGen::onI64ExtendI32U() {
  if (unreachable_)
    return;
  Value *val = pop();
  // Zero-extend i32 to i64: lo = val, hi = 0.
  pushI64(val, builder_.getLiteralNumber(0));
}

void WasmIRGen::onI64Extend8S() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  (void)hi;
  // Sign-extend lowest 8 bits: lo = (lo << 24) >> 24, hi = (lo >> 31).
  auto *shifted = builder_.createBinaryOperatorInst(
      lo, builder_.getLiteralNumber(24), ValueKind::BinaryLeftShiftInstKind);
  auto *newLo = builder_.createBinaryOperatorInst(
      shifted,
      builder_.getLiteralNumber(24),
      ValueKind::BinaryRightShiftInstKind);
  auto *newHi = builder_.createBinaryOperatorInst(
      newLo,
      builder_.getLiteralNumber(31),
      ValueKind::BinaryRightShiftInstKind);
  pushI64(newLo, newHi);
}

void WasmIRGen::onI64Extend16S() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  (void)hi;
  // Sign-extend lowest 16 bits: lo = (lo << 16) >> 16, hi = (lo >> 31).
  auto *shifted = builder_.createBinaryOperatorInst(
      lo, builder_.getLiteralNumber(16), ValueKind::BinaryLeftShiftInstKind);
  auto *newLo = builder_.createBinaryOperatorInst(
      shifted,
      builder_.getLiteralNumber(16),
      ValueKind::BinaryRightShiftInstKind);
  auto *newHi = builder_.createBinaryOperatorInst(
      newLo,
      builder_.getLiteralNumber(31),
      ValueKind::BinaryRightShiftInstKind);
  pushI64(newLo, newHi);
}

void WasmIRGen::onI64Extend32S() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  (void)hi;
  // Sign-extend lowest 32 bits (i.e., the lo half): hi = (lo >> 31).
  auto *newHi = builder_.createBinaryOperatorInst(
      builder_.createAsInt32Inst(lo),
      builder_.getLiteralNumber(31),
      ValueKind::BinaryRightShiftInstKind);
  pushI64(lo, newHi);
}

// --- f64 arithmetic (E.1) ---
// We use BinaryOperatorInst (not FBinaryMathInst) because the F-prefixed
// instructions require number-typed inputs, but our values are loaded from
// AllocStackInst with :any type. The regular BinaryOperatorInst works
// correctly on number values and can be optimized to F-instructions later.

void WasmIRGen::onF64Add() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryAddInstKind));
}

void WasmIRGen::onF64Sub() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinarySubtractInstKind));
}

void WasmIRGen::onF64Mul() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryMultiplyInstKind));
}

void WasmIRGen::onF64Div() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryDivideInstKind));
}

void WasmIRGen::onF64Neg() {
  Value *val = pop();
  push(builder_.createUnaryOperatorInst(
      val, ValueKind::UnaryMinusInstKind));
}

void WasmIRGen::onF64Abs() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_abs, {val}));
}

void WasmIRGen::onF64Sqrt() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_sqrt, {val}));
}

void WasmIRGen::onF64Ceil() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_ceil, {val}));
}

void WasmIRGen::onF64Floor() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_floor, {val}));
}

void WasmIRGen::onF64Trunc() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_trunc, {val}));
}

void WasmIRGen::onF64Nearest() {
  // Note: Wasm nearest is "round ties to even" (IEEE 754), while Math.round
  // rounds ties to +infinity. This is a known approximation for Phase 1.
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_round, {val}));
}

void WasmIRGen::onF64Min() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_min, {lhs, rhs}));
}

void WasmIRGen::onF64Max() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_max, {lhs, rhs}));
}

// --- f64 comparisons (E.1) ---
// Use BinaryStrictlyEqualInst/etc. (same pattern as i32 comparisons in D.4).
// For float comparisons, strict equality works correctly: NaN !== NaN,
// and the ordering comparisons follow IEEE 754 semantics (NaN comparisons
// return false, which is correct for Wasm).

void WasmIRGen::onF64Eq() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryStrictlyEqualInstKind);
  // Convert boolean to i32 (true→1, false→0) via BitOr with 0.
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF64Ne() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryStrictlyNotEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF64Lt() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryLessThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF64Gt() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryGreaterThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF64Le() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryLessThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF64Ge() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryGreaterThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

// --- f32 arithmetic (E.2) ---
// In Phase 1, f32 operations use f64 precision — we don't have Math.fround
// as a CallBuiltin. Constants are correctly rounded to f32 via onF32Const.
// This means intermediate results may accumulate f64 precision, but the
// overall correctness is acceptable for Phase 1. True f32 rounding will be
// added when Math.fround becomes a Hermes builtin or via Part F helpers.

void WasmIRGen::onF32Add() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryAddInstKind));
}

void WasmIRGen::onF32Sub() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinarySubtractInstKind));
}

void WasmIRGen::onF32Mul() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryMultiplyInstKind));
}

void WasmIRGen::onF32Div() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryDivideInstKind));
}

void WasmIRGen::onF32Neg() {
  Value *val = pop();
  push(builder_.createUnaryOperatorInst(
      val, ValueKind::UnaryMinusInstKind));
}

void WasmIRGen::onF32Abs() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_abs, {val}));
}

void WasmIRGen::onF32Sqrt() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_sqrt, {val}));
}

void WasmIRGen::onF32Ceil() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_ceil, {val}));
}

void WasmIRGen::onF32Floor() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_floor, {val}));
}

void WasmIRGen::onF32Trunc() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_trunc, {val}));
}

void WasmIRGen::onF32Nearest() {
  // Same approximation as f64.nearest: Math.round instead of round-ties-even.
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_round, {val}));
}

void WasmIRGen::onF32Min() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_min, {lhs, rhs}));
}

void WasmIRGen::onF32Max() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_max, {lhs, rhs}));
}

// --- f32 comparisons (E.3) ---
// Same pattern as f64 comparisons. Since values are doubles in Phase 1,
// comparisons work correctly including NaN handling (IEEE 754).

void WasmIRGen::onF32Eq() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryStrictlyEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF32Ne() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryStrictlyNotEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF32Lt() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryLessThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF32Gt() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryGreaterThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF32Le() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryLessThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF32Ge() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryGreaterThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

// --- f64/f32 copysign (F.5) ---

void WasmIRGen::onF64Copysign() {
  if (unreachable_)
    return;
  Value *b = pop();
  Value *a = pop();
  push(helpers_.emitF64Copysign(a, b));
}

void WasmIRGen::onF32Copysign() {
  if (unreachable_)
    return;
  Value *b = pop();
  Value *a = pop();
  push(helpers_.emitF32Copysign(a, b));
}

// --- f64/f32 conversions (E.1, E.2) ---

void WasmIRGen::onF64PromoteF32() {
  // f32 is already represented as double in our Phase 1 implementation,
  // so promotion is a no-op (the value is already f64).
  // Just leave the value on the stack.
}

void WasmIRGen::onF32DemoteF64() {
  // In Phase 1, demote is a no-op because we don't have Math.fround as a
  // CallBuiltin. The value stays at f64 precision. This is a known Phase 1
  // limitation — true f32 rounding will be added in a later phase.
}

// --- Type conversions (F.4) ---

void WasmIRGen::onI32TruncF32S() {
  if (unreachable_)
    return;
  Value *a = pop();
  // In Phase 1, f32 values are doubles — reuse the f64 trapping truncation.
  push(helpers_.emitI32TruncF64S(a));
}

void WasmIRGen::onI32TruncF64S() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32TruncF64S(a));
}

void WasmIRGen::onI32TruncF32U() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32TruncF64U(a));
}

void WasmIRGen::onI32TruncF64U() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32TruncF64U(a));
}

void WasmIRGen::onI32TruncSatF32S() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32TruncSatF64S(a));
}

void WasmIRGen::onI32TruncSatF64S() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32TruncSatF64S(a));
}

void WasmIRGen::onI32TruncSatF32U() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32TruncSatF64U(a));
}

void WasmIRGen::onI32TruncSatF64U() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32TruncSatF64U(a));
}

void WasmIRGen::onF32ConvertI32S() {
  if (unreachable_)
    return;
  // Convert signed i32 to f32.
  // In Phase 1, we simply reinterpret via AsInt32Inst (which ensures the
  // value is treated as signed). The result is a double that exactly
  // represents the int32 value. No Math.fround rounding in Phase 1.
  Value *a = pop();
  push(builder_.createAsInt32Inst(a));
}

void WasmIRGen::onF32ConvertI32U() {
  if (unreachable_)
    return;
  // Convert unsigned i32 to f32.
  // AsUint32Inst ensures the value is treated as unsigned.
  // No Math.fround rounding in Phase 1.
  Value *a = pop();
  push(builder_.createAsUint32Inst(a));
}

void WasmIRGen::onF64ConvertI32S() {
  if (unreachable_)
    return;
  // Convert signed i32 to f64. Double can exactly represent all i32 values.
  Value *a = pop();
  push(builder_.createAsInt32Inst(a));
}

void WasmIRGen::onF64ConvertI32U() {
  if (unreachable_)
    return;
  // Convert unsigned i32 to f64. Double can exactly represent all uint32
  // values.
  Value *a = pop();
  push(builder_.createAsUint32Inst(a));
}

void WasmIRGen::onI32ReinterpretF32() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32ReinterpretF32(a));
}

void WasmIRGen::onF32ReinterpretI32() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitF32ReinterpretI32(a));
}

// --- i64 conversion helpers: float→i64 truncations (G.4b) ---

void WasmIRGen::onI64TruncF64S() {
  if (unreachable_)
    return;
  Value *a = pop();
  Value *lo = helpers_.emitI64TruncF64S(a);
  Value *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64TruncF32S() {
  // Phase 1: all values are doubles, so f32 and f64 truncation are identical.
  onI64TruncF64S();
}

void WasmIRGen::onI64TruncF64U() {
  if (unreachable_)
    return;
  Value *a = pop();
  Value *lo = helpers_.emitI64TruncF64U(a);
  Value *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64TruncF32U() {
  // Phase 1: all values are doubles, so f32 and f64 truncation are identical.
  onI64TruncF64U();
}

void WasmIRGen::onI64TruncSatF64S() {
  if (unreachable_)
    return;
  Value *a = pop();
  Value *lo = helpers_.emitI64TruncSatF64S(a);
  Value *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64TruncSatF32S() {
  // Phase 1: all values are doubles, so f32 and f64 truncation are identical.
  onI64TruncSatF64S();
}

void WasmIRGen::onI64TruncSatF64U() {
  if (unreachable_)
    return;
  Value *a = pop();
  Value *lo = helpers_.emitI64TruncSatF64U(a);
  Value *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64TruncSatF32U() {
  // Phase 1: all values are doubles, so f32 and f64 truncation are identical.
  onI64TruncSatF64U();
}

// --- i64→float conversion helpers (G.4c) ---

void WasmIRGen::onF64ConvertI64S() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  push(helpers_.emitF64ConvertI64S(lo, hi));
}

void WasmIRGen::onF64ConvertI64U() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  push(helpers_.emitF64ConvertI64U(lo, hi));
}

void WasmIRGen::onF32ConvertI64S() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  push(helpers_.emitF32ConvertI64S(lo, hi));
}

void WasmIRGen::onF32ConvertI64U() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  push(helpers_.emitF32ConvertI64U(lo, hi));
}

void WasmIRGen::onI64ReinterpretF64() {
  if (unreachable_)
    return;
  Value *a = pop();
  Value *lo = helpers_.emitI64ReinterpretF64(a);
  Value *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onF64ReinterpretI64() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  push(helpers_.emitF64ReinterpretI64(lo, hi));
}

// --- unreachable and nop (D.11) ---

void WasmIRGen::onUnreachable() {
  if (unreachable_)
    return;

  // Emit a call to the wasmTrap helper, which throws a runtime error.
  helpers_.emitTrap();

  // UnreachableInst serves as the block terminator for IR verification.
  // The trap call above always throws, so this is never reached at runtime.
  builder_.createUnreachableInst();

  // After unreachable, code is dead.
  unreachable_ = true;

  // Create a new dead basic block for any dead code that follows.
  auto *deadBlock = builder_.createBasicBlock(currentFunc_);
  builder_.setInsertionBlock(deadBlock);
}

void WasmIRGen::onNop() {
  // nop does nothing.
}

// --- Unsupported opcode handling (D.13) ---

void WasmIRGen::warnUnsupported(
    const char *opcodeName,
    uint32_t numInputs,
    uint32_t numOutputs) {
  if (unreachable_)
    return;

  llvh::errs() << "warning: unsupported Wasm opcode: " << opcodeName << "\n";

  // Pop the expected number of inputs.
  for (uint32_t i = 0; i < numInputs; ++i) {
    if (!valueStack_.empty())
      pop();
  }

  // Push placeholder undefined values for outputs.
  for (uint32_t i = 0; i < numOutputs; ++i) {
    push(builder_.getLiteralUndefined());
  }
}

// --- Helper methods ---

Value *WasmIRGen::pop() {
  assert(!valueStack_.empty() && "value stack underflow");
  Value *v = valueStack_.back();
  valueStack_.pop_back();
  valueStackIsI64Hi_.pop_back();
  return v;
}

void WasmIRGen::push(Value *v) {
  valueStack_.push_back(v);
  valueStackIsI64Hi_.push_back(false);
}

void WasmIRGen::pushI64(Value *lo, Value *hi) {
  valueStack_.push_back(lo);
  valueStackIsI64Hi_.push_back(false); // lo32 is not the hi part
  valueStack_.push_back(hi);
  valueStackIsI64Hi_.push_back(true); // hi32 is marked
}

std::pair<Value *, Value *> WasmIRGen::popI64() {
  assert(valueStack_.size() >= 2 && "value stack underflow for i64 pop");
  assert(valueStackIsI64Hi_.back() && "expected i64 hi32 on top of stack");
  Value *hi = valueStack_.back();
  valueStack_.pop_back();
  valueStackIsI64Hi_.pop_back();
  assert(!valueStackIsI64Hi_.back() && "expected i64 lo32 below hi32");
  Value *lo = valueStack_.back();
  valueStack_.pop_back();
  valueStackIsI64Hi_.pop_back();
  return {lo, hi};
}

bool WasmIRGen::isTopI64() const {
  return !valueStackIsI64Hi_.empty() && valueStackIsI64Hi_.back();
}

WasmIRGen::ControlEntry &WasmIRGen::getControlEntry(uint32_t depth) {
  assert(depth < controlStack_.size() && "branch depth out of range");
  return controlStack_[controlStack_.size() - 1 - depth];
}

size_t WasmIRGen::numPhisForResultTypes(
    const std::vector<WasmValType> &resultTypes) {
  size_t count = 0;
  for (auto t : resultTypes) {
    count += (t == WasmValType::I64) ? 2 : 1;
  }
  return count;
}

std::vector<PhiInst *> WasmIRGen::createResultPhis(
    BasicBlock *block,
    const std::vector<WasmValType> &resultTypes) {
  std::vector<PhiInst *> phis;
  auto *savedBlock = builder_.getInsertionBlock();
  builder_.setInsertionBlock(block);
  for (auto t : resultTypes) {
    phis.push_back(builder_.createPhiInst());
    if (t == WasmValType::I64) {
      phis.push_back(builder_.createPhiInst());
    }
  }
  builder_.setInsertionBlock(savedBlock);
  return phis;
}

void WasmIRGen::addBranchPhiOperands(ControlEntry &entry) {
  if ((entry.kind == ControlEntry::Block || entry.kind == ControlEntry::If) &&
      !entry.resultPhis.empty()) {
    auto *currentBlock = builder_.getInsertionBlock();
    size_t numPhis = entry.resultPhis.size();
    // The number of stack slots consumed equals the number of phis
    // (i64 results have 2 phis and 2 stack slots).
    size_t available = valueStack_.size();

    if (available >= numPhis) {
      for (size_t i = 0; i < numPhis; ++i) {
        Value *val = valueStack_[available - numPhis + i];
        entry.resultPhis[i]->addEntry(val, currentBlock);
      }
      valueStack_.resize(available - numPhis);
      valueStackIsI64Hi_.resize(available - numPhis);
    } else {
      // Stack underflow — use undefined as placeholder.
      for (size_t i = 0; i < numPhis; ++i) {
        Value *val = (i >= numPhis - available)
            ? valueStack_[i - (numPhis - available)]
            : builder_.getLiteralUndefined();
        entry.resultPhis[i]->addEntry(val, currentBlock);
      }
      valueStack_.clear();
      valueStackIsI64Hi_.clear();
    }
  }
  // For Loop entries, br targets the loop header. Loop phis are for
  // loop parameters which will be handled in D.7.
}

void WasmIRGen::peekBranchPhiOperands(ControlEntry &entry) {
  if ((entry.kind == ControlEntry::Block || entry.kind == ControlEntry::If) &&
      !entry.resultPhis.empty()) {
    size_t numPhis = entry.resultPhis.size();
    size_t available = valueStack_.size();
    auto *currentBlock = builder_.getInsertionBlock();
    for (size_t i = 0; i < numPhis; ++i) {
      Value *val;
      if (available >= numPhis) {
        val = valueStack_[available - numPhis + i];
      } else {
        val = builder_.getLiteralUndefined();
      }
      entry.resultPhis[i]->addEntry(val, currentBlock);
    }
  }
}

void WasmIRGen::pushResultPhis(const ControlEntry &entry) {
  size_t phiIdx = 0;
  for (auto t : entry.resultTypes) {
    if (t == WasmValType::I64) {
      assert(phiIdx + 1 < entry.resultPhis.size());
      pushI64(entry.resultPhis[phiIdx], entry.resultPhis[phiIdx + 1]);
      phiIdx += 2;
    } else {
      assert(phiIdx < entry.resultPhis.size());
      push(entry.resultPhis[phiIdx]);
      phiIdx += 1;
    }
  }
}

bool WasmIRGen::isCurrentBlockTerminated() {
  auto *bb = builder_.getInsertionBlock();
  if (!bb || bb->empty())
    return false;
  return llvh::isa<TerminatorInst>(&bb->back());
}

// --- Memory access (H.1) ---

Value *WasmIRGen::emitNew(Value *constructor, llvh::ArrayRef<Value *> args) {
  auto *thisArg = builder_.createCreateThisInst(constructor, constructor);
  auto *call = builder_.createCallInst(
      constructor, constructor, thisArg, args);
  return builder_.createGetConstructedObjectInst(thisArg, call);
}

void WasmIRGen::createMemoryViews(Instruction *tlScope) {
  // Determine initial memory size in bytes.
  // Use the first memory (Wasm MVP has at most one).
  uint32_t initialPages = 0;
  if (!moduleInfo_.memories.empty()) {
    initialPages = moduleInfo_.memories[0].limits.initial;
  } else {
    // Check imported memories.
    for (auto &imp : moduleInfo_.imports) {
      if (imp.kind == WasmExternalKind::Memory) {
        initialPages = imp.memoryType.limits.initial;
        break;
      }
    }
  }

  auto *memSize = builder_.getLiteralNumber(
      static_cast<double>(initialPages) * 65536.0);

  // Create: var buffer = new ArrayBuffer(memSize)
  auto *abCtor = builder_.createTryLoadGlobalPropertyInst("ArrayBuffer");
  auto *buffer = emitNew(abCtor, {memSize});

  // Create typed array views and store in top-level scope variables.
  static const char *ctorNames[NUM_MEM_VIEWS] = {
      "Int8Array",
      "Uint8Array",
      "Int16Array",
      "Uint16Array",
      "Int32Array",
      "Uint32Array",
      "Float32Array",
      "Float64Array",
  };
  for (uint8_t i = 0; i < NUM_MEM_VIEWS; ++i) {
    auto *ctor = builder_.createTryLoadGlobalPropertyInst(ctorNames[i]);
    auto *view = emitNew(ctor, {buffer});
    builder_.createStoreFrameInst(tlScope, view, memViewVars_[i]);
  }
}

Value *WasmIRGen::loadMemView(MemView view) {
  return builder_.createLoadFrameInst(parentScopeInst_, memViewVars_[view]);
}

void WasmIRGen::onLoad(
    const char *opcodeName,
    uint32_t alignLog2,
    uint32_t offset) {
  if (unreachable_)
    return;

  // Pop the base address.
  Value *base = pop();

  // Compute effective address: base + offset.
  Value *addr;
  if (offset != 0) {
    addr = builder_.createBinaryOperatorInst(
        base,
        builder_.getLiteralNumber(static_cast<double>(offset)),
        ValueKind::BinaryAddInstKind);
  } else {
    addr = base;
  }

  // Determine which view to use and the element shift based on the opcode.
  llvh::StringRef op(opcodeName);

  // i64 loads: handled specially (split into lo/hi).
  if (op == "i64.load") {
    // Load two consecutive i32 values from HEAPU32.
    auto *view = loadMemView(HEAPU32);
    auto *idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(2),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    auto *lo = builder_.createLoadPropertyInst(view, idx);
    // Check OOB: if result === undefined, trap.
    auto *isUndef = builder_.createBinaryOperatorInst(
        lo,
        builder_.getLiteralUndefined(),
        ValueKind::BinaryStrictlyEqualInstKind);
    auto *trapBlock = builder_.createBasicBlock(currentFunc_);
    auto *okBlock = builder_.createBasicBlock(currentFunc_);
    builder_.createCondBranchInst(isUndef, trapBlock, okBlock);

    builder_.setInsertionBlock(trapBlock);
    helpers_.emitTrap();
    builder_.createUnreachableInst();

    builder_.setInsertionBlock(okBlock);
    // Load the hi32 word at idx+1.
    auto *idx1 = builder_.createBinaryOperatorInst(
        idx,
        builder_.getLiteralNumber(1),
        ValueKind::BinaryAddInstKind);
    auto *hi = builder_.createLoadPropertyInst(view, idx1);
    pushI64(lo, hi);
    return;
  }

  if (op == "i64.load8_s" || op == "i64.load8_u") {
    bool isSigned = (op == "i64.load8_s");
    auto *view = loadMemView(isSigned ? HEAP8 : HEAPU8);
    auto *lo = builder_.createLoadPropertyInst(view, addr);
    // OOB check.
    auto *isUndef = builder_.createBinaryOperatorInst(
        lo,
        builder_.getLiteralUndefined(),
        ValueKind::BinaryStrictlyEqualInstKind);
    auto *trapBlock = builder_.createBasicBlock(currentFunc_);
    auto *okBlock = builder_.createBasicBlock(currentFunc_);
    builder_.createCondBranchInst(isUndef, trapBlock, okBlock);

    builder_.setInsertionBlock(trapBlock);
    helpers_.emitTrap();
    builder_.createUnreachableInst();

    builder_.setInsertionBlock(okBlock);
    // Result as i32 (lower byte), zero or sign extended.
    auto *loI32 = builder_.createAsInt32Inst(lo);
    // For i64: hi = sign-extended bits or 0.
    Value *hi;
    if (isSigned) {
      // Sign-extend: hi = lo >> 31 (arithmetic shift fills with sign bit).
      hi = builder_.createBinaryOperatorInst(
          loI32,
          builder_.getLiteralNumber(31),
          ValueKind::BinaryRightShiftInstKind);
    } else {
      hi = builder_.getLiteralNumber(0);
    }
    pushI64(loI32, hi);
    return;
  }

  if (op == "i64.load16_s" || op == "i64.load16_u") {
    bool isSigned = (op == "i64.load16_s");
    auto *view = loadMemView(isSigned ? HEAP16 : HEAPU16);
    auto *idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(1),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    auto *lo = builder_.createLoadPropertyInst(view, idx);
    auto *isUndef = builder_.createBinaryOperatorInst(
        lo,
        builder_.getLiteralUndefined(),
        ValueKind::BinaryStrictlyEqualInstKind);
    auto *trapBlock = builder_.createBasicBlock(currentFunc_);
    auto *okBlock = builder_.createBasicBlock(currentFunc_);
    builder_.createCondBranchInst(isUndef, trapBlock, okBlock);

    builder_.setInsertionBlock(trapBlock);
    helpers_.emitTrap();
    builder_.createUnreachableInst();

    builder_.setInsertionBlock(okBlock);
    auto *loI32 = builder_.createAsInt32Inst(lo);
    Value *hi;
    if (isSigned) {
      hi = builder_.createBinaryOperatorInst(
          loI32,
          builder_.getLiteralNumber(31),
          ValueKind::BinaryRightShiftInstKind);
    } else {
      hi = builder_.getLiteralNumber(0);
    }
    pushI64(loI32, hi);
    return;
  }

  if (op == "i64.load32_s" || op == "i64.load32_u") {
    bool isSigned = (op == "i64.load32_s");
    auto *view = loadMemView(isSigned ? HEAP32 : HEAPU32);
    auto *idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(2),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    auto *lo = builder_.createLoadPropertyInst(view, idx);
    auto *isUndef = builder_.createBinaryOperatorInst(
        lo,
        builder_.getLiteralUndefined(),
        ValueKind::BinaryStrictlyEqualInstKind);
    auto *trapBlock = builder_.createBasicBlock(currentFunc_);
    auto *okBlock = builder_.createBasicBlock(currentFunc_);
    builder_.createCondBranchInst(isUndef, trapBlock, okBlock);

    builder_.setInsertionBlock(trapBlock);
    helpers_.emitTrap();
    builder_.createUnreachableInst();

    builder_.setInsertionBlock(okBlock);
    Value *hi;
    if (isSigned) {
      // The HEAP32 (Int32Array) already returns a signed i32.
      // Sign-extend hi from bit 31.
      auto *loI32 = builder_.createAsInt32Inst(lo);
      hi = builder_.createBinaryOperatorInst(
          loI32,
          builder_.getLiteralNumber(31),
          ValueKind::BinaryRightShiftInstKind);
    } else {
      hi = builder_.getLiteralNumber(0);
    }
    pushI64(lo, hi);
    return;
  }

  if (op == "i64.store" || op == "i64.store8" || op == "i64.store16" ||
      op == "i64.store32") {
    // This is actually a load dispatch error — stores go through onStore.
    // But just in case, handle gracefully.
    assert(false && "i64 store dispatched to onLoad");
    return;
  }

  // Non-i64 loads.
  MemView view;
  uint8_t shift = 0; // log2(element_size)

  if (op == "i32.load") {
    view = HEAP32;
    shift = 2;
  } else if (op == "i32.load8_s") {
    // Int8Array returns signed values natively.
    view = HEAP8;
    shift = 0;
  } else if (op == "i32.load8_u") {
    view = HEAPU8;
    shift = 0;
  } else if (op == "i32.load16_s") {
    // Int16Array returns signed values natively.
    view = HEAP16;
    shift = 1;
  } else if (op == "i32.load16_u") {
    view = HEAPU16;
    shift = 1;
  } else if (op == "f32.load") {
    view = HEAPF32;
    shift = 2;
  } else if (op == "f64.load") {
    view = HEAPF64;
    shift = 3;
  } else {
    // Unknown load opcode.
    llvh::errs() << "WARNING: unsupported load opcode: " << op << "\n";
    push(builder_.getLiteralUndefined());
    return;
  }

  // Compute typed array index.
  Value *idx;
  if (shift > 0) {
    idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(shift),
        ValueKind::BinaryUnsignedRightShiftInstKind);
  } else {
    idx = addr;
  }

  // Load from the typed array view.
  auto *viewVal = loadMemView(view);
  auto *loaded = builder_.createLoadPropertyInst(viewVal, idx);

  // OOB check: typed arrays return undefined for out-of-bounds reads.
  auto *isUndef = builder_.createBinaryOperatorInst(
      loaded,
      builder_.getLiteralUndefined(),
      ValueKind::BinaryStrictlyEqualInstKind);
  auto *trapBlock = builder_.createBasicBlock(currentFunc_);
  auto *okBlock = builder_.createBasicBlock(currentFunc_);
  builder_.createCondBranchInst(isUndef, trapBlock, okBlock);

  builder_.setInsertionBlock(trapBlock);
  helpers_.emitTrap();
  builder_.createUnreachableInst();

  builder_.setInsertionBlock(okBlock);
  push(loaded);
}

void WasmIRGen::onStore(
    const char *opcodeName,
    uint32_t alignLog2,
    uint32_t offset) {
  if (unreachable_)
    return;

  llvh::StringRef op(opcodeName);

  // i64 stores: pop i64 pair (lo, hi) then base address.
  if (op == "i64.store") {
    auto [lo, hi] = popI64();
    Value *base = pop();
    Value *addr;
    if (offset != 0) {
      addr = builder_.createBinaryOperatorInst(
          base,
          builder_.getLiteralNumber(static_cast<double>(offset)),
          ValueKind::BinaryAddInstKind);
    } else {
      addr = base;
    }
    auto *view = loadMemView(HEAPU32);
    auto *idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(2),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    builder_.createStorePropertyStrictInst(lo, view, idx);
    auto *idx1 = builder_.createBinaryOperatorInst(
        idx,
        builder_.getLiteralNumber(1),
        ValueKind::BinaryAddInstKind);
    builder_.createStorePropertyStrictInst(hi, view, idx1);
    return;
  }

  if (op == "i64.store8") {
    auto [lo, hi] = popI64();
    (void)hi;
    Value *base = pop();
    Value *addr;
    if (offset != 0) {
      addr = builder_.createBinaryOperatorInst(
          base,
          builder_.getLiteralNumber(static_cast<double>(offset)),
          ValueKind::BinaryAddInstKind);
    } else {
      addr = base;
    }
    auto *view = loadMemView(HEAPU8);
    builder_.createStorePropertyStrictInst(lo, view, addr);
    return;
  }

  if (op == "i64.store16") {
    auto [lo, hi] = popI64();
    (void)hi;
    Value *base = pop();
    Value *addr;
    if (offset != 0) {
      addr = builder_.createBinaryOperatorInst(
          base,
          builder_.getLiteralNumber(static_cast<double>(offset)),
          ValueKind::BinaryAddInstKind);
    } else {
      addr = base;
    }
    auto *view = loadMemView(HEAPU16);
    auto *idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(1),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    builder_.createStorePropertyStrictInst(lo, view, idx);
    return;
  }

  if (op == "i64.store32") {
    auto [lo, hi] = popI64();
    (void)hi;
    Value *base = pop();
    Value *addr;
    if (offset != 0) {
      addr = builder_.createBinaryOperatorInst(
          base,
          builder_.getLiteralNumber(static_cast<double>(offset)),
          ValueKind::BinaryAddInstKind);
    } else {
      addr = base;
    }
    auto *view = loadMemView(HEAPU32);
    auto *idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(2),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    builder_.createStorePropertyStrictInst(lo, view, idx);
    return;
  }

  // Non-i64 stores: pop value, then base address.
  Value *value = pop();
  Value *base = pop();

  Value *addr;
  if (offset != 0) {
    addr = builder_.createBinaryOperatorInst(
        base,
        builder_.getLiteralNumber(static_cast<double>(offset)),
        ValueKind::BinaryAddInstKind);
  } else {
    addr = base;
  }

  MemView view;
  uint8_t shift = 0;

  if (op == "i32.store") {
    view = HEAP32;
    shift = 2;
  } else if (op == "i32.store8") {
    view = HEAPU8;
    shift = 0;
  } else if (op == "i32.store16") {
    view = HEAPU16;
    shift = 1;
  } else if (op == "f32.store") {
    view = HEAPF32;
    shift = 2;
  } else if (op == "f64.store") {
    view = HEAPF64;
    shift = 3;
  } else {
    llvh::errs() << "WARNING: unsupported store opcode: " << op << "\n";
    return;
  }

  Value *idx;
  if (shift > 0) {
    idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(shift),
        ValueKind::BinaryUnsignedRightShiftInstKind);
  } else {
    idx = addr;
  }

  auto *viewVal = loadMemView(view);
  builder_.createStorePropertyStrictInst(value, viewVal, idx);
}

// --- Memory size/grow (H.2) ---

void WasmIRGen::onMemorySize() {
  if (unreachable_)
    return;

  // Load the HEAPU8 view and get its .length property.
  auto *heapu8 = loadMemView(HEAPU8);
  auto *len = builder_.createLoadPropertyInst(
      heapu8, builder_.getLiteralString("length"));

  // pages = length >>> 16 (divide by 65536).
  auto *pages = builder_.createBinaryOperatorInst(
      len,
      builder_.getLiteralNumber(16),
      ValueKind::BinaryUnsignedRightShiftInstKind);

  push(pages);
}

void WasmIRGen::onMemoryGrow() {
  if (unreachable_)
    return;

  // Pop delta (number of pages to grow).
  Value *delta = pop();

  // Load the HEAPU8 view.
  auto *heapu8 = loadMemView(HEAPU8);

  // Compute old page count: oldPages = HEAPU8.length >>> 16.
  auto *len = builder_.createLoadPropertyInst(
      heapu8, builder_.getLiteralString("length"));
  auto *oldPages = builder_.createBinaryOperatorInst(
      len,
      builder_.getLiteralNumber(16),
      ValueKind::BinaryUnsignedRightShiftInstKind);

  // Get maximum page count from module info.
  uint32_t maxPages = 65536; // Default: 4GB (max Wasm memory).
  if (!moduleInfo_.memories.empty()) {
    if (moduleInfo_.memories[0].limits.hasMaximum) {
      maxPages = moduleInfo_.memories[0].limits.maximum;
    }
  } else {
    for (auto &imp : moduleInfo_.imports) {
      if (imp.kind == WasmExternalKind::Memory) {
        if (imp.memoryType.limits.hasMaximum) {
          maxPages = imp.memoryType.limits.maximum;
        }
        break;
      }
    }
  }

  // Call the grow builtin: wasmMemoryGrow(heapu8, delta, maxPages).
  // Returns new ArrayBuffer on success, or -1 on failure.
  auto *result = helpers_.emitMemoryGrow(
      heapu8, delta, builder_.getLiteralNumber(maxPages));

  // Check if result === -1 (failure).
  auto *negOne = builder_.getLiteralNumber(-1);
  auto *isFailure = builder_.createBinaryOperatorInst(
      result, negOne, ValueKind::BinaryStrictlyEqualInstKind);

  // Create blocks for the conditional.
  auto *successBlock = builder_.createBasicBlock(currentFunc_);
  auto *failBlock = builder_.createBasicBlock(currentFunc_);
  auto *mergeBlock = builder_.createBasicBlock(currentFunc_);

  builder_.createCondBranchInst(isFailure, failBlock, successBlock);

  // --- Fail block: push -1 ---
  builder_.setInsertionBlock(failBlock);
  auto *failVal = builder_.getLiteralNumber(-1);
  builder_.createBranchInst(mergeBlock);

  // --- Success block: create new views and store them ---
  builder_.setInsertionBlock(successBlock);

  // result is the new ArrayBuffer. Create 8 typed array views from it.
  static const char *ctorNames[NUM_MEM_VIEWS] = {
      "Int8Array",
      "Uint8Array",
      "Int16Array",
      "Uint16Array",
      "Int32Array",
      "Uint32Array",
      "Float32Array",
      "Float64Array",
  };
  for (uint8_t i = 0; i < NUM_MEM_VIEWS; ++i) {
    auto *ctor = builder_.createTryLoadGlobalPropertyInst(ctorNames[i]);
    auto *view = emitNew(ctor, {result});
    builder_.createStoreFrameInst(parentScopeInst_, view, memViewVars_[i]);
  }

  // oldPages was computed before the branch; use it as the success value.
  builder_.createBranchInst(mergeBlock);

  // --- Merge block: phi for the result ---
  builder_.setInsertionBlock(mergeBlock);
  auto *phi = builder_.createPhiInst();
  phi->addEntry(failVal, failBlock);
  phi->addEntry(oldPages, successBlock);

  push(phi);
}

} // namespace wasm
} // namespace hermes
