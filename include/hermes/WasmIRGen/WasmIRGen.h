/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_WASMIRGEN_WASMIRGEN_H
#define HERMES_WASMIRGEN_WASMIRGEN_H

#include "hermes/IR/IRBuilder.h"
#include "hermes/WasmFrontend/WasmModuleInfo.h"

namespace hermes {

class Module;

namespace wasm {

/// Translates a parsed Wasm module into Hermes IR.
///
/// Usage:
///   1. Parse a Wasm binary into a WasmModuleInfo.
///   2. Construct WasmIRGen with the target Module and the WasmModuleInfo.
///   3. Call createFunctions() to create one Hermes IR Function per Wasm
///      function (imported + defined).
///   4. For each defined function body, the binary reader calls
///      beginFunction() / instruction callbacks / endFunction().
class WasmIRGen {
 public:
  WasmIRGen(Module &M, WasmModuleInfo &moduleInfo);

  /// Create Hermes IR Functions for all Wasm functions (imported + defined).
  /// Called once after module-level parsing is complete, before any function
  /// bodies are translated.
  void createFunctions();

  // --- Per-function translation (called by BinaryReaderHermesIRGen) ---

  /// Begin translating a Wasm function body.
  /// \p funcIndex is the Wasm function index (includes imports).
  /// \p localTypes are the types of the function's declared locals (not
  ///    including parameters, which are part of the function signature).
  void beginFunction(
      uint32_t funcIndex,
      const std::vector<WasmValType> &localTypes);

  /// End translating a Wasm function body.
  void endFunction();

  // --- Instruction callbacks (added incrementally in subsequent steps) ---

  /// Push an i32 constant onto the value stack.
  void onI32Const(int32_t value);
  /// Push an i64 constant onto the value stack (split into lo32, hi32).
  void onI64Const(int64_t value);
  /// Push an f32 constant onto the value stack.
  void onF32Const(float value);
  /// Push an f64 constant onto the value stack.
  void onF64Const(double value);

  /// Load a local variable onto the value stack.
  void onLocalGet(uint32_t localIndex);
  /// Pop the value stack and store into a local variable.
  void onLocalSet(uint32_t localIndex);
  /// Store the top of the value stack into a local variable without popping.
  void onLocalTee(uint32_t localIndex);

  // --- i32 arithmetic (D.3) ---

  void onI32Add();
  void onI32Sub();
  void onI32Mul();
  void onI32And();
  void onI32Or();
  void onI32Xor();
  void onI32Shl();
  void onI32ShrS();
  void onI32ShrU();

  // --- Return and drop (D.5) ---

  /// Explicit return from the current function.
  void onReturn();
  /// Pop and discard the top value from the stack.
  void onDrop();

  // --- i32 comparisons (D.4) ---

  void onI32Eq();
  void onI32Ne();
  void onI32LtS();
  void onI32GtS();
  void onI32LeS();
  void onI32GeS();
  void onI32LtU();
  void onI32GtU();
  void onI32LeU();
  void onI32GeU();
  void onI32Eqz();

  // --- Control flow (D.6, D.7, D.8) ---

  /// Enter a block with the given result types.
  void onBlock(const std::vector<WasmValType> &resultTypes);
  /// Enter a loop with the given result types.
  void onLoop(const std::vector<WasmValType> &resultTypes);
  /// Enter an if construct with the given result types.
  /// Pops the condition from the value stack.
  void onIf(const std::vector<WasmValType> &resultTypes);
  /// Switch to the else branch of the current if construct.
  void onElse();
  /// End the current block/loop/if.
  void onEnd();
  /// Unconditional branch to the control entry at \p depth.
  void onBr(uint32_t depth);
  /// Conditional branch: pop condition, branch if non-zero.
  void onBrIf(uint32_t depth);

  /// \return the array of IR Functions created by createFunctions(), indexed
  ///   by Wasm function index.
  llvh::ArrayRef<Function *> getIRFunctions() const {
    return irFunctions_;
  }

 private:
  WasmModuleInfo &moduleInfo_;
  IRBuilder builder_;

  /// One IR Function per Wasm function, indexed by Wasm function index.
  /// Includes both imported and defined functions.
  std::vector<Function *> irFunctions_;

  // --- Per-function state (valid between beginFunction/endFunction) ---

  /// The current Hermes IR function being built.
  Function *currentFunc_ = nullptr;

  /// The Wasm function index of the current function.
  uint32_t currentFuncIndex_ = 0;

  /// Abstract value stack: stack of Value* (Hermes IR SSA values).
  std::vector<Value *> valueStack_;

  /// AllocStackInst for each Wasm local (params + declared locals).
  std::vector<AllocStackInst *> locals_;

  /// Whether we are in unreachable code (after an unconditional br, return,
  /// or unreachable). In unreachable mode, instructions are no-ops until
  /// the next end/else that restores reachability.
  bool unreachable_ = false;

  /// Control flow stack (for block/loop/if).
  struct ControlEntry {
    enum Kind { Block, Loop, If };
    Kind kind;
    /// For Block/If: continuation after end (also the br target).
    /// For Loop: the loop header block (the br target).
    BasicBlock *contBlock;
    /// For Loop: the block after the loop's end (where fallthrough goes).
    /// For Block/If: nullptr (contBlock serves both purposes).
    BasicBlock *endBlock = nullptr;
    /// Only for If: the else block.
    BasicBlock *elseBlock = nullptr;
    /// Block signature result types.
    std::vector<WasmValType> resultTypes;
    /// Value stack height at entry.
    size_t stackHeight;
    /// Phi nodes for results at the continuation block.
    /// For Block/If: phis in contBlock for results from br/fallthrough.
    /// For Loop: phis in endBlock for results from fallthrough.
    std::vector<PhiInst *> resultPhis;
    /// Whether the code was unreachable when this entry was pushed.
    bool outerUnreachable = false;
    /// Whether any branch (br/br_if) has targeted this entry's contBlock.
    bool branchTargeted = false;
  };
  std::vector<ControlEntry> controlStack_;

  // --- Helper methods ---

  /// Pop the top value from the value stack.
  Value *pop();
  /// Push a value onto the value stack.
  void push(Value *v);

  /// Get the ControlEntry at the given branch depth.
  ControlEntry &getControlEntry(uint32_t depth);

  /// Add phi operands for branching to the given control entry from the
  /// current block. For Block/If entries, pops result values and adds them
  /// as phi incoming edges. For Loop entries, no phi operands are added
  /// (loop phis are for loop parameters, handled separately).
  void addBranchPhiOperands(ControlEntry &entry);

  /// Check if the current insertion block is terminated (ends with a
  /// terminator instruction).
  bool isCurrentBlockTerminated();
};

} // namespace wasm
} // namespace hermes

#endif // HERMES_WASMIRGEN_WASMIRGEN_H
