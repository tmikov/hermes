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
      Value *result = pop();
      builder_.createReturnInst(result);
    } else {
      builder_.createReturnInst(builder_.getLiteralUndefined());
    }
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

// --- Return and drop (D.5) ---

void WasmIRGen::onReturn() {
  if (unreachable_)
    return;

  const WasmFuncType &funcType =
      moduleInfo_.getFunctionType(currentFuncIndex_);

  if (!funcType.results.empty()) {
    Value *result = pop();
    builder_.createReturnInst(result);
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
  pop();
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

  // If the block has result types, create phi nodes in the continuation block.
  if (!resultTypes.empty()) {
    auto *savedBlock = builder_.getInsertionBlock();
    builder_.setInsertionBlock(contBlock);
    for (size_t i = 0; i < resultTypes.size(); ++i) {
      entry.resultPhis.push_back(builder_.createPhiInst());
    }
    builder_.setInsertionBlock(savedBlock);
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
  if (!resultTypes.empty()) {
    auto *savedBlock = builder_.getInsertionBlock();
    builder_.setInsertionBlock(endBlock);
    for (size_t i = 0; i < resultTypes.size(); ++i) {
      entry.resultPhis.push_back(builder_.createPhiInst());
    }
    builder_.setInsertionBlock(savedBlock);
  }

  // Branch from the current block to the loop header.
  if (!unreachable_ && !isCurrentBlockTerminated()) {
    builder_.createBranchInst(headerBlock);
  }

  // Set insertion point to the loop header.
  builder_.setInsertionBlock(headerBlock);

  controlStack_.push_back(std::move(entry));
}

void WasmIRGen::onEnd() {
  assert(!controlStack_.empty() && "control stack underflow");
  ControlEntry entry = std::move(controlStack_.back());
  controlStack_.pop_back();

  if (entry.kind == ControlEntry::Block) {
    bool fallsThrough = !unreachable_ && !isCurrentBlockTerminated();

    if (fallsThrough) {
      // Add phi operands from the fallthrough path.
      addBranchPhiOperands(entry);
      builder_.createBranchInst(entry.contBlock);
    }

    // Set insertion point to the continuation block.
    builder_.setInsertionBlock(entry.contBlock);

    // The continuation block is reachable if we fell through or if any
    // branch (br/br_if) targeted this block.
    unreachable_ = !fallsThrough && !entry.branchTargeted;

    // Push phi results onto the value stack.
    for (auto *phi : entry.resultPhis) {
      push(phi);
    }
  } else if (entry.kind == ControlEntry::Loop) {
    bool fallsThrough = !unreachable_ && !isCurrentBlockTerminated();

    if (fallsThrough) {
      // Add phi operands to the end block from the fallthrough path.
      // We handle this directly here rather than via addBranchPhiOperands,
      // because addBranchPhiOperands skips Loop entries (since br to a
      // loop targets the header, not the end block).
      if (!entry.resultPhis.empty()) {
        auto *currentBlock = builder_.getInsertionBlock();
        size_t numResults = entry.resultPhis.size();
        size_t available = valueStack_.size();
        if (available >= numResults) {
          for (size_t i = 0; i < numResults; ++i) {
            Value *val = valueStack_[available - numResults + i];
            entry.resultPhis[i]->addEntry(val, currentBlock);
          }
          valueStack_.resize(available - numResults);
        } else {
          // Stack underflow — use undefined as placeholder.
          for (size_t i = 0; i < numResults; ++i) {
            Value *val = (i >= numResults - available)
                ? valueStack_[i - (numResults - available)]
                : builder_.getLiteralUndefined();
            entry.resultPhis[i]->addEntry(val, currentBlock);
          }
          valueStack_.clear();
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

    // Push phi results onto the value stack.
    for (auto *phi : entry.resultPhis) {
      push(phi);
    }
  }
  // If kind will be handled in D.8.
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

  // If the block has results, we need to add phi operands from the
  // branch-taken path. The values for the phi must be read before the
  // branch, but we don't pop them (they stay on the stack for the
  // fallthrough path).
  if (!entry.resultPhis.empty() && entry.kind == ControlEntry::Block) {
    // For br_if targeting a block, the top N values are the results.
    // We peek at them (don't pop) and add them as phi operands.
    size_t numResults = entry.resultPhis.size();
    size_t available = valueStack_.size();
    auto *currentBlock = builder_.getInsertionBlock();
    for (size_t i = 0; i < numResults; ++i) {
      // Peek at the result values (don't pop for br_if).
      Value *val;
      if (available >= numResults) {
        val = valueStack_[available - numResults + i];
      } else {
        val = builder_.getLiteralUndefined();
      }
      entry.resultPhis[i]->addEntry(val, currentBlock);
    }
  }

  // Emit conditional branch: non-zero condition branches to target.
  builder_.createCondBranchInst(cond, entry.contBlock, fallthroughBlock);

  // Continue generating code in the fallthrough block.
  builder_.setInsertionBlock(fallthroughBlock);
}

// --- Helper methods ---

Value *WasmIRGen::pop() {
  assert(!valueStack_.empty() && "value stack underflow");
  Value *v = valueStack_.back();
  valueStack_.pop_back();
  return v;
}

void WasmIRGen::push(Value *v) {
  valueStack_.push_back(v);
}

WasmIRGen::ControlEntry &WasmIRGen::getControlEntry(uint32_t depth) {
  assert(depth < controlStack_.size() && "branch depth out of range");
  return controlStack_[controlStack_.size() - 1 - depth];
}

void WasmIRGen::addBranchPhiOperands(ControlEntry &entry) {
  if (entry.kind == ControlEntry::Block && !entry.resultPhis.empty()) {
    auto *currentBlock = builder_.getInsertionBlock();
    size_t numResults = entry.resultPhis.size();
    size_t available = valueStack_.size();

    if (available >= numResults) {
      // Normal case: pop N result values from the stack.
      for (size_t i = 0; i < numResults; ++i) {
        Value *val = valueStack_[available - numResults + i];
        entry.resultPhis[i]->addEntry(val, currentBlock);
      }
      valueStack_.resize(available - numResults);
    } else {
      // Stack underflow (e.g., due to unimplemented instructions).
      // Use undefined as placeholder for missing values.
      for (size_t i = 0; i < numResults; ++i) {
        Value *val = (i >= numResults - available)
            ? valueStack_[i - (numResults - available)]
            : builder_.getLiteralUndefined();
        entry.resultPhis[i]->addEntry(val, currentBlock);
      }
      valueStack_.clear();
    }
  }
  // For Loop entries, br targets the loop header. Loop phis are for
  // loop parameters which will be handled in D.7.
}

bool WasmIRGen::isCurrentBlockTerminated() {
  auto *bb = builder_.getInsertionBlock();
  if (!bb || bb->empty())
    return false;
  return llvh::isa<TerminatorInst>(&bb->back());
}

} // namespace wasm
} // namespace hermes
