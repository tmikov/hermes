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
#include "hermes/WasmIRGen/WasmHelpers.h"

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

  /// Create Hermes IR Functions for all Wasm functions (imported + defined),
  /// plus a top-level wrapper function that serves as the module entry point.
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

  // --- i64 value stack helpers (G.1) ---

  /// Push an i64 value as two stack slots: lo32 first, then hi32.
  void pushI64(Value *lo, Value *hi);
  /// Pop an i64 value from the stack (hi32 first, then lo32).
  /// \return {lo, hi}.
  std::pair<Value *, Value *> popI64();

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

  // --- i32 trapping division (F.2) ---

  void onI32DivS();
  void onI32DivU();
  void onI32RemS();
  void onI32RemU();

  // --- i32 bit manipulation (F.3) ---

  void onI32Clz();
  void onI32Ctz();
  void onI32Popcnt();
  void onI32Rotl();
  void onI32Rotr();
  void onI32Extend8S();
  void onI32Extend16S();

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
  /// Table branch (switch): pop index, branch to labels[index] or default.
  void onBrTable(
      const uint32_t *depths,
      uint32_t numTargets,
      uint32_t defaultDepth);

  // --- Parametric instructions (D.10) ---

  /// select: pop condition, val2, val1; push (cond ? val1 : val2).
  void onSelect();

  // --- Function calls (D.12) ---

  /// Call the function at \p funcIndex with arguments from the value stack.
  void onCall(uint32_t funcIndex);

  // --- unreachable and nop (D.11) ---

  /// Emit an UnreachableInst (Wasm trap).
  void onUnreachable();
  /// No-op instruction (nothing is emitted).
  void onNop();

  // --- f64 arithmetic (E.1) ---

  void onF64Add();
  void onF64Sub();
  void onF64Mul();
  void onF64Div();
  void onF64Neg();
  void onF64Abs();
  void onF64Sqrt();
  void onF64Ceil();
  void onF64Floor();
  void onF64Trunc();
  void onF64Nearest();
  void onF64Min();
  void onF64Max();

  // --- f64 comparisons (E.1) ---

  void onF64Eq();
  void onF64Ne();
  void onF64Lt();
  void onF64Gt();
  void onF64Le();
  void onF64Ge();

  // --- f32 arithmetic (E.2) ---
  // In Phase 1, f32 operations use f64 precision (no per-op rounding).
  // Constants are correctly rounded via float cast in onF32Const.

  void onF32Add();
  void onF32Sub();
  void onF32Mul();
  void onF32Div();
  void onF32Neg();
  void onF32Abs();
  void onF32Sqrt();
  void onF32Ceil();
  void onF32Floor();
  void onF32Trunc();
  void onF32Nearest();
  void onF32Min();
  void onF32Max();

  // --- f32 comparisons (E.3) ---

  void onF32Eq();
  void onF32Ne();
  void onF32Lt();
  void onF32Gt();
  void onF32Le();
  void onF32Ge();

  // --- i64 arithmetic (G.3) ---

  void onI64Add();
  void onI64Sub();
  void onI64Mul();
  void onI64DivS();
  void onI64DivU();
  void onI64RemS();
  void onI64RemU();
  void onI64And();
  void onI64Or();
  void onI64Xor();
  void onI64Shl();
  void onI64ShrS();
  void onI64ShrU();
  void onI64Rotl();
  void onI64Rotr();

  // --- i64 unary (G.3) ---

  void onI64Clz();
  void onI64Ctz();
  void onI64Popcnt();

  // --- i64 comparisons (G.3) ---

  void onI64Eqz();
  void onI64Eq();
  void onI64Ne();
  void onI64LtS();
  void onI64GtS();
  void onI64LeS();
  void onI64GeS();
  void onI64LtU();
  void onI64GtU();
  void onI64LeU();
  void onI64GeU();

  // --- i64 conversions: inline IR (G.4a) ---

  /// i32.wrap_i64: pop i64, push lo32 as i32 (discard hi32).
  void onI32WrapI64();
  /// i64.extend_i32_s: pop i32, sign-extend to i64.
  void onI64ExtendI32S();
  /// i64.extend_i32_u: pop i32, zero-extend to i64.
  void onI64ExtendI32U();
  /// i64.extend8_s: sign-extend lowest 8 bits of i64.
  void onI64Extend8S();
  /// i64.extend16_s: sign-extend lowest 16 bits of i64.
  void onI64Extend16S();
  /// i64.extend32_s: sign-extend lowest 32 bits of i64.
  void onI64Extend32S();

  // --- f64/f32 copysign (F.5) ---

  void onF64Copysign();
  void onF32Copysign();

  // --- f64/f32 conversions (E.1, E.2) ---

  void onF64PromoteF32();
  void onF32DemoteF64();

  // --- Type conversions (F.4) ---

  /// Trapping truncation: float/double to signed i32.
  void onI32TruncF32S();
  void onI32TruncF64S();
  /// Trapping truncation: float/double to unsigned i32.
  void onI32TruncF32U();
  void onI32TruncF64U();
  /// Saturating truncation: float/double to signed i32.
  void onI32TruncSatF32S();
  void onI32TruncSatF64S();
  /// Saturating truncation: float/double to unsigned i32.
  void onI32TruncSatF32U();
  void onI32TruncSatF64U();
  /// Int-to-float conversion.
  void onF32ConvertI32S();
  void onF32ConvertI32U();
  void onF64ConvertI32S();
  void onF64ConvertI32U();
  /// Reinterpret (bitcast).
  void onI32ReinterpretF32();
  void onF32ReinterpretI32();

  // --- Unsupported opcode handling (D.13) ---

  /// Emit a warning for an unsupported opcode. Pops \p numInputs values
  /// from the stack and pushes \p numOutputs placeholder values.
  void warnUnsupported(
      const char *opcodeName,
      uint32_t numInputs,
      uint32_t numOutputs);

  /// \return the array of IR Functions created by createFunctions(), indexed
  ///   by Wasm function index.
  llvh::ArrayRef<Function *> getIRFunctions() const {
    return irFunctions_;
  }

 private:
  WasmModuleInfo &moduleInfo_;
  IRBuilder builder_;
  WasmHelpers helpers_;

  /// One IR Function per Wasm function, indexed by Wasm function index.
  /// Includes both imported and defined functions.
  std::vector<Function *> irFunctions_;

  /// One Variable per Wasm function in the top-level scope, holding the
  /// pre-created closure. Indexed by Wasm function index.
  std::vector<Variable *> closureVars_;

  /// The VariableScope for the top-level function.
  VariableScope *topLevelVS_ = nullptr;

  // --- Per-function state (valid between beginFunction/endFunction) ---

  /// The current Hermes IR function being built.
  Function *currentFunc_ = nullptr;

  /// The Wasm function index of the current function.
  uint32_t currentFuncIndex_ = 0;

  /// Abstract value stack: stack of Value* (Hermes IR SSA values).
  /// For i64 values, two consecutive slots are used: [lo32, hi32].
  std::vector<Value *> valueStack_;

  /// Parallel to valueStack_: true if the slot is the hi32 part of an i64.
  /// Used by drop and select to determine if a value occupies 2 slots.
  std::vector<bool> valueStackIsI64Hi_;

  /// AllocStackInst for each Wasm local (params + declared locals).
  std::vector<AllocStackInst *> locals_;

  /// The parent (top-level) scope instruction, used to load pre-created
  /// closures from the environment at call sites.
  GetParentScopeInst *parentScopeInst_ = nullptr;

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

  /// Check if the top of the value stack is the hi32 part of an i64.
  bool isTopI64() const;

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
