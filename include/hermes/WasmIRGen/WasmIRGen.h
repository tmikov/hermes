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

#include "llvh/ADT/DenseMap.h"

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

  /// Finalize the top-level function after all sections have been parsed.
  /// Applies active data segments, calls the start function, builds the
  /// exports object, and emits the return instruction.
  /// Must be called after createFunctions() and after all function bodies
  /// and data sections have been processed.
  void finalizeModule();

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

  /// Enter a block with the given param and result types.
  void onBlock(
      const std::vector<WasmValType> &paramTypes,
      const std::vector<WasmValType> &resultTypes);
  /// Enter a loop with the given param and result types.
  void onLoop(
      const std::vector<WasmValType> &paramTypes,
      const std::vector<WasmValType> &resultTypes);
  /// Enter an if construct with the given param and result types.
  /// Pops the condition from the value stack.
  void onIf(
      const std::vector<WasmValType> &paramTypes,
      const std::vector<WasmValType> &resultTypes);
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

  // --- Function calls (D.12, J.2) ---

  /// Call the function at \p funcIndex with arguments from the value stack.
  void onCall(uint32_t funcIndex);

  /// Indirect call through a table.
  /// \p sigIndex is the expected type signature index.
  /// \p tableIndex is the table to call from.
  void onCallIndirect(uint32_t sigIndex, uint32_t tableIndex);

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
  // f32 operations produce f32-precision results by wrapping the result
  // in Math.fround via emitFround().

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

  // --- i64 conversion helpers: float→i64 truncations (G.4b) ---

  /// Trapping truncation: float/double to signed i64.
  void onI64TruncF32S();
  void onI64TruncF64S();
  /// Trapping truncation: float/double to unsigned i64.
  void onI64TruncF32U();
  void onI64TruncF64U();
  /// Saturating truncation: float/double to signed i64.
  void onI64TruncSatF32S();
  void onI64TruncSatF64S();
  /// Saturating truncation: float/double to unsigned i64.
  void onI64TruncSatF32U();
  void onI64TruncSatF64U();

  // --- i64 conversion helpers: i64→float and reinterpret (G.4c) ---

  /// f64.convert_i64_s: pop i64, push f64 (signed conversion).
  void onF64ConvertI64S();
  /// f64.convert_i64_u: pop i64, push f64 (unsigned conversion).
  void onF64ConvertI64U();
  /// f32.convert_i64_s: pop i64, push f32 (signed conversion, as double).
  void onF32ConvertI64S();
  /// f32.convert_i64_u: pop i64, push f32 (unsigned conversion, as double).
  void onF32ConvertI64U();
  /// i64.reinterpret_f64: pop f64, push i64 (bitcast).
  void onI64ReinterpretF64();
  /// f64.reinterpret_i64: pop i64, push f64 (bitcast).
  void onF64ReinterpretI64();

  // --- Memory access (H.1) ---

  /// Emit a memory load instruction.
  /// \p opcodeName identifies the load variant (e.g., "i32.load").
  /// \p alignLog2 is the log2 of the alignment annotation.
  /// \p offset is the static offset immediate.
  void onLoad(
      const char *opcodeName,
      uint32_t alignLog2,
      uint32_t offset);

  /// Emit a memory store instruction.
  /// \p opcodeName identifies the store variant (e.g., "i32.store").
  /// \p alignLog2 is the log2 of the alignment annotation.
  /// \p offset is the static offset immediate.
  void onStore(
      const char *opcodeName,
      uint32_t alignLog2,
      uint32_t offset);

  // --- Memory size/grow (H.2) ---

  /// Push the current memory size in pages onto the value stack.
  void onMemorySize();

  /// Pop delta, grow memory by that many pages.
  /// Pushes old page count on success, or -1 on failure.
  void onMemoryGrow();

  // --- Globals (K.1) ---

  /// global.get: push the value of the global at \p globalIndex.
  void onGlobalGet(uint32_t globalIndex);
  /// global.set: pop a value and store it into the global at \p globalIndex.
  void onGlobalSet(uint32_t globalIndex);

  // --- Exception handling (L.1) ---

  /// Enter a try block with the given result types.
  void onTry(const std::vector<WasmValType> &resultTypes);
  /// Handle a catch clause for the given tag index.
  void onCatch(uint32_t tagIndex);
  /// Handle a catch_all clause.
  void onCatchAll();
  /// Throw an exception with the given tag index.
  void onThrow(uint32_t tagIndex);
  /// Re-throw the caught exception from the catch at the given depth.
  void onRethrow(uint32_t depth);
  /// Delegate exceptions to an outer handler at the given depth.
  void onDelegate(uint32_t depth);

  // --- Bulk memory operations (N.1) ---

  /// memory.fill: pop size, value, dest; fill dest..dest+size with value.
  void onMemoryFill();

  /// memory.copy: pop size, src, dest; copy src..src+size to dest.
  void onMemoryCopy();

  /// memory.init: pop size, offset, dest; copy data segment to memory.
  void onMemoryInit(uint32_t segmentIndex);

  /// data.drop: mark data segment as no longer needed.
  void onDataDrop(uint32_t segmentIndex);

  // --- Table operations (J.1) ---

  /// table.get: pop index, push the function reference at that index.
  void onTableGet(uint32_t tableIndex);
  /// table.set: pop value and index, set table[index] = value.
  void onTableSet(uint32_t tableIndex);
  /// table.size: push the current number of entries in the table.
  void onTableSize(uint32_t tableIndex);
  /// table.grow: pop fill value and delta, grow table by delta entries.
  /// Pushes old size on success, or -1 on failure.
  void onTableGrow(uint32_t tableIndex);

  // --- Bulk table operations (N.2) ---

  /// table.fill: pop count, val, idx; fill table entries with val.
  void onTableFill(uint32_t tableIndex);
  /// table.copy: pop count, src, dst; copy entries between tables.
  void onTableCopy(uint32_t dstTableIndex, uint32_t srcTableIndex);
  /// table.init: pop count, src, dst; copy from element segment to table.
  void onTableInit(uint32_t segmentIndex, uint32_t tableIndex);
  /// elem.drop: mark element segment as no longer needed.
  void onElemDrop(uint32_t segmentIndex);

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

  /// One Variable per imported function, holding the JS callable passed
  /// via the imports object. Indexed by import function order (0-based,
  /// same as Wasm function index for imports since they come first).
  std::vector<Variable *> importFuncVars_;

  /// Typed array view indices into memViewVars_.
  enum MemView : uint8_t {
    HEAP8 = 0,
    HEAPU8,
    HEAP16,
    HEAPU16,
    HEAP32,
    HEAPU32,
    HEAPF32,
    HEAPF64,
    NUM_MEM_VIEWS,
  };

  /// Variables holding the 8 typed array views in the top-level scope.
  /// Only populated if the module has a memory section.
  Variable *memViewVars_[NUM_MEM_VIEWS] = {};

  /// Per-table Variables in the top-level scope.
  /// tableFuncVars_[i] holds a JS Array of closures (null = uninitialized).
  /// tableTypeVars_[i] holds a JS Array of type indices (-1 = uninitialized).
  /// Indexed by Wasm table index.
  std::vector<Variable *> tableFuncVars_;
  std::vector<Variable *> tableTypeVars_;

  /// Per-global Variables in the top-level scope.
  /// For non-i64 globals: one Variable per global.
  /// For i64 globals: two consecutive Variables (lo32, hi32).
  /// Use globalSlotIndex_ to map global index → first slot.
  std::vector<Variable *> globalVars_;

  /// Maps Wasm global index → starting index in globalVars_.
  /// For non-i64 globals, globalVars_[globalSlotIndex_[i]] is the single slot.
  /// For i64 globals, globalVars_[globalSlotIndex_[i]] is lo32 and
  /// globalVars_[globalSlotIndex_[i]+1] is hi32.
  std::vector<uint32_t> globalSlotIndex_;

  /// Variable holding a JS Array of data segments in the top-level scope.
  /// Each element is either a Uint8Array (segment bytes) or null (dropped).
  /// Only populated if the module has data segments.
  Variable *dataSegVar_ = nullptr;

  /// Variable holding a JS Array of element segments in the top-level scope.
  /// Each element is either a JS Array of interleaved [func, typeIdx, ...]
  /// or null (dropped). Only populated if the module has element segments.
  Variable *elemSegVar_ = nullptr;

  /// The VariableScope for the top-level function.
  VariableScope *topLevelVS_ = nullptr;

  /// The entry BasicBlock of the top-level function.
  /// Saved by createFunctions() for use by finalizeModule().
  BasicBlock *tlEntry_ = nullptr;

  /// The CreateScopeInst for the top-level function.
  /// Saved by createFunctions() for use by finalizeModule().
  BaseScopeInst *tlScope_ = nullptr;

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

  /// AllocStackInst for each Wasm local slot. For non-i64 locals, there is
  /// one slot per local. For i64 locals, there are two consecutive slots
  /// (lo32, hi32). Use localSlotIndex_ to find the starting slot for a
  /// given Wasm local index.
  std::vector<AllocStackInst *> locals_;

  /// Maps Wasm local index → starting index in locals_.
  /// For non-i64 locals, locals_[localSlotIndex_[i]] is the single slot.
  /// For i64 locals, locals_[localSlotIndex_[i]] is the lo32 slot and
  /// locals_[localSlotIndex_[i]+1] is the hi32 slot.
  std::vector<uint32_t> localSlotIndex_;

  /// Wasm type of each Wasm local (params then declared locals).
  std::vector<WasmValType> localTypes_;

  /// Map from f32 LiteralNumber (promoted to f64) to original f32 bit pattern.
  /// Needed because f32→f64 promotion may alter NaN payload bits, and Hermes
  /// canonicalizes NaN when emitting bytecode. We record the original bits so
  /// that i32.reinterpret_f32 can fold them at compile time.
  llvh::DenseMap<LiteralNumber *, uint32_t> f32NanBitsMap_;

  /// The parent (top-level) scope instruction, used to load pre-created
  /// closures from the environment at call sites.
  GetParentScopeInst *parentScopeInst_ = nullptr;

  /// Whether we are in unreachable code (after an unconditional br, return,
  /// or unreachable). In unreachable mode, instructions are no-ops until
  /// the next end/else that restores reachability.
  bool unreachable_ = false;

  /// Control flow stack (for block/loop/if/try).
  struct ControlEntry {
    enum Kind { Block, Loop, If, Try };
    Kind kind;
    /// For Block/If/Try: continuation after end (also the br target).
    /// For Loop: the loop header block (the br target).
    BasicBlock *contBlock;
    /// For Loop: the block after the loop's end (where fallthrough goes).
    /// For Block/If/Try: nullptr (contBlock serves both purposes).
    BasicBlock *endBlock = nullptr;
    /// Only for If: the else block.
    BasicBlock *elseBlock = nullptr;
    /// Block signature param types (for block params proposal).
    std::vector<WasmValType> paramTypes;
    /// Block signature result types.
    std::vector<WasmValType> resultTypes;
    /// Value stack height at entry (below any block params).
    size_t stackHeight;
    /// Phi nodes for results at the continuation block.
    /// For Block/If/Try: phis in contBlock for results from br/fallthrough.
    /// For Loop: phis in endBlock for results from fallthrough.
    std::vector<PhiInst *> resultPhis;
    /// For Loop: phi nodes in the header block for loop parameters.
    /// br/br_if targeting a loop passes values via these phis.
    std::vector<PhiInst *> paramPhis;
    /// Saved param values for If blocks with params, so they can be
    /// re-pushed at the start of the else branch.
    std::vector<Value *> savedParamValues;
    /// Whether the code was unreachable when this entry was pushed.
    bool outerUnreachable = false;
    /// Whether any branch (br/br_if) has targeted this entry's contBlock.
    bool branchTargeted = false;

    // --- Try-specific fields ---

    /// The catch dispatch block (target of TryStartInst).
    BasicBlock *catchBlock = nullptr;
    /// The CatchInst result (the caught exception value).
    /// Set when the first catch/catch_all is encountered.
    Value *caughtValue = nullptr;
    /// The block where the next catch clause's tag check begins.
    /// Updated each time a new catch/catch_all is handled.
    BasicBlock *nextCatchBlock = nullptr;
    /// Whether we have transitioned from the try body to catch handling.
    bool inCatch = false;
    /// Whether a catch_all clause was encountered.
    bool hasCatchAll = false;
  };
  std::vector<ControlEntry> controlStack_;

  // --- Helper methods ---

  /// Pop the top value from the value stack.
  Value *pop();
  /// Push a value onto the value stack.
  void push(Value *v);

  /// Wrap a value in Math.fround to produce f32 precision.
  Value *emitFround(Value *val);

  /// Check if the top of the value stack is the hi32 part of an i64.
  bool isTopI64() const;

  /// Get the ControlEntry at the given branch depth.
  ControlEntry &getControlEntry(uint32_t depth);

  /// Compute the number of phi nodes needed for the given result types.
  /// Each i64 result type contributes 2 phis (lo, hi); others contribute 1.
  static size_t numPhisForResultTypes(
      const std::vector<WasmValType> &resultTypes);

  /// Create phi nodes in \p block for the given result types.
  /// Returns the created phis (i64 types produce 2 phis each).
  std::vector<PhiInst *> createResultPhis(
      BasicBlock *block,
      const std::vector<WasmValType> &resultTypes);

  /// Add phi operands for branching to the given control entry from the
  /// current block. For Block/If entries, pops result values and adds them
  /// as phi incoming edges. For Loop entries, no phi operands are added
  /// (loop phis are for loop parameters, handled separately).
  void addBranchPhiOperands(ControlEntry &entry);

  /// Peek at (don't pop) the result values on the value stack for the given
  /// control entry and add them as phi incoming edges from the current block.
  /// Used by br_if and br_table where values must remain on the stack.
  void peekBranchPhiOperands(ControlEntry &entry);

  /// Push the result phis from a control entry onto the value stack.
  /// i64 results push as i64 pairs (lo phi, hi phi).
  void pushResultPhis(const ControlEntry &entry);

  /// Check if the current insertion block is terminated (ends with a
  /// terminator instruction).
  bool isCurrentBlockTerminated();

  /// Load a memory view variable from the top-level scope.
  /// \return the LoadFrameInst for the view.
  Value *loadMemView(MemView view);

  /// Get or lazily create the data segments Variable in topLevelVS_.
  /// Called from onMemoryInit/onDataDrop during function body compilation.
  Variable *getOrCreateDataSegVar();

  /// Get or lazily create the element segments Variable in topLevelVS_.
  /// Called from onTableInit/onElemDrop during function body compilation.
  Variable *getOrCreateElemSegVar();

  /// Emit `new Constructor(args)` and return the constructed object.
  Value *emitNew(Value *constructor, llvh::ArrayRef<Value *> args);

  /// Create the typed array views for the linear memory in the top-level
  /// function. Called from createFunctions() if the module has memory.
  /// \p tlScope is the CreateScopeInst for the top-level scope.
  void createMemoryViews(Instruction *tlScope);

  /// Create and initialize tables in the top-level function.
  /// Allocates JS Array pairs (functions + type indices) for each table,
  /// initializes them to null/-1, and applies active element segments.
  /// \p tlScope is the CreateScopeInst for the top-level scope.
  void createTables(Instruction *tlScope);

  /// Initialize Wasm globals in the top-level function.
  /// Evaluates init expressions and stores initial values.
  /// Imported globals are read from the imports object.
  /// \p tlScope is the CreateScopeInst for the top-level scope.
  void initializeGlobals(Instruction *tlScope);

  /// Load the table functions array from the top-level scope.
  Value *loadTableFuncs(uint32_t tableIndex);

  /// Load the table type-indices array from the top-level scope.
  Value *loadTableTypes(uint32_t tableIndex);

  /// Emit a byte-by-byte load from HEAPU8 for unaligned access.
  /// \p addr is the effective byte address.
  /// \p numBytes is the number of bytes to load (1, 2, 4, or 8).
  /// \return the assembled value as a single IR Value.
  Value *emitUnalignedLoad(Value *addr, uint32_t numBytes);

  /// Emit a byte-by-byte store to HEAPU8 for unaligned access.
  /// \p addr is the effective byte address.
  /// \p value is the value to store.
  /// \p numBytes is the number of bytes to store (1, 2, 4, or 8).
  void emitUnalignedStore(Value *addr, Value *value, uint32_t numBytes);

  /// Get the natural alignment (log2) for a given load/store opcode.
  /// Returns 0 for byte ops, 1 for 16-bit, 2 for 32-bit, 3 for 64-bit.
  static uint8_t getNaturalAlignLog2(llvh::StringRef opcodeName);

  /// Create an export wrapper function for the given Wasm function export.
  /// The wrapper presents a clean JS-compatible interface: 1 param per Wasm
  /// param, argument coercion, and return value marshaling.
  /// \p funcIndex is the Wasm function index of the exported function.
  /// \p exportName is the export name used for the wrapper function name.
  /// \p tlScope is the CreateScopeInst for the top-level scope, used to
  ///   load the internal function's closure.
  /// \return the created wrapper IR Function.
  Function *createExportWrapper(
      uint32_t funcIndex,
      llvh::StringRef exportName,
      Instruction *tlScope);

  /// Create an import trampoline function for the given imported function.
  /// The trampoline loads the imported JS function from the top-level scope,
  /// marshals Wasm-typed arguments to JS, calls the JS function, and
  /// converts the return value back to the expected Wasm type.
  /// \p funcIndex is the Wasm function index of the imported function.
  /// \p tlScope is the CreateScopeInst for the top-level scope.
  void createImportTrampoline(
      uint32_t funcIndex,
      Instruction *tlScope);
};

} // namespace wasm
} // namespace hermes

#endif // HERMES_WASMIRGEN_WASMIRGEN_H
