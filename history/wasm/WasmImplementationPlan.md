# WebAssembly Support: Detailed Implementation Plan

## Scope

This plan covers **Phase 1** (Correct MVP) from `history/wasm/project.md`. The goal is
to run any valid Wasm module correctly using existing Hermes infrastructure, with
no new IR instructions or bytecodes. Performance is secondary; correctness is
paramount.

Each step includes:
- **What**: Precise description of the work
- **Depends on**: Which prior steps must be complete
- **Completion criteria**: How to know it's done
- **Tests**: Specific tests to write (where applicable)

---

## Part A: Build System and Project Skeleton

### A.1: Create directory structure

**What**: Create the new directories for Wasm support:
```
include/hermes/WasmFrontend/     (public headers)
lib/WasmFrontend/                (parser + entry point)
include/hermes/WasmIRGen/        (IR generation headers)
lib/WasmIRGen/                   (IR generation implementation)
lib/VM/JSLib/WebAssembly/        (JS API implementation)
test/wasm/                       (lit tests)
unittests/WasmFrontend/          (unit tests for parser)
unittests/WasmIRGen/             (unit tests for IR generation)
```

**Depends on**: Nothing.

**Completion criteria**: Directories exist, empty `CMakeLists.txt` files are in
each `lib/` and `unittests/` directory. The top-level `CMakeLists.txt` and
`lib/CMakeLists.txt` include the new subdirectories (gated behind a
`HERMES_ENABLE_WASM` CMake option, default OFF). `unittests/CMakeLists.txt`
includes the new test subdirectories.

**Tests**: `cmake -B build -DHERMES_ENABLE_WASM=ON` succeeds. Build with empty
libraries produces no errors.

### A.2: Integrate wabt as external dependency

**What**: Add wabt to `external/wabt/` as a vendored dependency (or via CMake
`FetchContent`). Vendor the full wabt source (not just the binary reader
subset), because we need wabt's tools (`wat2wasm`, `wast2json`) for testing
(see A.7). Strip only the wabt test suite to reduce size.

Create an `external/wabt/CMakeLists.txt` (or use wabt's own CMakeLists.txt)
that builds:
- A static library `wabt_reader` containing the binary reader subset:
  `binary-reader.cc`, `binary-reader-nop.h`, `leb128.cc`, `opcode.cc`,
  `type.cc`, `error.cc`, `feature.cc`, and their dependencies. This is linked
  into the Hermes library.
- The full `libwabt` static library (needed by the test tools in A.7).

Decide between vendoring and FetchContent:
- **Vendoring** (recommended): Copy a tagged wabt release into `external/wabt/`.
  Strip the wabt test suite but keep the tool sources and WAT parser.
- **FetchContent**: Add `FetchContent_Declare(wabt ...)` to the top-level CMake.

**Depends on**: A.1

**Completion criteria**: `lib/WasmFrontend/CMakeLists.txt` can
`target_link_libraries(... wabt_reader)`. A minimal C++ file that
`#include "wabt/binary-reader.h"` compiles and links successfully.

**Tests**: Build the `hermesWasmFrontend` library target. Verify it links
against wabt_reader without undefined symbols.

### A.3: Create CMakeLists.txt for lib/WasmFrontend

**What**: Create `lib/WasmFrontend/CMakeLists.txt` with:
```cmake
add_hermes_library(hermesWasmFrontend
    WasmCompile.cpp
    BinaryReaderHermesIRGen.cpp
    LINK_OBJLIBS
    wabt_reader
    hermesIR
    hermesWasmIRGen
    hermesSupport
)
```
Include placeholder `.cpp` files.

**Depends on**: A.1, A.2

**Completion criteria**: `cmake --build build --target hermesWasmFrontend` compiles.

### A.4: Create CMakeLists.txt for lib/WasmIRGen

**What**: Create `lib/WasmIRGen/CMakeLists.txt` with:
```cmake
add_hermes_library(hermesWasmIRGen
    WasmIRGen.cpp
    WasmHelpers.cpp
    LINK_OBJLIBS
    hermesIR
    hermesSupport
)
```
Include placeholder `.cpp` files.

**Depends on**: A.1

**Completion criteria**: `cmake --build build --target hermesWasmIRGen` compiles.

### A.5: Create unit test CMakeLists.txt files

**What**: Create `unittests/WasmFrontend/CMakeLists.txt` and
`unittests/WasmIRGen/CMakeLists.txt` following the existing pattern (see
`unittests/IR/CMakeLists.txt` for reference). Set up the `add_hermes_unittest`
calls.

**Depends on**: A.3, A.4

**Completion criteria**: `cmake --build build --target WasmFrontendTest`
and `cmake --build build --target WasmIRGenTest` compile (with trivial
placeholder tests that pass).

### A.6: Wire hermesc to accept .wasm files

**What**: Add a `--wasm` flag or `.wasm` extension detection to `hermesc` so
it invokes the Wasm compilation pipeline instead of the JS pipeline. This is a
thin shim that calls `compileWasmModule()` (from `WasmCompile.h`). The function
signature is:
```cpp
/// Compile a Wasm binary module to Hermes bytecode.
/// \param buffer The raw .wasm bytes.
/// \param size Size in bytes.
/// \param outputFilename Where to write the .hbc file.
/// \returns true on success.
bool compileWasmModule(
    const uint8_t *buffer,
    size_t size,
    Module &M,
    std::string &errorMsg);
```
Initially this function can just return false with "not yet implemented".

**Depends on**: A.3

**Completion criteria**: `hermesc --wasm test.wasm` produces a clean error
message "Wasm compilation not yet implemented" (not a crash or unrecognized flag
error).

### A.7: Build wabt test tools (wat2wasm, wast2json)

**What**: Add CMake targets to build `wat2wasm` and `wast2json` from the
vendored wabt source. These are test-only tools — they are not linked into
Hermes and are not shipped. They are needed to:

- **`wat2wasm`**: Convert human-readable WAT (WebAssembly Text Format) to
  `.wasm` binaries. This is essential for writing readable lit tests. Without
  it, tests would require hand-crafted hex byte arrays.
- **`wast2json`**: Convert `.wast` spec test files into individual `.wasm`
  modules and a JSON file describing the test assertions. Needed for spec test
  suite integration (Part O).

Build these tools conditionally, only when `HERMES_ENABLE_WASM=ON`. Register
them in the lit test configuration (`test/lit.cfg`) so they are available as
`%wat2wasm` and `%wast2json` substitutions in lit tests. A typical lit test
will use a pipeline like:

```
// RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm %t.wasm -o %t.hbc && %hermes %t.hbc | %FileCheck %s
```

**Depends on**: A.2

**Completion criteria**: `cmake --build build --target wat2wasm` and
`cmake --build build --target wast2json` produce working executables.
`wat2wasm --help` runs successfully. A trivial WAT file can be converted to
`.wasm` and the output is a valid Wasm binary.

**Tests**: Convert a minimal WAT module `(module)` to `.wasm` with
`wat2wasm`. Verify non-zero output file.

---

## Part B: Wasm Data Structures (WasmModuleInfo)

### B.1: Define WasmValType and basic types

**What**: Create `include/hermes/WasmFrontend/WasmTypes.h` with:
```cpp
enum class WasmValType : uint8_t {
  I32 = 0x7F,
  I64 = 0x7E,
  F32 = 0x7D,
  F64 = 0x7C,
  V128 = 0x7B,
  FuncRef = 0x70,
  ExternRef = 0x6F,
};

struct WasmFuncType {
  std::vector<WasmValType> params;
  std::vector<WasmValType> results;
};

struct WasmLimits {
  uint32_t initial;
  uint32_t maximum;         // UINT32_MAX means no maximum
  bool hasMaximum;
};

struct WasmTableType {
  WasmValType elemType;     // funcref or externref
  WasmLimits limits;
};

struct WasmMemoryType {
  WasmLimits limits;
};

struct WasmGlobalType {
  WasmValType type;
  bool mutable_;
};
```

**Depends on**: A.1

**Completion criteria**: Header compiles. Types are sufficient to represent all
Wasm module metadata for the MVP + exception handling proposal.

**Tests**: Unit test that creates instances of each type and verifies fields.

### B.2: Define WasmImport, WasmExport, WasmFunction

**What**: Add to `WasmTypes.h` or a new `WasmModuleTypes.h`:
```cpp
enum class WasmExternalKind : uint8_t {
  Function = 0,
  Table = 1,
  Memory = 2,
  Global = 3,
};

struct WasmImport {
  std::string moduleName;
  std::string fieldName;
  WasmExternalKind kind;
  uint32_t typeIndex;       // index into types[] for functions
  WasmTableType tableType;  // for table imports
  WasmMemoryType memoryType; // for memory imports
  WasmGlobalType globalType; // for global imports
};

struct WasmExport {
  std::string name;
  WasmExternalKind kind;
  uint32_t index;           // index into the respective index space
};

struct WasmFunction {
  uint32_t typeIndex;       // index into types[]
  // Body is not stored; it is translated directly to IR during parsing.
};

struct WasmGlobal {
  WasmGlobalType type;
  // Init expression value (for simple cases: i32.const, i64.const, etc.)
  // For Phase 1, we support only constant init expressions.
  enum class InitKind { I32Const, I64Const, F32Const, F64Const, GlobalGet, RefNull, RefFunc };
  InitKind initKind;
  union {
    int32_t i32Val;
    int64_t i64Val;
    float f32Val;
    double f64Val;
    uint32_t globalIndex;
    uint32_t funcIndex;
  } initValue;
};
```

**Depends on**: B.1

**Completion criteria**: All types compile. Can represent the full structure of
any MVP Wasm module.

### B.3: Define WasmElemSegment, WasmDataSegment, WasmNameSection

**What**: Add segment types:
```cpp
struct WasmElemSegment {
  enum class Mode { Active, Passive, Declarative };
  Mode mode;
  uint32_t tableIndex;      // for active segments
  // Offset expression (same InitKind pattern as globals)
  WasmGlobal::InitKind offsetKind;
  int32_t offsetValue;      // i32.const value (common case)
  uint32_t offsetGlobalIdx; // for global.get
  std::vector<uint32_t> funcIndices; // element values
};

struct WasmDataSegment {
  enum class Mode { Active, Passive };
  Mode mode;
  uint32_t memoryIndex;     // for active segments (always 0 in MVP)
  WasmGlobal::InitKind offsetKind;
  int32_t offsetValue;
  uint32_t offsetGlobalIdx;
  std::vector<uint8_t> data;
};

struct WasmNameSection {
  std::string moduleName;
  std::vector<std::string> functionNames; // indexed by func index
  // Local names omitted for now.
};
```

**Depends on**: B.2

**Completion criteria**: Types compile.

### B.4: Define WasmModuleInfo

**What**: Create `include/hermes/WasmFrontend/WasmModuleInfo.h`:
```cpp
struct WasmModuleInfo {
  std::vector<WasmFuncType> types;
  std::vector<WasmImport> imports;
  std::vector<WasmFunction> functions;
  std::vector<WasmTableType> tables;
  std::vector<WasmMemoryType> memories;
  std::vector<WasmGlobal> globals;
  std::vector<WasmExport> exports;
  std::optional<uint32_t> startFunction;
  std::vector<WasmElemSegment> elements;
  std::vector<WasmDataSegment> dataSegments;
  WasmNameSection names;

  // Helper methods:

  /// Total number of functions (imported + defined).
  uint32_t totalFunctionCount() const;
  /// Number of imported functions.
  uint32_t importedFunctionCount() const;
  /// Get the type of function at the given index (handles imports).
  const WasmFuncType &getFunctionType(uint32_t funcIndex) const;
  /// Total number of globals (imported + defined).
  uint32_t totalGlobalCount() const;
  uint32_t importedGlobalCount() const;
  /// Total number of tables (imported + defined).
  uint32_t totalTableCount() const;
  uint32_t importedTableCount() const;
  /// Total number of memories (imported + defined).
  uint32_t totalMemoryCount() const;
  uint32_t importedMemoryCount() const;
};
```

**Depends on**: B.1, B.2, B.3

**Completion criteria**: Header compiles. Helper methods are implemented in a
corresponding `.cpp` file. Unit tests verify the helper methods.

**Tests**: Unit test: create a WasmModuleInfo with 2 imported functions and 3
defined functions, verify `totalFunctionCount()` returns 5,
`importedFunctionCount()` returns 2, `getFunctionType()` returns the correct
type for each index.

---

## Part C: Wasm Binary Parser (wabt Integration)

### C.1: Implement BinaryReaderHermesIRGen — module-level callbacks

**What**: Create `lib/WasmFrontend/BinaryReaderHermesIRGen.h` and `.cpp`.
Implement the `wabt::BinaryReaderNop` subclass with callbacks for module
structure (sections 1-12). These callbacks populate a `WasmModuleInfo`.

Callbacks to implement:
- `OnTypeCount`, `OnFuncType` → populate `moduleInfo_.types`
- `OnImportCount`, `OnImport`, `OnImportFunc`, `OnImportTable`,
  `OnImportMemory`, `OnImportGlobal` → populate `moduleInfo_.imports`
- `OnFunctionCount`, `OnFunction` → populate `moduleInfo_.functions`
- `OnTableCount`, `OnTable` → populate `moduleInfo_.tables`
- `OnMemoryCount`, `OnMemory` → populate `moduleInfo_.memories`
- `OnGlobalCount`, `BeginGlobal`, `OnGlobalInitExprI32ConstExpr`, etc. →
  populate `moduleInfo_.globals`
- `OnExportCount`, `OnExport` → populate `moduleInfo_.exports`
- `OnStartFunction` → set `moduleInfo_.startFunction`
- `OnElemSegmentCount`, `BeginElemSegment`, `OnElemSegmentFunctionIndexCount`,
  `OnElemSegmentFunctionIndex` → populate `moduleInfo_.elements`
- `OnDataSegmentCount`, `BeginDataSegment`, `OnDataSegmentData` →
  populate `moduleInfo_.dataSegments`

Leave function body callbacks (`BeginFunctionBody`, `OnOpcode*`) as no-ops for
now.

**Depends on**: A.2, B.4

**Completion criteria**: Given a valid `.wasm` binary, `ReadBinary()` with our
delegate populates `WasmModuleInfo` with correct types, imports, exports,
functions, memories, tables, globals, elements, and data segments.

**Tests**: Unit tests:
1. Parse a minimal `.wasm` file (hand-crafted byte array) containing one
   function type `(i32, i32) -> i32`, one function with that type, one memory
   (1 page), and one export. Verify all `WasmModuleInfo` fields.
2. Parse a `.wasm` file with imports (2 function imports, 1 memory import).
   Verify import fields.
3. Parse a `.wasm` file with data segments and element segments. Verify segment
   data.
4. Parse an invalid `.wasm` file (bad magic, truncated). Verify error is
   reported, no crash.

### C.2: Implement compileWasmModule entry point (skeleton)

**What**: Create `lib/WasmFrontend/WasmCompile.cpp` with the
`compileWasmModule()` function. This function:
1. Calls `wabt::ReadBinary()` with `BinaryReaderHermesIRGen`.
2. Returns the populated `WasmModuleInfo`.
3. (Future: drives WasmIRGen and BCGen.)

For now, it parses the module and prints the exports (for debugging), then
returns success.

**Depends on**: C.1

**Completion criteria**: `hermesc --wasm test.wasm` parses a valid Wasm file
without errors and prints the number of types, functions, imports, and exports.

**Tests**: Compile a `.wasm` file produced by `wat2wasm` (from wabt tools or
any Wasm toolchain). Verify the output summary matches expectations.

### C.2.1: Comprehensive module info dump lit tests

**What**: Add lit tests that compile `.wat` files to `.wasm` via `%wat2wasm`,
then run `hermesc --wasm` and verify the parsed module summary output using
`%FileCheck`. These tests exercise the wabt → `BinaryReaderHermesIRGen` →
`WasmModuleInfo` pipeline end-to-end with human-readable WAT input.

C.2 added basic lit tests for a simple module and imports. This step adds
additional coverage for module structures not yet tested:

1. **Globals**: Module with mutable and immutable globals, init expressions
   (i32.const, f64.const, global.get).
2. **Tables**: Module with a table declaration and element segments.
3. **Data segments**: Module with active data segments and memory.
4. **Multiple imports**: Module importing functions, tables, memories, and
   globals from different modules.
5. **Start function**: Module with a start function declaration.
6. **Name section**: Verify function names appear when present.

Each test is a standalone `.wat` file in `test/wasm/` with `REQUIRES: wasm`
and FileCheck assertions on the module summary output.

**Depends on**: C.2, A.7

**Completion criteria**: At least 4 new lit tests covering globals, tables,
data segments, and mixed imports. All pass with the existing module summary
output format.

**Tests**: The step *is* the tests. Each `.wat` file is a self-contained lit
test.

---

## Part D: WasmIRGen Core — Stack Machine to SSA

This is the most complex part. We build it incrementally, starting with the
simplest possible programs and adding features one by one.

### D.1: WasmIRGen class skeleton

**What**: Create `include/hermes/WasmIRGen/WasmIRGen.h` and
`lib/WasmIRGen/WasmIRGen.cpp`. Define the `WasmIRGen` class:

```cpp
class WasmIRGen {
 public:
  WasmIRGen(Module &M, WasmModuleInfo &moduleInfo);

  /// Create Hermes IR Functions for all Wasm functions.
  /// Called once after module-level parsing is complete.
  void createFunctions();

  // --- Per-function translation (called by BinaryReaderHermesIRGen) ---

  /// Begin translating a function body.
  void beginFunction(uint32_t funcIndex, const std::vector<WasmValType> &locals);

  /// End translating a function body.
  void endFunction();

  // --- Instruction callbacks (one per Wasm instruction category) ---
  void onI32Const(int32_t value);
  void onI64Const(int64_t value);
  void onF32Const(float value);
  void onF64Const(double value);
  void onLocalGet(uint32_t localIndex);
  void onLocalSet(uint32_t localIndex);
  void onLocalTee(uint32_t localIndex);
  // ... (added incrementally in subsequent steps)

 private:
  Module &M_;
  WasmModuleInfo &moduleInfo_;
  IRBuilder builder_;

  // Per-function state:

  /// The current Hermes IR function being built.
  Function *currentFunc_ = nullptr;

  /// Abstract value stack: stack of Value* (Hermes IR SSA values).
  std::vector<Value *> valueStack_;

  /// AllocStackInst for each Wasm local (params + locals).
  std::vector<AllocStackInst *> locals_;

  /// Control flow stack (for block/loop/if).
  struct ControlEntry {
    enum Kind { Block, Loop, If };
    Kind kind;
    BasicBlock *contBlock;    // continuation after end (or loop header for Loop)
    BasicBlock *elseBlock;    // only for If
    std::vector<WasmValType> resultTypes; // block signature results
    size_t stackHeight;       // value stack height at entry
    // For tracking phi nodes at the continuation block:
    std::vector<PhiInst *> resultPhis;
  };
  std::vector<ControlEntry> controlStack_;

  // Helper methods:
  Value *pop();
  void push(Value *v);
  void pushI32(Value *v);     // push with i32 wrapping semantics
  Value *ensureI32(Value *v); // apply |0 if needed
};
```

**Depends on**: A.4, B.4

**Completion criteria**: WasmIRGen compiles. `createFunctions()` creates one
empty Hermes IR `Function` per Wasm function with the correct number of
parameters. Each function has a single empty `BasicBlock` with
`ReturnInst(undefined)`.

**Tests**: Unit test: Create a WasmModuleInfo with 3 functions of different
signatures. Call `createFunctions()`. Verify the IR Module has 3 functions with
correct parameter counts.

### D.1.1: Wire wasm-to-IR pipeline and `--dump-ir` support

**What**: Wire `compileWasmModule()` to actually generate Hermes IR (not just
parse and print a summary), and make `hermesc --wasm --dump-ir` work. This
enables FileCheck-based lit tests that verify the IR structure produced by
WasmIRGen — critical for catching issues in the stack-to-SSA translation
that runtime-only tests might miss.

Changes:
1. Update `compileWasmModule()` to:
   a. Parse the wasm binary (already done in C.2).
   b. Call `WasmIRGen::createFunctions()` to create IR functions.
   c. Call `beginFunction()` / `endFunction()` for each function body.
   d. Return the populated `Module` to the caller.
2. Wire `BinaryReaderHermesIRGen` instruction callbacks to `WasmIRGen`
   methods. Initially only the callbacks implemented in D.1 (constants,
   local.get/set/tee) are functional; unimplemented opcodes produce an
   error message. **Each subsequent D.x step expands the wiring** as new
   instruction callbacks are implemented in WasmIRGen.
3. Update `processWasmFile()` in CompilerDriver to support `--dump-ir`:
   after generating IR, dump the module to stdout (same format as JS
   `--dump-ir`).
4. Add a basic lit test that compiles a trivial `.wat` function and checks
   the IR structure with FileCheck.

Typical `--dump-ir` lit test pattern (used by all subsequent D.x steps):
```
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir %t.wasm | %FileCheck %s

(module
  (func (param i32) (result i32)
    local.get 0))

;; CHECK-LABEL: function wasm_func_0(
;; CHECK:   %{{[0-9]+}} = LoadStackInst
;; CHECK:   %{{[0-9]+}} = ReturnInst
```

**Depends on**: D.1, C.1

**Completion criteria**: `hermesc --wasm --dump-ir test.wasm` produces readable
Hermes IR for a simple function with constants and locals. At least one lit test
verifies the IR structure via FileCheck.

**Tests**:
1. Lit test: function that returns its parameter → verify `LoadParamInst`,
   `StoreStackInst`, `LoadStackInst`, `ReturnInst` in the IR.
2. Lit test: function with `i32.const 42` → verify `LiteralNumber` appears.
3. Lit test: function with `local.tee` → verify the value is both stored and
   returned.

### D.2: Constants and locals

**What**: Implement:
- `onI32Const(val)` → push `LiteralNumber((double)val)`
- `onF32Const(val)` → push `LiteralNumber((double)val)`
- `onF64Const(val)` → push `LiteralNumber(val)`
- `onI64Const(val)` → push two values (lo32, hi32) as `LiteralNumber` (for
  split i32 representation; see Part G for i64 details)
- `beginFunction()` → create `AllocStackInst` for each param and local;
  initialize params from function arguments using `StoreStackInst`; initialize
  locals to zero/null per their type.
- `onLocalGet(idx)` → push `LoadStackInst(locals_[idx])`
- `onLocalSet(idx)` → pop, `StoreStackInst(val, locals_[idx])`
- `onLocalTee(idx)` → pop, store, push the same value back

Also wire the corresponding `BinaryReaderHermesIRGen` callbacks (`OnI32ConstExpr`,
`OnLocalGetExpr`, `OnLocalSetExpr`, `OnLocalTeeExpr`, etc.) to the new
WasmIRGen methods.

**Depends on**: D.1, D.1.1

**Completion criteria**: A function with only constants and local get/set
produces correct IR. The Mem2Reg pass can promote these to SSA.

**Tests** (unit tests + `--dump-ir` lit tests):
1. Wasm function `(func (param i32) (result i32) local.get 0)` → IR returns
   the first parameter.
2. Wasm function with 2 locals, local.set and local.get in sequence → IR has
   correct data flow.
3. Wasm function with `local.tee` → value is both stored and available.
4. `--dump-ir` lit test: verify `AllocStackInst`, `StoreStackInst`,
   `LoadStackInst` appear in the correct order for a function with locals.

### D.3: Simple i32 arithmetic

**What**: Implement i32 binary operations using the asm.js pattern:
- `i32.add` → `pop(); pop(); push(BitOr(AddN(a, b), 0))` (the `|0` truncates
  to int32)
- `i32.sub` → `pop(); pop(); push(BitOr(SubN(a, b), 0))`
- `i32.mul` → `pop(); pop(); push(CallBuiltin(Math.imul, a, b))`
- `i32.and` → `pop(); pop(); push(BitAnd(a, b))`
- `i32.or` → `pop(); pop(); push(BitOr(a, b))`
- `i32.xor` → `pop(); pop(); push(BitXor(a, b))`
- `i32.shl` → `pop(); pop(); push(LShift(a, b))`
- `i32.shr_s` → `pop(); pop(); push(RShift(a, b))`
- `i32.shr_u` → `pop(); pop(); push(URShift(a, b))`

Use existing `BinaryOperatorInst` (kind: `BitAndInstKind`, `BitOrInstKind`,
`LShiftInstKind`, etc.) and `FAddInst`/`FSubtractInst`/`FMultiplyInst` with
wrapping.

Also wire the corresponding `BinaryReaderHermesIRGen` callbacks
(`OnBinaryExpr` dispatch for each opcode).

**Depends on**: D.2

**Completion criteria**: Functions using i32 arithmetic produce correct IR that
evaluates to correct results.

**Tests** (lit tests, run via `hermes`; plus `--dump-ir` lit tests):
1. `i32.add(40, 2)` returns 42.
2. `i32.add(2147483647, 1)` returns -2147483648 (wrapping).
3. `i32.mul(65536, 65536)` returns 0 (wrapping, verifies Math.imul is used).
4. `i32.shr_u(-1, 0)` returns 4294967295 as unsigned (verifies URShift).
5. Combination of operations: `(a + b) * c - d`.
6. `--dump-ir` lit test: verify `i32.add` produces `FAddInst` + `AsInt32Inst`
   (or `BitOr` with 0) in the IR.

### D.4: i32 comparison operations

**What**: Implement i32 comparisons:
- `i32.eq` → `StrictlyEqual(a, b)` — produces boolean, convert to i32 (1 or 0)
- `i32.ne` → `StrictlyNotEqual(a, b)` → i32
- `i32.lt_s` → `Less(AsInt32(a), AsInt32(b))` → i32
- `i32.gt_s` → `Greater(AsInt32(a), AsInt32(b))` → i32
- `i32.le_s` → `LessEq(AsInt32(a), AsInt32(b))` → i32
- `i32.ge_s` → `GreaterEq(AsInt32(a), AsInt32(b))` → i32
- `i32.lt_u` → `Less(AsUint32(a), AsUint32(b))` → i32
- `i32.gt_u` → `Greater(AsUint32(a), AsUint32(b))` → i32
- `i32.le_u` → `LessEq(AsUint32(a), AsUint32(b))` → i32
- `i32.ge_u` → `GreaterEq(AsUint32(a), AsUint32(b))` → i32
- `i32.eqz` → `StrictlyEqual(a, 0)` → i32

Note: Boolean-to-i32 conversion: `CondBranch(bool, trueBlock, falseBlock)` with
`PhiInst` merging `LiteralNumber(1)` and `LiteralNumber(0)`. Or use
`BitOr(bool, 0)` which converts `true`→1, `false`→0 in Hermes.

**Depends on**: D.3

**Completion criteria**: All i32 comparison operations produce correct i32
results (0 or 1).

**Tests** (lit):
1. `i32.eq(5, 5)` → 1; `i32.eq(5, 6)` → 0.
2. `i32.lt_s(-1, 0)` → 1 (signed: -1 < 0).
3. `i32.lt_u(-1, 0)` → 0 (unsigned: 0xFFFFFFFF > 0).
4. `i32.eqz(0)` → 1; `i32.eqz(42)` → 0.

### D.5: Return instruction

**What**: Implement:
- `return` → pop result value (if function has result type), `ReturnInst(val)`.
  If no result, `ReturnInst(undefined)`.
- `endFunction()` → if the function body falls through (no explicit return),
  pop the result value from the stack and emit `ReturnInst`.
- `drop` → pop and discard.

**Depends on**: D.2

**Completion criteria**: Functions with explicit `return` and implicit fallthrough
both produce correct IR.

**Tests**:
1. Function `(result i32) i32.const 42 return` → returns 42.
2. Function `(result i32) i32.const 42` (implicit return) → returns 42.
3. Function `() i32.const 42 drop` → returns undefined (void function).

### D.6: Block and br/br_if

**What**: Implement structured control flow for `block`/`end` and `br`/`br_if`:

- `block (result T)`:
  - Push a `ControlEntry{Block}` on `controlStack_`.
  - Create a continuation `BasicBlock` (target of `br 0` = after `end`).
  - Record the value stack height.
  - If the block has a result type, create a `PhiInst` in the continuation
    block for the result.

- `end` (for block):
  - Pop `ControlEntry`.
  - If block has result, the value on top of the stack becomes an incoming
    edge to the continuation block's Phi.
  - Branch to the continuation block.
  - Set insertion point to the continuation block.
  - Push the Phi result onto the value stack (if any).

- `br depth`:
  - Look up `controlStack_[controlStack_.size() - 1 - depth]`.
  - For `Block`: branch to its `contBlock`, add current values as Phi operands.
  - For `Loop`: branch to its `contBlock` (which is the loop header).
  - After `br`, the current block is terminated. Start a new unreachable block
    (for dead code after unconditional branch).

- `br_if depth`:
  - Pop condition.
  - `CondBranchInst(cond, targetBlock, fallthroughBlock)`.
  - If block has results, manage Phi edges from the branch-taken path.

**Depends on**: D.4, D.5

**Completion criteria**: Programs with `block`/`br`/`br_if` produce correct IR
and execute correctly.

**Tests** (lit):
1. `(block (result i32) i32.const 42 br 0 end)` → 42.
2. `(block i32.const 1 br_if 0 unreachable end)` → no trap (br_if taken).
3. `(block i32.const 0 br_if 0 i32.const 99 end)` → 99 (br_if not taken,
   falls through to return 99; note: this block has no result type, 99 would be
   dropped or used after the block).
4. Nested blocks with `br` targeting outer block.

### D.7: Loop

**What**: Implement `loop`/`end`:

- `loop (params...) (results...)`:
  - Create a loop header `BasicBlock`.
  - Branch from current block to loop header.
  - Push `ControlEntry{Loop}` where `contBlock` = loop header (because `br` to
    a loop targets the top, not the end).
  - Create Phi nodes for loop parameters in the header.

- `end` (for loop):
  - Pop `ControlEntry`.
  - The loop body falls through to the code after `end` (not back to the
    header — that's what `br` does).
  - Create a continuation block after the loop.

- `br depth` (targeting loop): branches back to the loop header.

**Depends on**: D.6

**Completion criteria**: Programs with loops execute correctly, including loops
that branch back (`br`) and loops that fall through.

**Tests** (lit):
1. Simple countdown loop: initialize counter to 10, loop: decrement, br_if
   counter != 0. Return counter (should be 0).
2. Sum 1..100 in a loop: should return 5050.
3. Nested loop: outer loop 3 iterations, inner loop 4 iterations, count total
   iterations → 12.

### D.8: If/else

**What**: Implement `if`/`else`/`end`:

- `if (result T)`:
  - Pop condition.
  - Create `thenBlock`, `elseBlock`, `mergeBlock`.
  - `CondBranchInst(cond, thenBlock, elseBlock)`.
  - Set insertion point to `thenBlock`.
  - Push `ControlEntry{If}` with `contBlock = mergeBlock`,
    `elseBlock = elseBlock`.

- `else`:
  - The current `thenBlock` ends; branch to `mergeBlock`.
  - Add Phi edges from then-block for any result values.
  - Set insertion point to `elseBlock`.

- `end` (for if):
  - The current block (else, or then if no else) branches to `mergeBlock`.
  - Add Phi edges for result values.
  - Set insertion point to `mergeBlock`.
  - Push Phi result.

Note: `if` without `else` and with a result type is invalid in Wasm (wabt
validation catches this). `if` without `else` and without a result type is
valid — the `elseBlock` just branches directly to `mergeBlock`.

**Depends on**: D.6

**Completion criteria**: If/else constructs produce correct IR and execute
correctly.

**Tests** (lit):
1. `if (i32.const 1) i32.const 42 else i32.const 99 end` → 42.
2. `if (i32.const 0) i32.const 42 else i32.const 99 end` → 99.
3. `if` without `else` and without result: side-effecting body runs
   conditionally.
4. Nested if/else chains.

### D.9: br_table (switch)

**What**: Implement `br_table labels default`:
- Pop index value from stack.
- Create a `SwitchInst` with one case per label.
- Each case branches to the appropriate control entry's target block.
- The default case branches to the default label's target.

**Depends on**: D.6

**Completion criteria**: `br_table` programs execute correctly.

**Tests** (lit):
1. Switch with 4 cases: each returns a different value. Test index 0, 1, 2, 3,
   and out-of-range (falls to default).

### D.10: select

**What**: Implement `select`:
- Pop condition, val2, val1.
- `CondBranchInst(cond, trueBlock, falseBlock)` + `PhiInst` merging val1 (if
  cond != 0) and val2 (if cond == 0).
- Push Phi result.

Note: `select` is like `cond ? val1 : val2` (note order: val1 is "on true").

**Depends on**: D.4

**Completion criteria**: `select` produces correct results.

**Tests** (lit):
1. `(select (i32.const 42) (i32.const 99) (i32.const 1))` → 42.
2. `(select (i32.const 42) (i32.const 99) (i32.const 0))` → 99.

### D.11: unreachable and nop

**What**:
- `unreachable` → Call trap helper (Part F), then `UnreachableInst`.
- `nop` → No-op (don't emit anything).

**Depends on**: D.1

**Completion criteria**: `unreachable` causes a WebAssembly.RuntimeError trap.

**Tests** (lit):
1. Call a function containing `unreachable` → throws RuntimeError.

### D.12: Function calls (call)

**What**: Implement `call funcIndex`:
- Look up the function's type signature.
- Pop arguments from value stack (in reverse order, since Wasm stack is
  operand-order).
- Emit `CallInst` to the corresponding Hermes IR function.
- Push return value (if function has a result type).

For calls to imported functions, the call goes through a trampoline (Part I).
For now, implement calls to functions defined in the same module.

**Depends on**: D.5

**Completion criteria**: Direct calls between Wasm functions work.

**Tests** (lit):
1. Function A calls function B which returns a constant. A returns B's result.
2. Recursive factorial function.
3. Mutual recursion between two functions.

### D.13: Verify all BinaryReaderHermesIRGen ↔ WasmIRGen callbacks are wired

**What**: D.1.1 established the basic wiring between `BinaryReaderHermesIRGen`
and `WasmIRGen`, and each subsequent D.x step expanded it incrementally as new
instruction callbacks were implemented. This step verifies that **all** Wasm
MVP instruction callbacks are wired and that no opcodes silently fall through
to the `BinaryReaderNop` no-op base. Specifically:

1. Audit all `BinaryReaderDelegate` instruction callbacks against the Wasm MVP
   instruction set (Appendix A) and verify each is either: (a) wired to a
   WasmIRGen method, or (b) explicitly produces an "unsupported opcode" error.
2. Replace any remaining assert/error stubs for opcodes that should now be
   implemented (e.g., any that were deferred during D.2–D.12).
3. Add a comprehensive `--dump-ir` lit test that exercises at least one
   instruction from each category (control flow, parametric, variable, memory,
   i32 numeric, i64 numeric, f32 numeric, f64 numeric, conversions).

**Depends on**: C.1, D.1.1, D.2 through D.12

**Completion criteria**: A complete `.wasm` file with instructions from every
category can be parsed and translated to Hermes IR. No instruction silently
falls through to a no-op. A single comprehensive `--dump-ir` lit test passes.

**Tests**:
1. `--dump-ir` lit test: `.wat` module using one instruction from each
   category → verify IR contains the expected instruction patterns.
2. End-to-end test: compile a `.wasm` file containing basic arithmetic and
   control flow to `.hbc`, run it, verify output.

### D.14: Integration — compile .wasm to .hbc and run

**What**: Complete the `compileWasmModule()` pipeline:
1. Parse via wabt → `WasmModuleInfo` + Hermes IR functions.
2. Run the standard optimizer pipeline (Mem2Reg, SimplifyCFG, DCE, etc.).
3. Run BCGen to produce bytecode.
4. Serialize to `.hbc`.

This requires creating a suitable Hermes `Module` and `Context`, setting up
the compilation pipeline, and writing the output.

**Depends on**: D.13

**Completion criteria**: `hermesc --wasm test.wasm -o test.hbc && hermes test.hbc`
works for a simple program that computes and prints a result.

**Tests** (lit):
1. End-to-end: `.wasm` file with an exported function that adds two numbers.
   JS glue code calls the export. Output is correct.
2. End-to-end: `.wasm` file with loops and branches.

---

## Part E: f32 and f64 Operations

### E.1: f64 arithmetic

**What**: Implement f64 operations — these map directly to existing IR:
- `f64.add` → `FAddInst(a, b)`
- `f64.sub` → `FSubtractInst(a, b)`
- `f64.mul` → `FMultiplyInst(a, b)`
- `f64.div` → `FDivideInst(a, b)`
- `f64.neg` → `FNegateInst(a)`
- `f64.eq/ne/lt/gt/le/ge` → `FEqualInst`, `FNotEqualInst`, etc.
- `f64.abs` → `CallBuiltin(Math.abs, a)`
- `f64.sqrt` → `CallBuiltin(Math.sqrt, a)`
- `f64.ceil/floor/trunc/nearest` → `CallBuiltin(Math.ceil/floor/trunc/round, a)`
- `f64.min/max` → `CallBuiltin(Math.min/max, a, b)`
- `f64.copysign` → helper function (see Part F)

**Depends on**: D.3

**Completion criteria**: All f64 operations produce correct results.

**Tests** (lit):
1. `f64.add(1.5, 2.5)` → 4.0
2. `f64.div(1.0, 3.0)` → 0.333... (verify precision)
3. `f64.sqrt(4.0)` → 2.0
4. `f64.min(NaN, 1.0)` → NaN (Wasm min/max propagate NaN)
5. `f64.neg(-0.0)` → 0.0

### E.2: f32 arithmetic

**What**: Same as f64 but with `Math.fround()` applied after each operation:
- `f32.add` → `CallBuiltin(Math.fround, FAddInst(a, b))`
- `f32.sub` → `CallBuiltin(Math.fround, FSubtractInst(a, b))`
- etc.

Also:
- `f32.abs` → `CallBuiltin(Math.fround, CallBuiltin(Math.abs, a))`
- `f32.sqrt` → `CallBuiltin(Math.fround, CallBuiltin(Math.sqrt, a))`
- etc.

**Depends on**: E.1

**Completion criteria**: f32 operations produce correctly-rounded f32 results.

**Tests** (lit):
1. `f32.add(1.0f, 1e-10f)` → verify the result is correctly rounded to f32
   precision (not f64 precision).
2. `f32.mul(large, large)` → verify overflow to Infinity happens at f32 range.

### E.3: f64/f32 comparison returning i32

**What**: Wasm float comparisons return i32 (0 or 1), not boolean. Implement:
- `f64.eq` → compare, then convert boolean to i32 (same pattern as D.4).
- `f64.ne/lt/gt/le/ge` → same pattern.
- `f32.eq/ne/lt/gt/le/ge` → same.

Note: Wasm comparisons with NaN: `f64.eq(NaN, NaN)` → 0 (not equal),
`f64.lt(NaN, 1.0)` → 0, etc. These match IEEE 754 semantics, which Hermes's
comparison instructions already follow.

**Depends on**: E.1, D.4

**Completion criteria**: Float comparisons return correct i32 results, including
NaN cases.

**Tests**: `f64.eq(NaN, NaN)` → 0. `f64.lt(1.0, 2.0)` → 1.

---

## Part F: Helper Functions for Missing Operations

### F.1: Create WasmHelpers infrastructure

**What**: Create `lib/WasmIRGen/WasmHelpers.cpp` and `.h`. This file defines
native C++ helper functions that are called from Wasm-generated IR for
operations that have no direct JS/asm.js equivalent.

Each helper is a `CallResult<HermesValue>` function with the standard Hermes
native function signature. They are registered as builtin methods.

Alternatively, they can be emitted as `CallInst` to NativeFunction objects
that are set up during Wasm instantiation.

Define a `WasmHelpers` class that creates these helper functions and provides
handles to them for WasmIRGen to call.

**Depends on**: D.1

**Completion criteria**: Infrastructure for calling helper functions from
generated Wasm IR is in place. At least one trivial helper can be called.

### F.2: i32 trapping division helpers

**What**: Implement:
- `wasm_i32_div_s(a, b)`: Trap if `b == 0` or `(a == INT32_MIN && b == -1)`.
  Otherwise return `a / b` (signed, truncated toward zero).
- `wasm_i32_div_u(a, b)`: Trap if `b == 0`. Return `(uint32_t)a / (uint32_t)b`.
- `wasm_i32_rem_s(a, b)`: Trap if `b == 0`. Return `a % b`. Note:
  `INT32_MIN % -1` → 0 (not a trap for rem).
- `wasm_i32_rem_u(a, b)`: Trap if `b == 0`. Return `(uint32_t)a % (uint32_t)b`.

Each helper checks the trap condition and calls
`runtime.raiseError(WebAssembly.RuntimeError, "integer divide by zero")` or
`"integer overflow"` on trap.

**Depends on**: F.1

**Completion criteria**: Trapping division/remainder work correctly for all edge
cases.

**Tests** (lit):
1. `i32.div_s(10, 3)` → 3.
2. `i32.div_s(10, 0)` → trap "integer divide by zero".
3. `i32.div_s(-2147483648, -1)` → trap "integer overflow".
4. `i32.rem_s(-2147483648, -1)` → 0 (not a trap!).
5. `i32.div_u(0xFFFFFFFF, 2)` → 2147483647.

### F.3: i32 bit manipulation helpers

**What**: Implement:
- `wasm_i32_clz(a)` → `CallBuiltin(Math.clz32, a)` (already exists in Hermes)
- `wasm_i32_ctz(a)` → native helper (count trailing zeros)
- `wasm_i32_popcnt(a)` → native helper (population count)
- `wasm_i32_rotl(a, b)` → native helper: `(a << (b & 31)) | (a >>> (32 - (b & 31)))`
- `wasm_i32_rotr(a, b)` → native helper: `(a >>> (b & 31)) | (a << (32 - (b & 31)))`
- `wasm_i32_extend8_s(a)` → `(a << 24) >> 24` (sign-extend from 8 bits)
- `wasm_i32_extend16_s(a)` → `(a << 16) >> 16` (sign-extend from 16 bits)

Note: `i32.clz` maps to `Math.clz32` which already exists. For `ctz` and
`popcnt`, there are no JS builtins, so native helpers are needed.

Implementation for `ctz`: Use compiler builtins (`__builtin_ctz`) or a
bit-twiddling algorithm.

**Depends on**: F.1

**Completion criteria**: All bit manipulation operations produce correct results.

**Tests** (lit):
1. `i32.clz(1)` → 31. `i32.clz(0)` → 32.
2. `i32.ctz(0x80000000)` → 31. `i32.ctz(0)` → 32.
3. `i32.popcnt(0x0F0F0F0F)` → 16.
4. `i32.rotl(0x80000001, 1)` → 3.
5. `i32.extend8_s(0xFF)` → -1. `i32.extend8_s(0x7F)` → 127.

### F.4: Conversion helpers

**What**: Implement type conversion operations:

**Truncation (float → int) with trapping**:
- `i32.trunc_f32_s(a)` → trap on NaN or out-of-range. Return `(int32_t)(float)a`.
- `i32.trunc_f32_u(a)` → trap on NaN or out-of-range. Return `(uint32_t)(float)a`.
- `i32.trunc_f64_s(a)` → similar.
- `i32.trunc_f64_u(a)` → similar.

**Saturating truncation (no trap)**:
- `i32.trunc_sat_f32_s(a)` → clamp to [INT32_MIN, INT32_MAX], NaN → 0.
- `i32.trunc_sat_f32_u(a)` → clamp to [0, UINT32_MAX], NaN → 0.
- `i32.trunc_sat_f64_s(a)` → similar.
- `i32.trunc_sat_f64_u(a)` → similar.

**Float conversions**:
- `f32.convert_i32_s(a)` → `Math.fround((double)(int32_t)a)`
- `f32.convert_i32_u(a)` → `Math.fround((double)(uint32_t)a)`
- `f64.convert_i32_s(a)` → `(double)(int32_t)a` (exact for all i32)
- `f64.convert_i32_u(a)` → `(double)(uint32_t)a` (exact for all i32)
- `f32.demote_f64(a)` → `Math.fround(a)`
- `f64.promote_f32(a)` → identity (already stored as f64)

**Reinterpret (bit-cast)**:
- `i32.reinterpret_f32(a)` → native helper using `memcpy` or union
- `f32.reinterpret_i32(a)` → native helper

**Depends on**: F.1

**Completion criteria**: All conversion operations produce correct results
including edge cases (NaN, infinity, overflow).

**Tests** (lit):
1. `i32.trunc_f64_s(2.9)` → 2.
2. `i32.trunc_f64_s(-2.9)` → -2 (truncates toward zero).
3. `i32.trunc_f64_s(NaN)` → trap.
4. `i32.trunc_f64_s(3e10)` → trap (out of range).
5. `i32.trunc_sat_f64_s(3e10)` → INT32_MAX (2147483647).
6. `i32.trunc_sat_f64_s(NaN)` → 0.
7. `f32.demote_f64(1.0000000000000002)` → 1.0f (rounded).
8. `i32.reinterpret_f32(0.0)` → 0.
9. `i32.reinterpret_f32(-0.0)` → 0x80000000.

### F.5: f64/f32 copysign, min, max helpers

**What**:
- `f64.copysign(a, b)` → copy the sign bit of b to a. Native helper using
  bit manipulation.
- `f64.min(a, b)` and `f64.max(a, b)` → Wasm min/max semantics differ from
  `Math.min`/`Math.max`:
  - If either operand is NaN, result is NaN (same as JS).
  - `min(-0.0, +0.0)` → `-0.0` (JS `Math.min` returns `-0.0` too, so OK).
  - `max(-0.0, +0.0)` → `+0.0` (JS `Math.max` returns `+0.0` too, so OK).
  - Actually, Wasm specifies that if either input is NaN, result is NaN. And
    for +-0, min(-0,+0) = -0, max(-0,+0) = +0. `Math.min`/`Math.max` match!

  So `Math.min`/`Math.max` can be used directly for f64. For f32, wrap in
  `Math.fround`.

- `f32.copysign` → same as f64.copysign but with f32 rounding.

**Depends on**: F.1

**Completion criteria**: copysign, min, max produce correct results.

**Tests**: `f64.copysign(1.0, -1.0)` → -1.0. `f64.min(NaN, 1.0)` → NaN.

---

## Part G: i64 Support (Split i32 Pairs)

### G.1: Define i64 representation

**What**: Define the convention for representing i64 values on the Hermes value
stack. Each i64 occupies **two** stack slots: `[lo32, hi32]` where `lo32` is
the lower 32 bits (as a Number) and `hi32` is the upper 32 bits (as a Number).

Add helper methods to WasmIRGen:
- `pushI64(Value *lo, Value *hi)` — push two values.
- `popI64() → (Value *lo, Value *hi)` — pop two values.
- Track in the type system (WasmValType) which stack positions are i64 halves.

**Depends on**: D.2

**Completion criteria**: i64 values can be pushed and popped from the abstract
value stack.

### G.2: i64 constants

**What**: `i64.const value`:
- Split into lo32 and hi32.
- `pushI64(LiteralNumber(lo32), LiteralNumber(hi32))`

**Depends on**: G.1

### G.3: i64 arithmetic helpers

**What**: Implement native helper functions for i64 arithmetic using split i32:
- `wasm_i64_add(lo_a, hi_a, lo_b, hi_b)` → returns `{lo_result, hi_result}`
  (two HermesValues, e.g., via a shared return buffer or by returning an array).
- `wasm_i64_sub`, `wasm_i64_mul`, `wasm_i64_div_s`, `wasm_i64_div_u`,
  `wasm_i64_rem_s`, `wasm_i64_rem_u`
- `wasm_i64_and`, `wasm_i64_or`, `wasm_i64_xor`
- `wasm_i64_shl`, `wasm_i64_shr_s`, `wasm_i64_shr_u`
- `wasm_i64_clz`, `wasm_i64_ctz`, `wasm_i64_popcnt`
- `wasm_i64_rotl`, `wasm_i64_rotr`
- `wasm_i64_eqz`, `wasm_i64_eq`, `wasm_i64_ne`, comparisons (return i32)

For returning two values: use a shared mutable global (per-instance) or
encode the two i32 halves in the return. One approach: return `lo` as the
function return value, and write `hi` to a known location (e.g., an
AllocStackInst or a global helper variable). WasmIRGen emits code to read
the hi part after each i64 helper call.

**Depends on**: G.1, F.1

**Completion criteria**: All i64 arithmetic operations produce correct results,
including edge cases (overflow, underflow, division traps).

**Tests** (lit):
1. `i64.add(0x00000001_00000000, 0x00000001_00000000)` → `0x00000002_00000000`.
2. `i64.mul(0xFFFFFFFF, 0xFFFFFFFF)` → correct 64-bit result.
3. `i64.div_s(10, 0)` → trap.
4. `i64.shl(1, 63)` → `0x80000000_00000000`.
5. `i64.clz(1)` → 63.

### G.4: i64 conversion helpers

**What**: Implement i64 ↔ other type conversions:
- `i32.wrap_i64` → take lo32, discard hi32.
- `i64.extend_i32_s` → sign-extend i32 to i64: `lo = val, hi = (val >> 31)`.
- `i64.extend_i32_u` → zero-extend: `lo = val, hi = 0`.
- `i64.trunc_f32_s`, `i64.trunc_f32_u`, `i64.trunc_f64_s`, `i64.trunc_f64_u`
  → native helpers.
- `i64.trunc_sat_*` → saturating versions.
- `f32.convert_i64_s`, `f32.convert_i64_u` → native helpers.
- `f64.convert_i64_s`, `f64.convert_i64_u` → native helpers.
- `i64.reinterpret_f64`, `f64.reinterpret_i64` → bit-cast between i64 and f64.
- `i64.extend8_s`, `i64.extend16_s`, `i64.extend32_s` → sign extension.

**Depends on**: G.3, F.4

**Completion criteria**: All i64 conversion operations produce correct results.

**Tests**: Similar pattern to F.4 tests but for i64 range.

### G.5: i64 locals and control flow

**What**: Handle i64 in locals (local.get/set/tee) and control flow (block
results, phi nodes). Each i64 local needs **two** `AllocStackInst` slots. Each
i64 block result needs **two** `PhiInst` nodes.

Update `beginFunction()`, `onLocalGet()`, `onLocalSet()`, `onLocalTee()`, and
all control flow constructs (block, loop, if, br) to handle i64 correctly.

**Depends on**: G.1, D.6, D.7, D.8

**Completion criteria**: i64 values flow correctly through locals, blocks,
loops, and if/else.

**Tests** (lit):
1. Function with i64 parameter: `local.get 0` returns the correct i64 value.
2. i64 loop counter that counts past 2^32.
3. If/else returning i64 values.

---

## Part H: Linear Memory

### H.1: Memory access helpers (load/store)

**What**: Implement Wasm memory load/store operations using the asm.js typed
array pattern. The typed array views (`HEAP8`, `HEAPU8`, `HEAP16`, `HEAPU16`,
`HEAP32`, `HEAPU32`, `HEAPF32`, `HEAPF64`) are stored as `Variable`s in the
top-level scope (`topLevelVS_`), alongside the pre-created function closures.
The top-level function body creates the views and stores them via
`StoreFrameInst`. Each Wasm function loads them via `LoadFrameInst` from the
parent scope (using the same `GetParentScopeInst` already used for closures).

For each memory access instruction:
1. Pop base address from stack.
2. Compute effective address: `addr = base + offset` (offset is an immediate).
3. Check the alignment annotation (see H.3): if under-aligned, emit the
   byte-assembly path. Otherwise, use the typed array fast path:
4. Load the appropriate typed array view from the top-level scope via
   `LoadFrameInst`.
5. Compute typed array index: shift address by element size
   (e.g., `addr >> 2` for i32).
6. For **loads**: emit `GetByVal(typedArray, index)`, then compare the result
   to `undefined`. If equal, trap with "out of bounds memory access". This
   leverages the typed array's built-in bounds check (OOB reads return
   `undefined`) and is cheaper than a pre-access range computation.
7. For **stores**: emit `PutByVal(typedArray, index, value)` unconditionally.
   OOB writes are silently ignored by the typed array (known Phase 1 spec
   deviation — OOB stores don't trap). Phase 2's interpreter-level bounds
   check will handle this correctly.

**Note**: The typed array fast path assumes naturally aligned access. See H.3
for details on alignment handling and the known Phase 1 spec limitation.

Instructions to implement:
- `i32.load`, `i32.load8_s`, `i32.load8_u`, `i32.load16_s`, `i32.load16_u`
- `i32.store`, `i32.store8`, `i32.store16`
- `f32.load`, `f32.store`
- `f64.load`, `f64.store`
- `i64.load`, `i64.load8_s`, `i64.load8_u`, `i64.load16_s`, `i64.load16_u`,
  `i64.load32_s`, `i64.load32_u`
- `i64.store`, `i64.store8`, `i64.store16`, `i64.store32`

For sign-extending loads (`i32.load8_s`, etc.): load unsigned, then apply sign
extension: `(val << 24) >> 24` for 8-bit, `(val << 16) >> 16` for 16-bit.

**Depends on**: D.3, F.1

**Completion criteria**: Memory loads and stores work correctly for all data
types and widths.

**Tests** (lit):
1. Store i32 at address 0, load it back → same value.
2. Store i32 at address 4, load8_s at address 4 → lower byte, sign-extended.
3. Store 0xFF at address 0 (via i32.store8), load8_u → 255, load8_s → -1.
4. f64 store and load → preserves full precision.
5. Out-of-bounds load → trap.
6. Out-of-bounds store → trap.
7. Access at memory boundary (last valid address) → succeeds.

### H.2: memory.size and memory.grow

**What**:
- `memory.size` → returns current memory size in pages (helper call that reads
  the memory size from the instance state).
- `memory.grow delta` → helper call that:
  1. Computes `newPages = currentPages + delta`.
  2. Checks against maximum.
  3. Reallocates the backing buffer.
  4. Detaches old ArrayBuffer, creates new one.
  5. Re-creates typed array views and updates the corresponding `Variable`s
     in `topLevelVS_` via `StoreFrameInst`. Since every memory access loads
     the view via `LoadFrameInst`, subsequent accesses automatically see the
     new views.
  6. Returns old page count, or -1 on failure.

**Depends on**: H.1

**Completion criteria**: memory.grow correctly resizes memory, and subsequent
loads/stores work with the new size.

**Tests** (lit):
1. `memory.size` on initial 1-page memory → 1.
2. `memory.grow(1)` → returns 1 (old size). `memory.size` → 2.
3. Store data at address 65536 (second page) after grow → succeeds.
4. `memory.grow` beyond maximum → returns -1.

### H.3: Unaligned access handling

**What**: The Wasm spec requires that loads and stores work correctly
regardless of the effective address alignment. The alignment annotation in
each load/store instruction is a pessimizing hint: `align=1` means the
address may have any alignment; `align=4` means the producer believes the
address will be 4-byte aligned, but the spec requires correct results even
if it is not. See §4.5.2 of `WasmSupport.md` for full discussion.

Phase 1 approach: check the alignment annotation at compile time. If
`annotation < natural_alignment`, emit the byte-assembly path. If
`annotation >= natural_alignment`, use the typed array fast path.

The byte-by-byte path for `i32.load align=1`:
```
b0 = HEAPU8[addr]
b1 = HEAPU8[addr + 1]
b2 = HEAPU8[addr + 2]
b3 = HEAPU8[addr + 3]
result = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
```

**Known spec limitation**: If a naturally-annotated access (e.g.,
`i32.load align=4`) receives a misaligned address at runtime, Phase 1
will produce an incorrect result. The spec says this must still work, but
the typed array approach cannot handle it. In practice, producing compilers
(LLVM, etc.) emit correct alignment annotations, so this does not affect
real-world modules. Phase 2's `WasmLoad`/`WasmStore` bytecodes with raw
pointer access will eliminate this limitation.

**Depends on**: H.1

**Completion criteria**: Loads and stores with non-natural alignment
annotations produce correct results.

**Tests** (lit):
1. `i32.load align=1` at an odd address → correct value.
2. `f64.store` + `f64.load` at unaligned address (using `align=1`) →
   correct round-trip.
3. `i32.store align=2` + `i32.load align=2` at 2-byte-aligned but not
   4-byte-aligned address → correct value.

---

## Part I: Import/Export Trampolines

### I.1: Export wrapper functions

**What**: For each exported Wasm function, create a `NativeFunction` (or
`FinalizableNativeFunction`) that:
1. Validates argument count and types (Number → i32/f32/f64, BigInt → i64).
2. Marshals JS arguments to Wasm types.
3. Calls the compiled Wasm function.
4. Marshals the Wasm return value to JS (i32/f32/f64 → Number, i64 → BigInt).

These wrappers are created during `WebAssembly.instantiate()` and placed in
`instance.exports`.

**Depends on**: D.12

**Completion criteria**: JS code can call exported Wasm functions with correct
argument/return value marshaling.

**Tests** (lit):
1. Export a function `(i32, i32) -> i32`. Call from JS with `instance.exports.add(3, 4)` → 7.
2. Export a function with no params and no results. Call from JS.
3. Export a function returning f64. Verify JS gets the correct Number.

### I.2: Import trampoline functions

**What**: For each imported function, during instantiation:
1. Look up the JS function from the import object.
2. Create a trampoline Hermes IR function (or NativeFunction) that:
   a. Receives Wasm-typed arguments.
   b. Converts to JS types (Number for i32/f32/f64, BigInt for i64).
   c. Calls the JS function via `CallInst`.
   d. Converts the JS return value back to the expected Wasm type.
   e. If the JS function throws, the exception propagates.

For Phase 1, import trampolines are native functions created at instantiation
time and stored in the instance. WasmIRGen emits `CallInst` to a function
pointer loaded from the instance's import table.

**Depends on**: I.1

**Completion criteria**: Wasm code can call imported JS functions.

**Tests** (lit):
1. Import a JS function `env.log(i32)` that prints its argument. Wasm calls it.
   Verify output.
2. Import a JS function `env.add(i32, i32) -> i32`. Wasm calls it and uses the
   result.
3. Import a JS function that throws. Verify the exception propagates and can be
   caught by JS.

---

## Part J: Tables and call_indirect

### J.1: Table representation

**What**: Implement `WasmTable` as a GC-managed array of `HermesValue`:
- Store function references (as `Callable*` or `NativeFunction*`).
- Store the type index for each entry (for `call_indirect` type checking).
- Support `table.get`, `table.set`, `table.grow`, `table.size`.

During instantiation:
- Allocate tables with initial size.
- Apply element segments (copy function references into tables).

**Depends on**: B.4

**Completion criteria**: Tables can be created, populated, and accessed.

### J.2: call_indirect

**What**: Implement `call_indirect typeIdx tableIdx`:
1. Pop i32 index from stack.
2. Pop arguments from stack.
3. Bounds-check the index against table size → trap if out of bounds.
4. Load the function reference from the table → trap if null (uninitialized).
5. Type-check: compare the function's type index against the expected type
   index → trap if mismatch.
6. Call the function.
7. Push result.

For Phase 1, this is a helper-function call that performs all the checks and
the call.

**Depends on**: J.1, D.12

**Completion criteria**: `call_indirect` works correctly with type checking and
traps.

**Tests** (lit):
1. Table with 3 functions. `call_indirect` with index 0, 1, 2 → correct
   function called.
2. `call_indirect` with out-of-bounds index → trap.
3. `call_indirect` with null entry → trap.
4. `call_indirect` with type mismatch → trap.

---

## Part K: Globals

### K.1: Wasm globals

**What**: Implement Wasm globals:
- Store in a per-instance array of `HermesValue`.
- `global.get idx` → load from the globals array.
- `global.set idx` → store to the globals array (only if mutable).

During instantiation:
- For defined globals: evaluate init expressions and store results.
- For imported globals: use the provided value.

WasmIRGen emits helper calls for global access (or loads/stores from the
environment).

**Depends on**: B.4, F.1

**Completion criteria**: Globals can be read and written.

**Tests** (lit):
1. Mutable global: set to 42, get → 42.
2. Immutable global: initialized to 99, get → 99.
3. Global initialized from another global (via init expression).

---

## Part L: Exception Handling

### L.1: Wasm try/catch/throw

**What**: Map Wasm exception handling to Hermes IR:
- `try` → `TryStartInst(tryBody, catchBlock)`
- `catch tag` → `CatchInst` + check if caught value is a `WebAssembly.Exception`
  with matching tag. If not, re-throw.
- `catch_all` → `CatchInst`. Check if caught value is a
  `WebAssembly.RuntimeError` (trap) — if so, re-throw (traps bypass Wasm
  catch). Otherwise, handle.
- `throw tag` → Create `WebAssembly.Exception` object with tag and payload,
  `ThrowInst`.
- `rethrow depth` → Re-throw the caught exception from the referenced catch
  block.

**Depends on**: D.6

**Completion criteria**: Wasm exception handling works correctly, including the
trap-bypasses-catch rule.

**Tests** (lit):
1. `throw` a Wasm exception, `catch` with matching tag → caught.
2. `throw` a Wasm exception, `catch` with wrong tag → not caught, propagates.
3. `throw` in Wasm, `catch_all` → caught.
4. Trap (e.g., division by zero), `catch_all` → NOT caught by Wasm (re-thrown).
5. JS exception propagating through Wasm `try`/`catch_all` → caught.
6. Wasm exception propagating to JS `try`/`catch` → caught as
   `WebAssembly.Exception`.

---

## Part M: WebAssembly JS API

### M.1: WebAssembly error types

**What**: Define `WebAssembly.CompileError`, `WebAssembly.LinkError`,
`WebAssembly.RuntimeError` as error subclasses. Use the
`NATIVE_ERROR_TYPE` macro pattern from `NativeErrorTypes.def`:
- Add entries to a new `WasmErrorTypes.def` or directly to `NativeErrorTypes.def`.
- Or implement manually following the pattern in `Error.cpp`.

**Depends on**: A.1

**Completion criteria**: `new WebAssembly.CompileError("msg")` creates an error
object. `instanceof WebAssembly.CompileError` works. Same for LinkError and
RuntimeError.

**Tests** (lit):
1. `new WebAssembly.CompileError("test")` — verify name and message.
2. `err instanceof Error` → true.
3. `err instanceof WebAssembly.CompileError` → true.

### M.2: WebAssembly.validate

**What**: Implement `WebAssembly.validate(bytes)`:
1. Check that `bytes` is an `ArrayBuffer` or typed array view.
2. Call wabt's `ReadBinary` with a validation-only delegate (or our full
   delegate, discarding the result).
3. Return `true` if valid, `false` otherwise.

**Depends on**: C.1, M.1

**Completion criteria**: `WebAssembly.validate()` correctly identifies valid
and invalid Wasm modules.

**Tests** (lit):
1. Valid module → true.
2. Invalid module (bad magic) → false.
3. Invalid module (type error) → false.

### M.3: WebAssembly.Module

**What**: Implement `JSWebAssemblyModule` as a JSObject subclass (following
the pattern from B.4 of the VM exploration):
1. Define `CellKind::JSWebAssemblyModuleKind` in `CellKinds.def`.
2. Implement `JSWebAssemblyModule` class with `WasmModuleInfo` and compiled
   bytecode.
3. Constructor: `new WebAssembly.Module(bytes)` → parse and compile.
4. Static methods: `WebAssembly.Module.exports(module)`,
   `WebAssembly.Module.imports(module)`.

**Depends on**: C.2, M.1

**Completion criteria**: `new WebAssembly.Module(bytes)` creates a module
object. `.exports()` and `.imports()` return correct metadata.

**Tests** (lit):
1. Create module from valid bytes. `WebAssembly.Module.exports(mod)` returns
   correct export list.
2. Create module from invalid bytes → throws `WebAssembly.CompileError`.

### M.4: WebAssembly.Instance

**What**: Implement `JSWebAssemblyInstance` with the full instantiation process
(described in design doc §4.8.5):
1. Validate imports.
2. Allocate memories.
3. Allocate tables.
4. Initialize globals.
5. Apply element segments.
6. Apply data segments.
7. Execute start function.
8. Build exports object.

**Depends on**: M.3, I.1, I.2, H.1, J.1, K.1

**Completion criteria**: `new WebAssembly.Instance(module, imports)` produces a
working instance with callable exports.

**Tests** (lit):
1. Instantiate module with no imports → exports work.
2. Instantiate module with function imports → imports are called correctly.
3. Instantiate with memory import → shared memory works.
4. Instantiate with missing import → throws LinkError.
5. Data segment initialization → memory contains correct bytes.
6. Element segment initialization → table contains correct functions.
7. Start function runs during instantiation.

### M.5: WebAssembly.Memory

**What**: Implement `JSWebAssemblyMemory`:
- Constructor: `new WebAssembly.Memory({initial: N, maximum: M})`.
- `memory.buffer` getter → returns the `ArrayBuffer`.
- `memory.grow(delta)` → grows, returns old page count.

**Depends on**: H.2

**Completion criteria**: Memory objects can be created, shared with instances,
grown, and their buffer accessed from JS.

**Tests** (lit):
1. Create memory with initial 1 page. `memory.buffer.byteLength` → 65536.
2. `memory.grow(1)` → returns 1. `memory.buffer.byteLength` → 131072.
3. Share memory between JS and Wasm: JS writes to buffer, Wasm reads it.

### M.6: WebAssembly.Table

**What**: Implement `JSWebAssemblyTable`:
- Constructor: `new WebAssembly.Table({element: "anyfunc", initial: N, maximum: M})`.
- `table.get(idx)` → returns the function at index, or null.
- `table.set(idx, func)` → sets the function at index.
- `table.grow(delta)` → grows the table.
- `table.length` getter.

**Depends on**: J.1

**Completion criteria**: Table objects work with the JS API.

**Tests** (lit):
1. Create table, set a function, get it back.
2. `table.length` returns correct value.
3. `table.grow` works.

### M.7: WebAssembly.Global

**What**: Implement `JSWebAssemblyGlobal`:
- Constructor: `new WebAssembly.Global({value: "i32", mutable: true}, 42)`.
- `global.value` getter/setter.

**Depends on**: K.1

**Completion criteria**: Global objects work with the JS API.

**Tests**: Create mutable global, modify via `.value`, verify change.

### M.8: WebAssembly.compile and WebAssembly.instantiate

**What**: Implement the async API:
- `WebAssembly.compile(bytes)` → returns `Promise<Module>`. Since Hermes doesn't
  do async compilation, this is synchronous compilation wrapped in a resolved
  Promise.
- `WebAssembly.instantiate(bytes, imports)` → returns
  `Promise<{module, instance}>`.
- `WebAssembly.instantiate(module, imports)` → returns `Promise<instance>`.

**Depends on**: M.3, M.4

**Completion criteria**: The async API works (with synchronous compilation under
the hood).

**Tests** (lit):
1. `WebAssembly.compile(bytes).then(mod => ...)` → works.
2. `WebAssembly.instantiate(bytes, imports).then(({module, instance}) => ...)`.

### M.9: WebAssembly.Exception and WebAssembly.Tag

**What**: Implement exception handling JS API:
- `WebAssembly.Tag` → represents an exception tag type.
- `WebAssembly.Exception` → represents a thrown Wasm exception with tag and
  payload.
- `exception.is(tag)` → checks if exception matches tag.
- `exception.getArg(tag, index)` → extracts payload value.

**Depends on**: L.1, M.1

**Completion criteria**: Wasm exceptions are interoperable with JS.

**Tests**: Throw Wasm exception, catch in JS, check `exception.is(tag)` and
`exception.getArg(tag, 0)`.

---

## Part N: Bulk Memory Operations

### N.1: memory.fill, memory.copy, memory.init, data.drop

**What**: Implement as helper function calls:
- `memory.fill(dest, value, count)` → `memset` the linear memory region.
  Bounds-check first.
- `memory.copy(dest, src, count)` → `memmove` within linear memory.
  Bounds-check both regions. Handle overlapping correctly.
- `memory.init(segIdx, dest, src, count)` → copy from a data segment into
  linear memory.
- `data.drop(segIdx)` → mark a data segment as dropped (prevents further
  `memory.init` with it).

**Depends on**: H.1

**Completion criteria**: All bulk memory operations work correctly.

**Tests** (lit):
1. `memory.fill(0, 0xFF, 100)` → first 100 bytes are 0xFF.
2. `memory.copy(100, 0, 50)` → bytes 100-149 match bytes 0-49.
3. `memory.copy` with overlapping regions (src < dest) → correct result.
4. `memory.init` from data segment → correct bytes copied.
5. `data.drop` + `memory.init` → trap.

### N.2: table.fill, table.copy, table.init, elem.drop

**What**: Analogous to N.1 but for tables:
- `table.fill(idx, val, count)` → fill table entries.
- `table.copy(destTable, srcTable, dest, src, count)` → copy between tables.
- `table.init(segIdx, dest, src, count)` → copy from element segment.
- `elem.drop(segIdx)` → drop element segment.

**Depends on**: J.1

**Completion criteria**: Table bulk operations work correctly.

**Tests**: Similar pattern to N.1 tests.

---

## Part O: Testing with Spec Test Suite

### O.1: Set up Wasm spec test runner

**What**: The official Wasm spec test suite (github.com/WebAssembly/spec) uses
`.wast` files that contain test assertions. Set up a test runner that:
1. Converts `.wast` files to `.wasm` + JS assertions (using `%wast2json` from
   A.7).
2. Compiles each `.wasm` module with `hermesc --wasm`.
3. Runs the JS assertion harness with `hermes`.

Alternatively, write a custom `.wast` interpreter that drives compilation and
assertion checking.

**Depends on**: A.7, D.14, M.4

**Completion criteria**: The spec test runner can execute at least the basic
test files (`i32.wast`, `f64.wast`, `block.wast`, etc.).

### O.2: Progressively pass spec tests

**What**: Track which spec test files pass and which fail. Create a skip list
for known failures. Work through failures one by one, fixing issues as they
arise.

Key spec test files (in rough order of difficulty):
1. `i32.wast` — i32 arithmetic
2. `i64.wast` — i64 arithmetic
3. `f32.wast`, `f64.wast` — floating point
4. `block.wast`, `loop.wast`, `if.wast` — control flow
5. `br.wast`, `br_if.wast`, `br_table.wast` — branches
6. `call.wast`, `call_indirect.wast` — function calls
7. `local_get.wast`, `local_set.wast`, `local_tee.wast` — locals
8. `global.wast` — globals
9. `memory.wast` — linear memory
10. `select.wast` — select instruction
11. `conversions.wast` — type conversions
12. `func.wast` — function declarations
13. `type.wast` — type section
14. `imports.wast`, `exports.wast` — imports/exports
15. `table.wast`, `elem.wast` — tables and elements
16. `data.wast` — data segments
17. `start.wast` — start function
18. `unreachable.wast`, `traps.wast` — trap behavior
19. `nop.wast`, `return.wast`, `stack.wast` — misc
20. `names.wast` — name section
21. `linking.wast` — module linking
22. `binary.wast` — binary format edge cases

**Depends on**: O.1

**Completion criteria**: All spec test files pass (modulo features we
explicitly don't support, like SIMD). Maintain a CI target
`check-hermes-wasm` that runs the Wasm spec tests.

---

## Dependency Graph Summary

```
A.1 → A.2 → A.3 → A.6
              ↓
A.1 → A.4
A.3,A.4 → A.5

B.1 → B.2 → B.3 → B.4

A.2,B.4 → C.1 → C.2 → C.2.1

A.4,B.4 → D.1
D.1,C.1 → D.1.1
D.1,D.1.1 → D.2 → D.3 → D.4 → D.5 → D.6 → D.7, D.8, D.9
                                        ↓
                                  D.10, D.11
                      D.2 → D.12
          C.1,D.1.1,D.2-D.12 → D.13 → D.14

D.3 → E.1 → E.2
D.4,E.1 → E.3

D.1 → F.1 → F.2, F.3, F.4, F.5

D.2 → G.1 → G.2, G.3, G.4, G.5

D.3,F.1 → H.1 → H.2, H.3

D.12 → I.1, I.2

B.4 → J.1 → J.2
B.4,F.1 → K.1

D.6 → L.1

A.1 → M.1 → M.2, M.3, M.4, M.5, M.6, M.7, M.8, M.9

H.1 → N.1
J.1 → N.2

D.14,M.4 → O.1 → O.2
```

---

## Recommended Implementation Order

The plan above lists steps by component. For actual development, interleave
the components to reach working end-to-end milestones as early as possible:

### Milestone 1: Minimal End-to-End (Weeks 1-3)
A.1 → A.2 → A.3 → A.4 → A.5 → A.6
B.1 → B.2 → B.3 → B.4
C.1 → C.2 → C.2.1
D.1 → D.1.1 → D.2 → D.3 → D.5 → D.12 → D.13 → D.14

**Result**: Can compile a `.wasm` file with simple arithmetic and run it.
`--dump-ir` tests verify IR structure at each step.

### Milestone 2: Control Flow (Weeks 3-5)
D.4 → D.6 → D.7 → D.8 → D.9 → D.10 → D.11

**Result**: Loops, branches, if/else all work.

### Milestone 3: Full i32 + f64 (Weeks 5-7)
E.1 → E.2 → E.3
F.1 → F.2 → F.3 → F.4 → F.5

**Result**: All i32 and f64 operations work, including trapping operations.

### Milestone 4: Memory and Globals (Weeks 7-9)
H.1 → H.2 → H.3
K.1

**Result**: Linear memory and globals work.

### Milestone 5: Imports, Exports, Tables (Weeks 9-11)
I.1 → I.2
J.1 → J.2

**Result**: Module interop with JS works. Indirect calls work.

### Milestone 6: i64 (Weeks 11-13)
G.1 → G.2 → G.3 → G.4 → G.5

**Result**: i64 operations work (slow but correct).

### Milestone 7: Exception Handling (Weeks 13-14)
L.1

### Milestone 8: JS API (Weeks 14-17)
M.1 → M.2 → M.3 → M.4 → M.5 → M.6 → M.7 → M.8 → M.9

**Result**: Full WebAssembly JS API.

### Milestone 9: Bulk Memory + Testing (Weeks 17-20)
N.1 → N.2
O.1 → O.2

**Result**: Spec test suite passes.

---

## Risk Areas and Mitigations

1. **i64 split representation complexity**: Every i64 operation doubles the
   register pressure and complicates control flow (two Phi nodes per i64 merge
   point). Mitigation: thoroughly test with the i64 spec tests early.

2. **wabt API stability**: wabt's `BinaryReaderDelegate` API may change between
   versions. Mitigation: vendor a specific tagged release and document the
   version.

3. **Typed array performance**: Using `GetByVal`/`PutByVal` on typed arrays for
   every memory access may be very slow. Mitigation: this is acceptable for
   Phase 1; Phase 2 adds specialized bytecodes.

4. **Register pressure**: Complex Wasm functions may exceed 256 registers. The
   BCGen register allocator falls back to Reg32 variants, but some bytecodes may
   not have Reg32 variants. Mitigation: audit which bytecodes Wasm-generated
   code uses and verify Reg32 variants exist.

5. **memory.grow invalidating typed array views**: After memory.grow, all
   cached typed array view references become stale. Generated code must reload
   views after any call that might trigger memory.grow. Mitigation: treat
   memory.grow as an invalidation point; reload views after every call (safe
   but slow) or use escape analysis to determine which calls can trigger grow.

6. **Multi-value returns**: Phase 1 only supports single-value returns for both
   functions and blocks. Some Wasm modules may use multi-value. Mitigation:
   detect and reject multi-value modules with a clear error message.
