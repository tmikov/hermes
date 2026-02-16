# Project: WebAssembly Support for Hermes: Design Document

You are adding WebAssembly (Wasm) support to the Hermes JavaScript engine.

## 1. Executive Summary

This document describes a design for adding WebAssembly (Wasm) support to the
Hermes JavaScript engine. The approach treats Wasm as an alternative frontend to
the existing Hermes compiler pipeline: Wasm binary modules are parsed, validated,
and translated into Hermes IR (the same SSA-based intermediate representation
used by the JavaScript compiler), then passed through optimization and bytecode
generation to produce Hermes bytecodes that run on the existing VM.

This strategy maximizes reuse of the existing compiler infrastructure (optimizer,
register allocator, bytecode generator, bytecode file format) while requiring
targeted extensions to the IR, bytecodes, and runtime for Wasm-specific
semantics.

The fundamental challenge is that Hermes is designed for a dynamically-typed
language where all values are NaN-boxed `HermesValue` (64-bit), while Wasm is
statically typed with four core value types (i32, i64, f32, f64). This document
proposes a phased approach: an initial implementation using existing mechanisms
(encoding Wasm values in `HermesValue` with helper-function calls for missing
operations), followed by performance-oriented extensions (new typed IR
instructions, new bytecodes, and possible typed-register support).

## 2. Goals and Non-Goals

### Goals

- **Correctness**: Full conformance with the WebAssembly MVP specification, plus
  widely-supported Wasm 2.0+ features (multi-value, bulk memory, reference types,
  exception handling).
- **Pipeline reuse**: Wasm compilation flows through the existing Hermes IR →
  optimizer → bytecode generator pipeline.
- **JS interop**: Implement the standard `WebAssembly` JS API
  (`WebAssembly.Module`, `WebAssembly.Instance`, `WebAssembly.Memory`, etc.) so
  JS code can instantiate and call Wasm modules.
- **Ahead-of-time compilation**: Wasm modules can be compiled to Hermes bytecode
  files (`.hbc`) ahead of time, consistent with Hermes's design philosophy.
- **Incremental delivery**: A phased implementation plan that delivers a
  correct-but-slow MVP first, then adds performance optimizations.

### Non-Goals (for initial phases)

- **SIMD (v128)**: ~236 additional instructions with no existing Hermes IR
  support. Deferred to a future phase.
- **Wasm GC (Wasm 3.0)**: Struct and array types with garbage collection.
  Deferred.
- **Threads and shared memory**: Requires SharedArrayBuffer and atomics. Deferred.
- **memory64**: 64-bit memory addressing. Deferred.
- **Near-native performance**: The initial phases prioritize correctness over
  performance. Performance parity with dedicated Wasm engines is a long-term
  aspiration, not an initial requirement.

## 3. Architecture Overview

```
                    ┌──────────────────────────────┐
                    │       Wasm Binary (.wasm)     │
                    └──────────────┬───────────────┘
                                   │
                    ┌──────────────▼───────────────┐
                    │   Wasm Binary Reader           │
                    │   (wabt BinaryReaderDelegate;  │
                    │    custom decoder in future)    │
                    └──────────────┬───────────────┘
                                   │
                    ┌──────────────▼───────────────┐
                    │   WasmModuleInfo (new)         │
                    │   (types, imports, exports,    │
                    │    memories, tables, globals,  │
                    │    data segments, elements)    │
                    └──────────────┬───────────────┘
                                   │
                    ┌──────────────▼───────────────┐
                    │   WasmIRGen (new)              │
                    │   Wasm functions → Hermes IR  │
                    │   (stack machine → SSA)        │
                    └──────────────┬───────────────┘
                                   │
              ┌────────────────────▼────────────────────┐
              │          Existing Hermes Pipeline        │
              │                                          │
              │  ┌─────────────┐  ┌──────────────────┐  │
              │  │  Optimizer   │  │  BCGen (register  │  │
              │  │  (passes)    │→ │  alloc, lowering, │  │
              │  │              │  │  bytecode emit)   │  │
              │  └─────────────┘  └──────────────────┘  │
              └────────────────────┬────────────────────┘
                                   │
                    ┌──────────────▼───────────────┐
                    │   Hermes Bytecode (.hbc)       │
                    │   (Wasm bytecodes; can share   │
                    │    file with JS in future)      │
                    └──────────────┬───────────────┘
                                   │
              ┌────────────────────▼────────────────────┐
              │          Hermes VM Runtime               │
              │                                          │
              │  ┌──────────────┐  ┌─────────────────┐  │
              │  │  Interpreter  │  │  Wasm Runtime    │  │
              │  │  (existing)   │  │  Support (new):  │  │
              │  │               │  │  - Linear memory │  │
              │  │               │  │  - Tables        │  │
              │  │               │  │  - Globals       │  │
              │  │               │  │  - Trap handling │  │
              │  └──────────────┘  └─────────────────┘  │
              └────────────────────────────────────────┘
```

The pipeline has these new components:

1. **Wasm Binary Reader** — Decodes the `.wasm` binary format, validates types
   and structure per the spec. Phase 1 uses wabt's `BinaryReaderDelegate`;
   can be replaced with a custom decoder later (see §4.1).
2. **WasmModuleInfo** — An in-memory representation of the parsed Wasm module
   (analogous to the AST for JavaScript).
3. **WasmIRGen** — Translates each Wasm function body from the stack machine
   encoding into Hermes IR (SSA form with BasicBlocks).
4. **Wasm Runtime Support** — Runtime data structures for linear memory, tables,
   globals, and trap handling, integrated with the existing Hermes VM.
5. **WebAssembly JS API** — JS-visible API objects (`WebAssembly.Module`, etc.)
   implemented as native functions/objects.

## 4. Component Design

### 4.1 Wasm Binary Parser

#### 4.1.1 Approach: wabt `BinaryReaderDelegate` (Phase 1)

Phase 1 uses the binary reader from **wabt** (WebAssembly Binary Toolkit) as
the Wasm decoder. wabt is the official reference toolkit maintained by the
WebAssembly project (Apache-2.0 license, C++17, minimal dependencies).

wabt's binary reader exposes a visitor/delegate API designed exactly for this
use case:

```cpp
// wabt entry point:
Result ReadBinary(const void* data, size_t size,
                  BinaryReaderDelegate* reader,
                  const ReadBinaryOptions& options);
```

`BinaryReaderDelegate` is an abstract interface with callbacks for every
section and instruction. `BinaryReaderNop` provides a no-op base class so
only relevant callbacks need to be overridden. Existing implementations in
wabt include `BinaryReaderIR` (builds wabt's internal AST) and
`BinaryReaderInterp` (feeds into wabt's interpreter).

For Hermes, we implement a **`BinaryReaderHermesIRGen`** subclass that
populates a `WasmModuleInfo` from module-level callbacks and drives
`WasmIRGen` from instruction callbacks:

```cpp
class BinaryReaderHermesIRGen : public wabt::BinaryReaderNop {
  WasmModuleInfo &moduleInfo_;
  WasmIRGen &irGen_;

  // Module structure callbacks → populate WasmModuleInfo
  Result OnTypeCount(Index count) override;
  Result OnFuncType(Index index, ...) override;
  Result OnImport(Index index, ...) override;
  Result OnExport(Index index, ...) override;
  Result OnMemory(Index index, ...) override;
  // ...

  // Function body callbacks → drive WasmIRGen
  Result BeginFunctionBody(Index index, Offset size) override;
  Result OnLocalDecl(Index decl_index, Index count, Type type) override;

  // Instruction callbacks → emit Hermes IR via WasmIRGen
  Result OnI32ConstExpr(uint32_t value) override;
  Result OnBinaryExpr(Opcode opcode) override;
  Result OnLoadExpr(Opcode opcode, ...) override;
  Result OnCallExpr(Index func_index) override;
  Result OnIfExpr(Type sig_type) override;
  Result OnBrExpr(Index depth) override;
  // ... one callback per instruction category
};
```

**Why wabt**: wabt's reader is battle-tested, spec-compliant, supports 16+
Wasm proposals (exception handling, multi-value, bulk memory, reference types,
etc.), and handles validation interleaved with parsing. This lets us focus
engineering effort on the hard novel part — Wasm→Hermes IR translation — rather
than on binary decoding.

**Integration**: wabt uses CMake and can be pulled in via `add_subdirectory()`
or as a vendored dependency. Only the binary reader subset is needed, not the
full toolkit (wat2wasm, wasm-interp, etc.). Key source files: `binary-reader.cc`,
`binary-reader-nop.h`, `leb128.cc`, `opcode.cc`, `type.cc`, `error.cc`.

**Location**: `lib/WasmFrontend/` (new directory) — contains
`BinaryReaderHermesIRGen` and the top-level `compileWasmModule()` entry point.
wabt sources are in `external/wabt/` (vendored) or pulled via CMake
`FetchContent`.

#### 4.1.2 Wasm Sections

The following sections are processed via `BinaryReaderDelegate` callbacks
(listed in specification order):

| Section ID | Name      | Contents                                          |
|-----------|-----------|---------------------------------------------------|
| 0         | Custom    | Name section, debug info (informational only)     |
| 1         | Type      | Function type signatures `(params) → (results)`   |
| 2         | Import    | Imported functions, tables, memories, globals      |
| 3         | Function  | Type index for each function defined in this module|
| 4         | Table     | Table type declarations                            |
| 5         | Memory    | Memory type declarations (initial/max pages)       |
| 6         | Global    | Global variable declarations with init expressions |
| 7         | Export    | Exported names → (kind, index)                     |
| 8         | Start     | Optional start function index                      |
| 9         | Element   | Table initialization segments                      |
| 10        | Code      | Function bodies (locals + instructions)            |
| 11        | Data      | Memory initialization segments                     |
| 12        | DataCount | Count of data segments (for validation; from bulk memory proposal, but required by many toolchains) |

Validation is performed by wabt during reading (single-pass, interleaved with
parsing): type checking, branch target validation, index bounds checking,
import/export type matching, and abstract value stack consistency at control
flow merge points.

#### 4.1.3 Future: Custom Wasm Decoder

If the wabt callback-per-instruction overhead becomes a bottleneck, or if
tighter integration is desired, the reader can be replaced with a custom
decoder. All major JS engines (V8, SpiderMonkey, JSC) roll their own decoders
for this reason — they fuse parsing with code generation in a single tight
loop, avoiding the overhead of virtual dispatch per instruction.

The Wasm binary format is designed to be simple to decode:

- LEB128 integers for all sizes and indices
- Linear section-based layout: magic + version, then typed sections
- ~170 MVP opcodes (single byte), plus prefixed opcodes for extensions
  (`0xFB` GC, `0xFC` bulk memory, `0xFD` SIMD, `0xFE` atomics)
- Designed for single-pass decoding — structured control flow means no
  back-patching

A custom decoder would be a straightforward state machine that reads opcodes,
decodes LEB128 immediates, and calls directly into `WasmIRGen` methods (no
virtual dispatch). The primary engineering cost is the sheer number of opcodes
(~500 including all proposals) and the validation rules.

The migration path is clean: `WasmIRGen` is designed against its own interface,
not against wabt's `BinaryReaderDelegate`. The `BinaryReaderHermesIRGen`
adapter is a thin shim that translates wabt callbacks to `WasmIRGen` calls.
Replacing wabt with a custom decoder only changes the driver, not the IR
generation logic.

### 4.2 WasmModuleInfo

**Location**: `include/hermes/WasmFrontend/WasmModuleInfo.h`

```cpp
/// Represents a parsed and validated Wasm module.
struct WasmModuleInfo {
  /// Function type signatures: (vec<ValType> params, vec<ValType> results).
  std::vector<WasmFuncType> types;

  /// Imported entities (functions, tables, memories, globals).
  std::vector<WasmImport> imports;

  /// Functions defined in this module (type index + body).
  std::vector<WasmFunction> functions;

  /// Table declarations.
  std::vector<WasmTableType> tables;

  /// Memory declarations (initial/max pages).
  std::vector<WasmMemoryType> memories;

  /// Global variable declarations.
  std::vector<WasmGlobal> globals;

  /// Exported entities.
  std::vector<WasmExport> exports;

  /// Optional start function index.
  std::optional<uint32_t> startFunction;

  /// Element segments (table initializers).
  std::vector<WasmElemSegment> elements;

  /// Data segments (memory initializers).
  std::vector<WasmDataSegment> dataSegments;

  /// Name section data (for debugging).
  WasmNameSection names;
};

enum class WasmValType : uint8_t {
  I32 = 0x7F,
  I64 = 0x7E,
  F32 = 0x7D,
  F64 = 0x7C,
  V128 = 0x7B,      // SIMD (future)
  FuncRef = 0x70,
  ExternRef = 0x6F,
};
```

### 4.3 WasmIRGen: Wasm → Hermes IR Translation

**Location**: `lib/WasmIRGen/` (new directory)

This is the core of the design. WasmIRGen converts each Wasm function into a
Hermes IR `Function` containing `BasicBlock`s and `Instruction`s in SSA form.

#### 4.3.1 Stack Machine to SSA Translation

Wasm is a stack machine; Hermes IR is SSA (Static Single Assignment) with
explicit registers. The translation uses a standard algorithm:

1. **Abstract Value Stack**: Maintain a stack of `Value*` (Hermes IR values).
   Each Wasm instruction pops its operands from this stack and pushes its
   results.

2. **Basic Block Creation**: Each Wasm structured control flow construct
   (`block`, `loop`, `if/else`) creates new `BasicBlock`s:
   - `block`: creates continuation block (branch target = after end)
   - `loop`: creates loop header block (branch target = loop top)
   - `if/else`: creates then-block, else-block, and merge-block

3. **Phi Nodes at Merge Points**: When control flow merges (end of `block`,
   end of `if/else`, loop back-edge), `PhiInst` nodes reconcile divergent
   values from different predecessors.

4. **Branch Handling**: `br`, `br_if`, `br_table` become `BranchInst`,
   `CondBranchInst`, and `SwitchInst` respectively, targeting the appropriate
   `BasicBlock`.

**Example**: Translating a simple Wasm function `(i32, i32) → i32` that adds
two parameters:

```wasm
(func (param i32) (param i32) (result i32)
  local.get 0
  local.get 1
  i32.add)
```

Produces Hermes IR approximately:

```
function wasm_func_0(p0, p1):
entry:
  %0 = AsInt32Inst %p0         ; ensure i32
  %1 = AsInt32Inst %p1         ; ensure i32
  %2 = FAddInst %0, %1         ; double addition
  %3 = AsInt32Inst %2          ; truncate to i32 (asm.js pattern: (a+b)|0)
  %4 = ReturnInst %3
```

#### 4.3.2 Control Flow Translation

Wasm's structured control flow maps to Hermes IR BasicBlocks as follows:

**`block`/`end`**:
```
Wasm:                    Hermes IR:
  block (result i32)       BB_before:
    ...body...               ...body...
    br 0  ──────────────►    BranchInst BB_after
  end                      BB_after:
                             %result = PhiInst [BB_before, val]...
```

**`loop`/`end`**:
```
Wasm:                    Hermes IR:
  loop                     BB_header:
    ...body...               %phi = PhiInst [BB_before, init], [BB_body, updated]
    br_if 0 ────────────►   CondBranchInst %cond, BB_header, BB_cont
  end                      BB_cont:
```

**`if`/`else`/`end`**:
```
Wasm:                    Hermes IR:
  if (result i32)          BB_before:
    ...then...               CondBranchInst %cond, BB_then, BB_else
  else                     BB_then:
    ...else...               ...then...
  end                        BranchInst BB_merge
                           BB_else:
                             ...else...
                             BranchInst BB_merge
                           BB_merge:
                             %result = PhiInst [BB_then, v1], [BB_else, v2]
```

**`br_table`** (switch): Translates to `SwitchInst` with one case per label.

**Key insight**: Because Wasm only has structured control flow (no arbitrary
gotos), the translation to BasicBlocks with branches is always possible and
produces reducible CFGs, which is ideal for Hermes's optimizer.

#### 4.3.3 Wasm Locals ↔ Hermes IR Variables

Wasm locals (including parameters) are mutable, while SSA values are not. The
translation handles this by initially treating Wasm locals as memory locations
(similar to how Hermes JS IRGen handles `let`/`var`):

- Each Wasm local gets an `AllocStackInst` (stack-allocated variable).
- `local.get` → `LoadStackInst`
- `local.set` → `StoreStackInst`
- `local.tee` → `StoreStackInst` + the value remains on the abstract stack.

The existing `Mem2Reg` optimization pass then promotes these to SSA registers,
inserting `PhiInst` at join points. This is the standard approach and works
well because Wasm locals have no address-taking (they cannot escape).

### 4.4 Type Mapping: Wasm Values in HermesValue

This is the most fundamental challenge. All Hermes VM registers hold
`HermesValue` — a 64-bit NaN-boxed value designed for JavaScript's dynamic
type system. Wasm values are statically typed and unboxed.

#### 4.4.1 Encoding Strategy

| Wasm Type | HermesValue Encoding | Notes |
|-----------|---------------------|-------|
| `i32`     | `double` (Number)   | All i32 values are exactly representable as double. Encode with `HermesValue::encodeTrustedNumberValue((double)val)`. |
| `i64`     | **Not directly representable** | Doubles cannot represent all i64 values. Requires special handling (see below). |
| `f32`     | `double` (Number)   | Promote to f64. Must round back to f32 after each f32 operation to maintain correct rounding behavior. |
| `f64`     | `double` (Number)   | Natural fit. Direct encoding. |
| `funcref` | `Object` (pointer)  | Reference to a wrapper object. |
| `externref`| `HermesValue` (any)  | Opaque JS value passed through. |

#### 4.4.2 The i32 Challenge

Storing i32 as double is exact (all 2^32 unsigned values and all signed i32
values fit in a double). However, Wasm i32 semantics differ from JS number
semantics:

- **Wrapping arithmetic**: `i32.add` wraps at 32 bits. `(double)a + (double)b`
  does not wrap. We must apply `ToInt32` (or mask with 0xFFFFFFFF) after each
  i32 arithmetic operation.
- **Unsigned operations**: `i32.div_u`, `i32.rem_u`, `i32.shr_u`, comparisons
  like `i32.lt_u` require treating the 32 bits as unsigned. JS `>>>` and
  `ToUint32` handle some cases; others need explicit handling.
- **Trapping division**: `i32.div_s` traps on division by zero and on
  `INT32_MIN / -1`. JS division returns `Infinity` or `-Infinity`. We must add
  explicit trap checks.
- **Bitwise operations**: `i32.and`, `i32.or`, `i32.xor`, `i32.shl`,
  `i32.shr_s`, `i32.shr_u` map well to JS bitwise operators (which already
  operate on int32).
- **Rotate, clz, ctz, popcnt**: No JS equivalents. Must be implemented as
  helper calls or new instructions.

**Existing Hermes support**: `AsInt32Inst`, `AsUint32Inst` convert values to
int32/uint32 (matching JS `ToInt32`/`ToUint32`). The bytecodes `ToInt32`,
`ToUint32` exist. The bitwise bytecodes (`BitAnd`, `BitOr`, `BitXor`, `LShift`,
`RShift`, `URshift`) all operate on int32. These cover a good portion of i32
operations.

**Missing**: Trapping division, unsigned division/remainder, rotate,
clz/ctz/popcnt, and explicit i32 wrapping for add/sub/mul. Note that unsigned
comparisons (`i32.lt_u`, etc.) do **not** need helpers — applying `AsUint32Inst`
to both operands converts them to their unsigned double representation (e.g.,
-1.0 → 4294967295.0), after which the standard `Less`/`Greater` comparison
bytecodes produce the correct unsigned result.

#### 4.4.3 The i64 Problem

This is the most significant impedance mismatch. A `double` has only 53 bits of
integer precision, so it cannot represent all i64 values (which require 64 bits).

**Options**:

1. **Split into two i32 values** (lo32/hi32): Each i64 Wasm value becomes two
   HermesValue registers, each holding an i32-as-double. Every i64 operation
   becomes a sequence of i32 operations. This is correct and requires no VM
   changes, but doubles register pressure and makes i64 operations slow
   (4-10x more instructions per operation).

2. **Use BigInt**: Hermes has BigInt support. This is semantically correct and
   matches the JS-Wasm type mapping (i64 ↔ BigInt). However, BigInt operations
   allocate on the heap and are orders of magnitude slower than native i64
   operations.

3. **Box in an external allocation**: Allocate an 8-byte buffer (or use a
   lightweight GC cell) to hold the i64, and store a pointer in HermesValue.
   Correct but slow due to allocation and indirection.

4. **GC-excluded register region**: Partition each Hermes function's register
   frame into a GC-scanned region (for normal HermesValues) and a GC-excluded
   region that can hold raw 64-bit integers. Since these slots are invisible
   to the GC, they can contain arbitrary bit patterns as long as they are never
   pointers to the GC heap. This requires VM-level changes (the register
   allocator must track which slots are GC-excluded, and the GC must know the
   boundary) but avoids heap allocation and provides native i64 performance.
   New bytecodes for i64 operations would read/write the GC-excluded slots
   directly.

Note that packing a raw i64 into the NaN-boxed HermesValue bits is **not**
feasible: a 64-bit integer requires all 64 bits, leaving no room for the tag
bits that HermesValue needs to distinguish value types.

**Recommendation**: Phase 1 uses option (1) — split i32 pairs — for correctness
with no VM changes. Phase 3 implements option (4) — GC-excluded register
slots — for performance (see §8 for the full phase plan).

#### 4.4.4 The f32 Challenge

Storing f32 as f64 is lossless for the value itself, but f32 **operations**
produce f32-precision results. For example, `f32.add(a, b)` must produce the
result as if the computation were done in single precision, not double.

**Solution**: After each f32 arithmetic operation, apply a narrowing step:
`result = (float)(double_result)` (equivalent to `Math.fround()`). This
correctly rounds to f32 precision.

**Hermes support**: No existing `Math.fround` bytecode, but it can be
implemented as a cast: `(double)(float)value`. For Phase 1, this can be a
helper-function call. For Phase 2, a new `FRoundInst` / `FRound` bytecode.

#### 4.4.5 f64

Direct mapping — no issues. Hermes `double` == Wasm `f64`. The existing
`FAddInst`, `FSubtractInst`, `FMultiplyInst`, `FDivideInst` instructions and
their corresponding `AddN`, `SubN`, `MulN`, `DivN` bytecodes operate on doubles
and can be reused directly.

### 4.5 Linear Memory

Wasm linear memory is a contiguous, byte-addressable, bounds-checked memory
region.

#### 4.5.1 Backing Storage

Allocate a `JSArrayBuffer` for the linear memory and create typed array views
over it, following the same pattern asm.js uses:

```cpp
// During Wasm instantiation:
auto buffer = JSArrayBuffer::create(runtime, initialPages * 65536);

// Create typed array views for each access width:
auto HEAP8   = JSTypedArray<int8_t>::create(runtime, buffer);
auto HEAPU8  = JSTypedArray<uint8_t>::create(runtime, buffer);
auto HEAP16  = JSTypedArray<int16_t>::create(runtime, buffer);
auto HEAPU16 = JSTypedArray<uint16_t>::create(runtime, buffer);
auto HEAP32  = JSTypedArray<int32_t>::create(runtime, buffer);
auto HEAPU32 = JSTypedArray<uint32_t>::create(runtime, buffer);
auto HEAPF32 = JSTypedArray<float>::create(runtime, buffer);
auto HEAPF64 = JSTypedArray<double>::create(runtime, buffer);
```

The `JSArrayBuffer` provides GC integration
(`creditExternalMemory`/`debitExternalMemory`) and JS API compatibility
(`WebAssembly.Memory.buffer`). The typed array views are stored as `Variable`s
in the top-level scope (`topLevelVS_`), alongside the pre-created function
closures. The top-level function body creates the typed array views and stores
them via `StoreFrameInst`. Each Wasm function accesses them via
`LoadFrameInst` from the parent scope (using the same `GetParentScopeInst`
mechanism already used for loading closures at call sites).

#### 4.5.2 Memory Access Translation (asm.js Pattern)

Wasm memory access translates directly to typed array element access, exactly
as asm.js compilers do. Each Wasm load/store uses the appropriately-typed
heap view with a shifted index:

```
// i32.load offset=K  (byte address = base + K)
//   → HEAP32[(base + K) >> 2]
%addr = FAddInst %base, K
%idx = AsUint32Inst (URShift %addr, 2)
%value = GetByVal %HEAP32, %idx

// i32.load8_s offset=K
//   → HEAP8[base + K]
%addr = FAddInst %base, K
%value = GetByVal %HEAP8, %addr

// f64.load offset=K
//   → HEAPF64[(base + K) >> 3]
%addr = FAddInst %base, K
%idx = AsUint32Inst (URShift %addr, 3)
%value = GetByVal %HEAPF64, %idx
```

This uses entirely existing IR instructions and bytecodes (`GetByVal`,
`PutByVal`, bitwise shifts). No helper functions or new bytecodes are needed
for memory access in Phase 1.

**Bounds checking and traps**: The Wasm spec requires trapping on out-of-bounds
access, unlike asm.js which silently returns `undefined` for OOB reads and
ignores OOB writes.

**Phase 1 approach**: Rather than emitting an explicit pre-access bounds check
(computing `addr + accessSize > memorySize`), Phase 1 leverages the typed
array's built-in bounds checking:

- **Loads**: Perform the `GetByVal` unconditionally. If the index is out of
  bounds, the typed array returns `undefined`. Compare the result to
  `undefined` and trap if equal. This is a single comparison after the access,
  which is more efficient than a pre-access range computation:

```
%idx = URShift %addr, 2
%val = GetByVal %HEAP32, %idx
%oob = BinaryStrictlyEqualInst %val, undefined
CondBranchInst %oob, BB_trap, BB_continue

BB_trap:
  CallInst @wasm_trap_oob
  UnreachableInst

BB_continue:
  ; use %val
```

- **Stores**: `PutByVal` to an OOB index is silently ignored by the typed
  array -- there is no sentinel to detect the failure. Phase 1 accepts this
  as a known spec deviation: OOB stores silently fail instead of trapping.
  Real-world Wasm modules never rely on OOB stores trapping for correctness
  (it is always a bug in the module). Phase 2's interpreter-level bounds
  check will trap correctly for both loads and stores.

This approach avoids the overhead of pre-access bounds checking entirely,
matching asm.js performance characteristics for the common (in-bounds) case.

**Unaligned access**: The Wasm spec requires that all loads and stores work
correctly regardless of the alignment of the effective address. The alignment
value encoded in each load/store instruction is a *pessimizing* hint: it only
works in the "negative" direction. `align=1` means the address may have any
alignment; `align=4` means the producer *believes* the address will be 4-byte
aligned, but the engine must still produce the correct result if it is not.
The spec implicitly assumes hardware that supports unaligned memory access
transparently (x86, ARM with unaligned access support, etc.).

This is a significant problem for the typed array approach. `HEAP32[addr >> 2]`
only produces the correct result when `addr` is 4-byte aligned. If `addr` is
misaligned, the right-shift rounds down to the wrong element. The typed array
approach **cannot** handle arbitrary unaligned access correctly.

**Phase 1 (mostly correct)**: Phase 1 uses the asm.js typed array pattern as
described above, which assumes naturally aligned access. When the alignment
annotation indicates non-natural alignment (e.g., `i32.load align=1`), Phase 1
falls back to byte-level assembly from `HEAPU8`:

```
// i32.load align=1 at byte address addr:
%b0 = GetByVal %HEAPU8, %addr
%b1 = GetByVal %HEAPU8, (%addr + 1)
%b2 = GetByVal %HEAPU8, (%addr + 2)
%b3 = GetByVal %HEAPU8, (%addr + 3)
%val = %b0 | (%b1 << 8) | (%b2 << 16) | (%b3 << 24)
```

This handles the common case correctly: loads/stores annotated with natural
alignment use the fast typed array path, and those explicitly annotated with
non-natural alignment use the correct byte assembly path. The only spec
violation is the (unlikely) case where a naturally-annotated load/store
receives a misaligned address at runtime — the spec says this must still work,
but Phase 1 will produce an incorrect result. In practice, compilers like LLVM
emit correct alignment annotations, so this does not affect real-world modules.

**Phase 2 (fully spec-compliant)**: New `WasmLoad32`/`WasmStore32` bytecodes
access the linear memory backing buffer directly via raw pointer arithmetic,
bypassing typed arrays entirely. Raw pointer access at an arbitrary byte offset
is naturally correct for any alignment (on architectures that support unaligned
access, which includes all Hermes targets: x86, ARM64, ARM32 with unaligned
access enabled). These bytecodes inline an explicit bounds check (a simple
integer comparison of `addr + size` against `memorySize`) in the interpreter
dispatch loop, then perform the raw pointer access. This traps correctly for
both loads and stores, resolving the Phase 1 limitation for OOB stores. It also
avoids typed array object overhead and index shifting. The alignment hint can
optionally be used on architectures where aligned access is faster, but
correctness does not depend on it.

Note: Guard pages (reserving 4 GiB + guard region, catching SIGSEGV) are
**not** used because Hermes is a library embedded in larger applications.
Hijacking signal handlers is too risky in that context.

#### 4.5.3 memory.grow

`memory.grow` extends the memory by N pages (64 KiB each):

1. Calculate new size: `newSize = currentSize + delta * 65536`.
2. Check against maximum (if declared).
3. `realloc` or `mremap` the backing buffer.
4. Update the cached `memoryPtr` and `memorySize`.
5. Detach the old `JSArrayBuffer` and create a new one (per JS API semantics).
6. **Re-create all typed array views** (`HEAP8`, `HEAP32`, `HEAPF64`, etc.)
   over the new buffer and **update the corresponding `Variable`s in
   `topLevelVS_`** via `StoreFrameInst`. The old views reference the detached
   buffer and are no longer usable. Since every memory access loads the view
   via `LoadFrameInst` from the top-level scope, subsequent accesses
   automatically see the new views after `memory.grow` updates the scope.
7. Return previous size in pages, or -1 on failure.

### 4.6 Tables

Wasm tables are arrays of typed references (typically `funcref`) used for
indirect function calls (`call_indirect`).

#### 4.6.1 Representation

A Wasm table is represented as a GC-managed array of `HermesValue`:

```cpp
struct WasmTable {
  /// Element type (funcref or externref).
  WasmValType elemType;
  /// Current elements.
  std::vector<HermesValue> elements;  // GC-rooted
  /// Size limits.
  uint32_t minSize, maxSize;
};
```

For `funcref` elements, each entry holds either:
- `null` (uninitialized)
- A `NativeFunction` wrapping a Wasm function (for direct Hermes calls)
- An expected type index (for `call_indirect` type checking)

#### 4.6.2 call_indirect Translation

`call_indirect` is the most complex call instruction:

```
// Wasm: call_indirect (type $sig) (table 0)
%tableIdx = pop()    // i32 index from stack
%funcRef = wasm_table_get(%table, %tableIdx)  // bounds-checked
wasm_check_signature(%funcRef, $sig)           // type check, trap on mismatch
result = call %funcRef(args...)
```

In Phase 1, this entire sequence is a helper-function call. In Phase 2, a
specialized `WasmCallIndirect` bytecode can inline the common fast path.

### 4.7 Globals

Wasm globals are module-level typed variables (mutable or immutable).

#### 4.7.1 Representation

Wasm globals are stored in a per-instance array, separate from the Hermes
environment chain:

```cpp
struct WasmGlobals {
  /// Type and mutability for each global.
  std::vector<WasmGlobalType> types;
  /// Values (stored as HermesValue, or raw for Phase 2).
  std::vector<HermesValue> values;  // GC-rooted
};
```

`global.get` and `global.set` become loads/stores from this array, translated
to indexed access in the IR. In Phase 1, these are `CallInst`s to helper
functions. In Phase 2, they could use a dedicated bytecode or be
lowered to `LoadFromEnvironment`/`StoreToEnvironment` by placing them in a
Hermes environment.

### 4.8 Function Calls and Imports/Exports

In Phase 1, Wasm modules are compiled separately from JS. Each Wasm module
produces its own set of Hermes IR functions and bytecodes. Imports and exports
are resolved at runtime during `WebAssembly.instantiate()`, with trampolines
handling the JS↔Wasm boundary.

#### 4.8.1 Wasm → Wasm Calls (Internal)

Direct calls between Wasm functions within the same Wasm module translate
directly to Hermes `CallInst`, since all functions from one Wasm module are
compiled into the same Hermes IR `Module`. The optimizer can inline these.

#### 4.8.2 Wasm → JS Calls (Imports)

Imported functions are not known at Wasm compile time (they come from the JS
import object at instantiation). Each import slot gets a **trampoline** that
reads the actual JS function from the instance state and calls it:

1. Load the JS function handle from the import table (populated at
   instantiation).
2. Marshal Wasm arguments to JS: i32/f32/f64 → Number, i64 → BigInt.
3. Call the JS function via the standard Hermes calling convention.
4. Marshal the JS return value back to the expected Wasm type.
5. If the JS function throws, the exception propagates naturally through
   Hermes's exception mechanism.

#### 4.8.3 JS → Wasm Calls (Exports)

Wasm exports are exposed as JS functions (via `instance.exports`):

1. Validate argument count and types.
2. Marshal JS arguments to Wasm types (Number → i32/f32/f64, BigInt → i64).
3. Call the compiled Wasm function (which is a normal Hermes bytecode function).
4. Marshal the Wasm return value(s) to JS.

Implementation: Each exported Wasm function is wrapped in a
`FinalizableNativeFunction` that performs marshaling.

For multi-value returns (Wasm functions returning multiple values), the
wrapper returns a JS array.

#### 4.8.4 Function Type Checking

Wasm `call_indirect` requires runtime type checking. Each Wasm function type
signature is assigned a unique ID. The runtime checks that the actual function's
type ID matches the expected type ID at each indirect call site.

#### 4.8.5 Instantiation Process

When `WebAssembly.instantiate()` is called, the following steps occur:

1. **Validate imports**: Check that all declared imports are provided and that
   their types match (functions have matching signatures, memories/tables/globals
   have compatible types).
2. **Allocate memories**: Create `JSArrayBuffer` + typed array views for each
   declared memory. For imported memories, use the provided memory object.
3. **Allocate tables**: Create table storage for each declared table. For
   imported tables, use the provided table object.
4. **Initialize globals**: Evaluate each global's init expression (which may
   reference imported globals or `ref.null`/`ref.func`) and store the result.
   For imported globals, use the provided value.
5. **Apply element segments**: For each active element segment, evaluate the
   offset expression and copy function references into the target table. Trap
   if any segment's range is out of bounds.
6. **Apply data segments**: For each active data segment, evaluate the offset
   expression and copy bytes into the target memory. Trap if any segment's
   range is out of bounds.
7. **Execute start function**: If the module declares a start function, call it.
   The start function takes no arguments and returns no results. It runs after
   all initialization is complete. If the start function traps, instantiation
   fails with a `WebAssembly.RuntimeError`.
8. **Build exports object**: Create the `instance.exports` object with wrapped
   functions, memory/table/global accessors.

Steps 5-6 must check bounds before copying: if `offset + data.length >
memory.size` (or table size), the entire instantiation fails with a
`LinkError` and no partial initialization occurs.

**IMPORTANT — AOT compatibility requirement**: All instantiation logic,
including import validation (step 1), must be emitted as IR by WasmIRGen into
the compiled top-level function. It must NOT be implemented as a separate
runtime step outside the compiled bytecode. This is because the same compiled
bytecode runs in both the JS API path (`new WebAssembly.Instance()`) and the
AOT path (`hermesc --wasm foo.wasm -o foo.hbc`). In the AOT path, the `.hbc`
file is a self-contained program — there is no `WebAssembly.Instance`
constructor or `WasmModuleData` at runtime. The compiled top-level function
reads imports from `globalThis.__wasm_imports__`, performs all initialization,
and returns the exports object. If import validation were implemented outside
the bytecode (e.g., in `instantiateModuleImpl` using metadata from
`WasmModuleData`), it would only work for the JS API path and would be
silently skipped in the AOT path. Emitting validation as IR ensures it works
identically in both paths, requires no HBC format extensions, and keeps the
compiled module self-contained.

#### 4.8.6 Future: Joint JS+Wasm Compilation

A significant performance opportunity exists in compiling JS and Wasm sources
together in a single `hermesc` invocation (e.g.,
`hermesc app.js module.wasm -o bundle.hbc`). In this mode, both JS and Wasm
functions would be compiled into the same Hermes IR `Module`, enabling:

- **Static import resolution**: When the compiler can determine which JS
  functions flow into the Wasm import object (common in generated glue code
  from Emscripten/wasm-bindgen), Wasm import calls become direct `CallInst`s
  to the JS functions, eliminating trampoline overhead entirely.
- **Cross-language inlining**: Small JS imports can be inlined into Wasm
  callers and vice versa, removing call overhead and enabling further
  optimization across the boundary.
- **Type specialization**: The compiler knows that a JS function called from
  Wasm will always receive specific types (e.g., Number), so it can skip
  type checks and coercions in the JS function body.
- **Single bytecode file**: Both JS and Wasm bytecodes are serialized into
  one `.hbc` file, simplifying deployment.

This requires the build tool to pass both sources to `hermesc` and either a
convention or static analysis to identify the import bindings. For the common
pattern where the JS glue code is generated (Emscripten, wasm-pack), this is
straightforward — the glue code has a predictable structure where imports are
statically resolvable.

### 4.9 Wasm Traps

Wasm traps are unrecoverable errors within Wasm execution. They occur on:

- Out-of-bounds memory access
- Out-of-bounds table access
- Integer divide by zero
- Integer overflow (`INT_MIN / -1`)
- Invalid `call_indirect` type
- `unreachable` instruction
- Stack overflow
- Out-of-range float-to-int truncation (non-saturating)

**Implementation**: Traps are translated to `runtime.raiseError()` calls
(producing a `WebAssembly.RuntimeError`). In the IR, trap checks are explicit:

```
%divisor = ...
%isZero = FCompareInst EQ, %divisor, 0
CondBranchInst %isZero, BB_trap, BB_continue

BB_trap:
  CallInst @wasm_trap("integer divide by zero")  // noreturn helper
  UnreachableInst

BB_continue:
  %result = ... // division
```

For Phase 2, the interpreter can handle some traps inline (e.g., bounds
checking via an integer comparison in the interpreter dispatch loop), but the
IR representation remains explicit for correctness.

**Interaction with Wasm exception handling**: Traps throw
`WebAssembly.RuntimeError`, which is a JS exception. However, per the Wasm spec,
traps must **not** be catchable by Wasm `try`/`catch`/`catch_all` — they bypass
all Wasm exception handlers and unwind the entire Wasm call stack. Only JS
`try`/`catch` can catch traps. Since traps are implemented as JS exceptions
(via `runtime.raiseError()`), Wasm `catch` and `catch_all` handlers must check
whether the caught value is a `WebAssembly.RuntimeError` and re-throw it if so,
before processing it as a Wasm exception. See §4.10 for details.

### 4.10 Wasm Exception Handling

Wasm exception handling (`try`/`catch`/`throw`/`rethrow`) maps directly to
Hermes's existing exception infrastructure:

| Wasm construct | Hermes IR |
|---------------|-----------|
| `try` | `TryStartInst` |
| `catch $tag` | `CatchInst` + tag check |
| `catch_all` | `CatchInst` (unconditional) |
| `throw $tag` | `ThrowInst` |
| `rethrow` | `ThrowInst` (re-throw caught value) |

Wasm exceptions are **tagged**: each `throw` carries a tag index (identifying
the exception type) and zero or more typed payload values. JS exceptions are
untyped (any value). The bridge is straightforward:

- **Wasm `throw`**: Create a `WebAssembly.Exception` JS object containing the
  tag and payload values, then `ThrowInst` it.
- **Wasm `catch $tag`**: `CatchInst` the JS value, check if it's a
  `WebAssembly.Exception` with the matching tag. If yes, extract the payload
  values. If no, re-throw (it's either a different Wasm exception or a JS
  exception propagating through Wasm).
- **Wasm `catch_all`**: `CatchInst` unconditionally, then check if the caught
  value is a `WebAssembly.RuntimeError` (i.e., a trap). If it is a trap,
  re-throw it immediately — traps must not be caught by Wasm exception handlers
  (per spec, traps bypass all Wasm `try`/`catch` blocks). If it is not a trap,
  handle it normally (it is either a Wasm exception or a JS exception).
- **JS exceptions propagating through Wasm**: Hermes's exception mechanism
  already propagates through any call stack. A JS exception thrown by an
  imported function will naturally unwind through Wasm frames unless caught by
  `catch_all`. Note that `catch $tag` also implicitly re-throws non-matching
  exceptions, so JS exceptions that don't match any tag will propagate
  correctly.

The bytecodes `Throw`, `Catch`, and the exception handler tables in the
bytecode file format already support this. No new bytecodes are needed.

### 4.11 Optimization

The existing Hermes optimizer passes can be reused, with varying effectiveness:

| Pass | Applicability to Wasm |
|------|----------------------|
| **Mem2Reg** | Essential — promotes Wasm locals from stack to SSA |
| **SimplifyCFG** | Useful — cleans up control flow after translation |
| **CSE** | Useful — eliminates common subexpressions |
| **Dead Code Elimination** | Useful |
| **InstSimplify** | Useful — constant folding, algebraic simplification |
| **Code Motion** | Useful — hoists loop-invariant bounds checks |
| **Inlining** | Useful — can inline small Wasm functions |
| **Type Inference** | Less useful — Wasm is already typed, but helps optimizer understand values are numbers |
| **Scope Hoisting** | Not applicable — Wasm has no closures |
| **LowerGeneratorFunction** | Not applicable |

**Wasm-specific optimizations** (new passes):

1. **Bounds Check Elimination**: If a memory access at offset `o` with size `s`
   is dominated by a successful access at offset `o'` where `o + s <= o' + s'`,
   the bounds check can be eliminated.

2. **i32 Wrapping Elimination**: If a chain of i32 operations feeds into another
   i32 operation that would truncate anyway (e.g., `i32.add` followed by
   `i32.and`), intermediate wrapping can be skipped.

3. **Constant Propagation for Globals**: Immutable Wasm globals can be inlined.

### 4.12 Bytecode Generation

Wasm-generated IR flows through the existing BCGen pipeline:

1. **Lowering passes** — `LowerCalls`, `LowerCondBranch`, `LoadConstants`, etc.
   apply as normal.
2. **Register allocation** — The existing `HVMRegisterAllocator` assigns Hermes
   virtual registers to IR values.
3. **Bytecode emission** — IR instructions are lowered to Hermes bytecodes.

For Phase 1, Wasm helper calls emit as `Call1`/`Call2`/`CallN` bytecodes. For
Phase 2, new Wasm-specific bytecodes are emitted directly.

**Register pressure**: Most Hermes bytecodes use `Reg8` (8-bit) register
encoding, limiting a function to 256 registers. Wasm functions can have many
locals, and with i64 values split into two registers (Phase 1), register
pressure can be high. Hermes has `Reg32` variants for some bytecodes when
`Reg8` is insufficient. For Wasm, the register allocator must fall back to
`Reg32` bytecode variants when a function exceeds 256 live values. This may
require verifying that all bytecodes used by Wasm-generated code have `Reg32`
variants, and adding any that are missing.

**Call frame overhead**: Every Wasm function call goes through the standard
Hermes calling convention (frame allocation, argument passing via
`NativeArgs`). This is heavier than a native function call. For deeply
recursive Wasm functions or tight call loops, this overhead is significant.
Phase 2 could explore a lighter calling convention for Wasm-to-Wasm calls
within the same module, but Phase 1 uses the standard mechanism for
simplicity.

## 5. JS API Implementation

**Location**: `lib/VM/JSLib/WebAssembly.cpp` (new) and `API/hermes/extensions/`

### 5.1 WebAssembly Namespace

Implement the `WebAssembly` global object with:

- `WebAssembly.validate(bytes)` → `bool`
- `WebAssembly.compile(bytes)` → `Promise<Module>` (or sync for Hermes AOT)
- `WebAssembly.instantiate(bytes, imports)` → `Promise<{module, instance}>`
- `WebAssembly.compileStreaming()` — Not applicable to Hermes (no fetch).

### 5.2 WebAssembly.Module

A `HostObject` wrapping a compiled `WasmModuleInfo` + compiled Hermes bytecode:

```cpp
class JSWebAssemblyModule : public JSObject {
  /// The parsed module info.
  std::shared_ptr<WasmModuleInfo> moduleInfo_;
  /// Compiled bytecode for all functions.
  std::shared_ptr<hbc::BytecodeModule> bytecodeModule_;
};
```

- `WebAssembly.Module.exports(module)` → list of exports
- `WebAssembly.Module.imports(module)` → list of imports

### 5.3 WebAssembly.Instance

A `HostObject` wrapping an instantiated module with resolved imports:

```cpp
class JSWebAssemblyInstance : public JSObject {
  /// Reference to the module.
  Handle<JSWebAssemblyModule> module_;
  /// Linear memories (typically one).
  std::vector<Handle<JSWebAssemblyMemory>> memories_;
  /// Tables.
  std::vector<Handle<JSWebAssemblyTable>> tables_;
  /// Global values.
  std::vector<HermesValue> globals_;
  /// Exported functions (wrapped as JS-callable).
  Handle<JSObject> exports_;
};
```

### 5.4 WebAssembly.Memory

```cpp
class JSWebAssemblyMemory : public JSObject {
  /// Backing ArrayBuffer.
  Handle<JSArrayBuffer> buffer_;
  /// Raw pointer for fast access from Wasm code.
  uint8_t *dataPtr_;
  uint32_t dataSize_;
  /// Limits.
  uint32_t initialPages_, maxPages_;
};
```

- `memory.buffer` → returns the ArrayBuffer
- `memory.grow(delta)` → grows memory, returns previous page count

### 5.5 WebAssembly.Table

```cpp
class JSWebAssemblyTable : public JSObject {
  WasmValType elemType_;
  std::vector<HermesValue> elements_;  // GC-rooted
  uint32_t minSize_, maxSize_;
};
```

### 5.6 WebAssembly.Global

```cpp
class JSWebAssemblyGlobal : public JSObject {
  WasmValType type_;
  bool mutable_;
  HermesValue value_;
};
```

### 5.7 Error Types

- `WebAssembly.CompileError` — extends `Error`
- `WebAssembly.LinkError` — extends `Error`
- `WebAssembly.RuntimeError` — extends `Error`

These can be implemented using the existing `NATIVE_ERROR_TYPE` macro pattern
in `lib/VM/JSLib/Error.cpp`.

## 6. Required IR and Bytecode Extensions

### 6.1 Phase 1: asm.js Pattern (Minimal Changes)

Phase 1 requires **no new IR instructions or bytecodes**. The key insight is
that Wasm is essentially a binary-encoded asm.js — the same patterns that make
asm.js run on existing JS engines apply here. Most Wasm operations map directly
to existing Hermes IR and bytecodes using standard JS/asm.js idioms.

**Operations that map directly to existing Hermes IR/bytecodes** (asm.js
pattern):

| Wasm instruction | asm.js / Hermes equivalent |
|-----------------|----------------------------|
| **i32 arithmetic** | |
| `i32.add` | `AddN` + `BitOr 0` (i.e., `(a + b) \| 0`) |
| `i32.sub` | `SubN` + `BitOr 0` |
| `i32.and/or/xor` | `BitAnd`/`BitOr`/`BitXor` (already int32) |
| `i32.shl` | `LShift` |
| `i32.shr_s` | `RShift` |
| `i32.shr_u` | `URshift` |
| **i32 comparisons** | |
| `i32.eq/ne` | `StrictEq`/`StrictNeq` |
| `i32.lt_s/le_s/gt_s/ge_s` | `Less`/`LessEq`/`Greater`/`GreaterEq` |
| `i32.lt_u/le_u/gt_u/ge_u` | `AsUint32Inst` both operands + `Less`/etc. |
| `i32.eqz` | `StrictEq` with 0 |
| **f64 arithmetic** | |
| `f64.add/sub/mul/div` | `AddN`/`SubN`/`MulN`/`DivN` |
| `f64.neg` | `Negate` |
| `f64.eq/ne/lt/gt/le/ge` | Existing comparison bytecodes |
| **Memory access** | |
| `i32.load` | bounds check + `GetByVal` on `Int32Array` view |
| `i32.load8_s/8_u/16_s/16_u` | bounds check + `GetByVal` on `Int8Array`/`Uint8Array`/`Int16Array`/`Uint16Array` |
| `f32.load` / `f64.load` | bounds check + `GetByVal` on `Float32Array`/`Float64Array` |
| `*.store` variants | bounds check + `PutByVal` on appropriate typed array view |
| **Control flow** | |
| `block/loop/if/else` | `BasicBlock` + `BranchInst`/`CondBranchInst` + `PhiInst` |
| `br/br_if` | `BranchInst`/`CondBranchInst` |
| `br_table` | `SwitchInst` |
| `call` | `CallInst` |
| `return` | `ReturnInst` |
| **Other** | |
| `drop` | (no-op; value not used) |
| `select` | `CondBranchInst` + `PhiInst` (see note below) |
| `nop` | (omitted) |
| `local.get/set/tee` | `LoadStackInst`/`StoreStackInst` (promoted by Mem2Reg) |

**Note on `select`**: The `CondBranchInst` + `PhiInst` translation creates two
extra basic blocks per `select`, which is heavyweight for what is essentially a
ternary operator (`cond ? a : b`). LLVM emits `select` frequently. A Phase 2
optimization could lower `select` to a `SelectInst` or conditional-move pattern
to avoid the basic block overhead.

**Operations that need helper-function calls** (no existing asm.js/JS
equivalent):

```
// i32 multiplication (double multiply loses precision for large int32s;
// asm.js uses Math.imul for this):
wasm_i32_mul  → CallBuiltin Math.imul

// Trapping division (JS division returns Infinity, not trap):
wasm_i32_div_s, wasm_i32_div_u   (trap on /0 or INT_MIN/-1)
wasm_i32_rem_s, wasm_i32_rem_u   (trap on /0)

// Bit manipulation (no JS operator equivalents except Math.clz32):
wasm_i32_clz    → CallBuiltin Math.clz32
wasm_i32_ctz, wasm_i32_popcnt
wasm_i32_rotl, wasm_i32_rotr
wasm_i32_extend8_s, wasm_i32_extend16_s

// f32 precision (asm.js uses Math.fround):
wasm_f32_fround → CallBuiltin Math.fround (applied after each f32 op)

// f32/f64 special operations (no direct JS operator):
wasm_f{32,64}_{ceil,floor,trunc,nearest}  → Math.ceil/floor/trunc/round
wasm_f{32,64}_{sqrt,min,max}              → Math.sqrt/min/max
wasm_f{32,64}_copysign
wasm_f{32,64}_abs                         → Math.abs

// Trapping float-to-int conversions:
wasm_i32_trunc_f{32,64}_{s,u}   (trap on NaN or out-of-range)
wasm_i32_trunc_sat_f{32,64}_{s,u}  (saturating, no trap)

// Reinterpret (bit-cast between int and float):
wasm_i32_reinterpret_f32, wasm_f32_reinterpret_i32

// i64 operations (all, via split i32 pairs):
wasm_i64_*

// Wasm trap (raises WebAssembly.RuntimeError):
wasm_trap

// memory.grow, memory.size, memory.fill, memory.copy, memory.init
// (these are complex runtime operations, not simple typed array access)
```

Note that many of the "helper" calls above actually map to existing JS builtins
(`Math.imul`, `Math.clz32`, `Math.fround`, `Math.sqrt`, `Math.ceil`, etc.).
These can be emitted as `CallBuiltin` instructions in the IR, which Hermes
already optimizes. Only the truly novel operations (trapping division, rotate,
ctz, popcnt, reinterpret, i64 arithmetic) require new native helper functions.

### 6.2 Phase 2: New IR Instructions and Bytecodes

For performance, add specialized bytecodes for operations that are bottlenecked
by helper-function call overhead or typed-array access overhead.

#### 6.2.1 New Bytecodes

```
// Memory access (inline bounds check + raw pointer access, bypassing
// typed array object overhead):
WasmLoad32     Reg8, Reg8, UInt32   // dest, base, offset
WasmLoad64     Reg8, Reg8, UInt32
WasmLoad8S     Reg8, Reg8, UInt32
WasmLoad8U     Reg8, Reg8, UInt32
WasmLoad16S    Reg8, Reg8, UInt32
WasmLoad16U    Reg8, Reg8, UInt32
WasmStore32    Reg8, Reg8, UInt32   // value, base, offset
WasmStore64    Reg8, Reg8, UInt32
WasmStore8     Reg8, Reg8, UInt32
WasmStore16    Reg8, Reg8, UInt32

// i32 trapping arithmetic (avoids call overhead for the most common trap
// checks — div by zero and overflow):
WasmI32DivS    Reg8, Reg8, Reg8
WasmI32DivU    Reg8, Reg8, Reg8
WasmI32RemS    Reg8, Reg8, Reg8
WasmI32RemU    Reg8, Reg8, Reg8

// i32 bit manipulation (no JS equivalent at all):
WasmI32Ctz     Reg8, Reg8
WasmI32Popcnt  Reg8, Reg8
WasmI32Rotl    Reg8, Reg8, Reg8
WasmI32Rotr    Reg8, Reg8, Reg8

// f32 precision:
FRound         Reg8, Reg8           // round to f32 precision

// Trap:
WasmTrap       UInt32               // trap with message string index
```

#### 6.2.2 Phase 3: i64 Support via GC-Excluded Register Slots

Partition each function's register frame into a GC-scanned region and a
GC-excluded region. The GC-excluded slots hold raw 64-bit integers and are
invisible to the garbage collector (they must never contain pointers to the GC
heap).

This requires:
- The register allocator must track which registers are GC-excluded.
- The bytecode frame layout must encode the boundary between GC-scanned and
  GC-excluded regions so the GC knows where to stop scanning.
- New bytecodes for i64 operations that read/write GC-excluded slots directly.

New i64 bytecodes:
```
WasmI64Add, WasmI64Sub, WasmI64Mul, WasmI64DivS, WasmI64DivU, ...
WasmI64And, WasmI64Or, WasmI64Xor, WasmI64Shl, WasmI64ShrS, WasmI64ShrU
WasmI64Eq, WasmI64Ne, WasmI64LtS, WasmI64LtU, ...
WasmI64Const    Reg8, Imm64         // new immediate type needed
WasmI64Load     Reg8, Reg8, UInt32
WasmI64Store    Reg8, Reg8, UInt32
```

## 7. Performance Considerations

### 7.1 Expected Performance Characteristics

| Phase | Relative to native Wasm engine | Bottleneck |
|-------|-------------------------------|------------|
| Phase 1 | ~20-100x slower | Helper-function call overhead for every Wasm operation; i64 split into two registers |
| Phase 2 | ~5-20x slower | Still using NaN-boxed doubles for i32; bounds checks not elided; no JIT |
| Phase 3 | ~2-10x slower | Native i64 in HermesValue; most operations inline in interpreter; no JIT |
| Phase 4 (JIT) | ~1-3x slower | JIT compilation of Wasm bytecodes to native code |

### 7.2 Key Performance Opportunities

1. **Bounds check elimination**: The most impactful optimization. Many memory
   accesses in a loop can share a single bounds check. The optimizer can hoist
   checks out of loops or prove them redundant.

2. **i32 wrapping elimination**: Many i32 operation sequences can defer wrapping
   to the final consumer (since intermediate overflow doesn't affect the wrapped
   result for add/sub/mul).

3. **Inline memory access in interpreter**: Phase 2's `WasmLoad32`/`WasmStore32`
   bytecodes let the interpreter perform the bounds check (a simple integer
   comparison) + raw pointer load/store in a single dispatch, avoiding
   function-call overhead and typed array indirection.

4. **JIT compilation**: Hermes already has a JIT framework (`HERMESVM_ALLOW_JIT`).
   Wasm functions are ideal JIT candidates because they are statically typed and
   have no dynamic dispatch overhead. A Wasm-to-native JIT tier would provide
   near-native performance.

### 7.3 Memory Overhead

Each Wasm instance adds:
- Linear memory: `initialPages * 64 KiB` (typically 1-256 MiB for real apps)
- Table: `numElements * 8 bytes` (typically small)
- Globals: `numGlobals * 8 bytes` (typically small)
- Compiled bytecode: roughly proportional to Wasm code size

## 8. Implementation Phases

### Phase 1: Correct MVP (estimated scope: large)

**Goal**: Run any valid Wasm module correctly, with all JS API objects. Covers
the MVP plus widely-supported features (exception handling).

**Components**:
1. Wasm binary reader via wabt `BinaryReaderDelegate` (with
   `BinaryReaderHermesIRGen` adapter)
2. WasmModuleInfo data structures
3. WasmIRGen (stack-to-SSA translation, all control flow, locals)
4. Helper-function calls for operations without JS equivalents (trapping
   division, ctz/popcnt/rotate, reinterpret casts); `CallBuiltin` for
   `Math.imul`, `Math.clz32`, `Math.fround`, `Math.sqrt`, etc.
5. Linear memory via JSArrayBuffer + typed array views (asm.js pattern)
6. Table support
7. Global support
8. Import/export trampolines
9. i64 via split i32 pairs
10. Exception handling via existing TryStart/Catch/Throw IR + WebAssembly.Exception
11. WebAssembly JS API (Module, Instance, Memory, Table, Global, Exception, errors)

**Reused from existing Hermes**:
- IR infrastructure (Module, Function, BasicBlock, Instruction, Value)
- Optimizer (Mem2Reg, SimplifyCFG, CSE, DCE, InstSimplify)
- Register allocator
- BCGen (lowering, bytecode emission)
- Bytecode file format and serialization
- Interpreter (for executing the generated bytecodes)
- GC (for managing JS API wrapper objects and ArrayBuffers)

**Limitations**: Phase 1 supports only single-value function returns and
single-value block types (i.e., each `block`, `if`, and `loop` produces at most
one result value). Multi-value returns (functions or blocks producing multiple
values) are deferred to Phase 4. The Hermes calling convention returns a single
`HermesValue`; multi-value support will require returning values via a side
channel (e.g., writing extra return values to the caller's stack frame or a
shared buffer).

**Testing**: Run the Wasm spec test suite (available as `.wast` files).

### Phase 2: i32/f32 Performance (estimated scope: medium)

**Goal**: Eliminate helper-call overhead for the most common operations.

**Components**:
1. New IR instructions for i32 arithmetic, comparisons, memory access
2. New bytecodes (~20) for inline i32/memory operations
3. Interpreter handlers for new bytecodes
4. FRound instruction/bytecode for f32
5. Bounds check elimination optimization pass

### Phase 3: i64 Performance (estimated scope: medium)

**Goal**: Native i64 support via GC-excluded register slots.

**Components**:
1. Partitioned register frame (GC-scanned + GC-excluded regions)
2. Register allocator support for GC-excluded slots
3. New IR instructions and bytecodes for i64 operations (~20)
4. i64 ↔ BigInt marshaling at JS-Wasm boundary

### Phase 4: Wasm 2.0+ Features (estimated scope: large)

**Goal**: Support widely-deployed post-MVP features.

**Components**:
1. Multi-value returns
2. Bulk memory operations (memory.fill/copy/init, data.drop)
3. Reference types (externref, funcref, table instructions)
4. Tail calls (return_call, return_call_indirect)

### Phase 5: SIMD (estimated scope: very large)

**Goal**: Support v128 and ~236 SIMD instructions.

**Components**:
1. v128 type representation (likely requires two HermesValue registers or a new
   128-bit cell type)
2. ~236 new bytecodes for SIMD operations
3. Platform-specific interpreter optimization (SSE/NEON intrinsics)

### Phase 6: JIT Integration (estimated scope: very large)

**Goal**: JIT-compile Wasm functions to native code for near-native performance.

**Components**:
1. Type-specialized code generation from Wasm-annotated IR
2. Direct register allocation (no NaN-boxing in JIT code)
3. Inline bounds checks with guard-page fallback
4. Tier-up heuristics (interpreter → JIT)

## 9. Open Questions

1. **Ahead-of-time only or runtime compilation too?** Hermes's key design
   principle is AOT compilation. Should Wasm modules only be compilable at build
   time (via `hermesc`), or should runtime `WebAssembly.compile()` be supported?
   Runtime compilation is needed for full API compatibility but conflicts with
   Hermes's no-JIT-at-runtime philosophy. A possible middle ground: allow
   runtime Wasm compilation but compile to bytecode (not native), consistent with
   how Hermes handles `eval()` in some configurations.

2. **Bytecode file format integration**: Should Wasm modules be embedded in the
   same `.hbc` bytecode file as the JS code that uses them, or in separate
   files? Embedding enables single-file deployment but complicates the file
   format. Separate files are simpler but require runtime linking.

3. **i64 representation priority**: The split-i32 approach (Phase 1) is
   significantly slower for i64-heavy workloads. If target Wasm modules use i64
   heavily, Phase 3 should be prioritized or the split-i32 approach should be
   reconsidered in favor of GC-excluded register slots from the start.

4. **Bounds checking strategy**: Phase 1 uses post-access undefined comparison
   for loads and accepts silent failure for OOB stores. Phase 2 uses explicit
   integer comparison in the interpreter. Guard pages (signal-based) are ruled
   out because Hermes is a library embedded in larger applications where
   hijacking signal handlers is too risky.

5. **Interaction with HV32 mode**: The GC-excluded register slot approach
   works regardless of HermesValue encoding mode (HV64 or HV32), since the
   GC-excluded region is not subject to HermesValue tagging constraints.
   However, the register frame layout and GC scanning boundary must account
   for the different slot sizes in each mode.

6. **Joint Wasm/JS compilation priority**: Section 4.8.5 describes the
   potential for compiling JS and Wasm together. How early should this be
   prioritized? It offers large performance wins (eliminating trampolines,
   enabling cross-language inlining) but requires build-tool integration and
   static analysis of import bindings.

7. **React Native integration**: What's the expected use case? Are Wasm modules
   bundled with the app (AOT compiled) or loaded at runtime? This affects the
   API surface and compilation strategy.

8. **Custom decoder timing**: The design starts with wabt's binary reader for
   correctness and speed of implementation. When should a custom decoder replace
   it? Likely when profiling shows the callback-per-instruction overhead is
   significant relative to IR generation, or when the wabt dependency becomes a
   maintenance burden. All major engines (V8, SpiderMonkey, JSC) use custom
   decoders, but they also need streaming compilation — a requirement Hermes's
   AOT model doesn't share.

## Appendix A: Wasm Instruction Set Summary

For reference, the complete Wasm MVP instruction set that must be supported:

### Control Flow (12 instructions)
`unreachable`, `nop`, `block`, `loop`, `if`, `else`, `end`, `br`, `br_if`,
`br_table`, `return`, `call`, `call_indirect`

### Parametric (2 instructions)
`drop`, `select`

### Variable (5 instructions)
`local.get`, `local.set`, `local.tee`, `global.get`, `global.set`

### Memory (25 instructions)
`i32.load`, `i64.load`, `f32.load`, `f64.load`,
`i32.load8_s`, `i32.load8_u`, `i32.load16_s`, `i32.load16_u`,
`i64.load8_s`, `i64.load8_u`, `i64.load16_s`, `i64.load16_u`,
`i64.load32_s`, `i64.load32_u`,
`i32.store`, `i64.store`, `f32.store`, `f64.store`,
`i32.store8`, `i32.store16`,
`i64.store8`, `i64.store16`, `i64.store32`,
`memory.size`, `memory.grow`

### i32 Numeric (30 instructions)
`i32.const`, `i32.eqz`,
`i32.eq`, `i32.ne`, `i32.lt_s`, `i32.lt_u`, `i32.gt_s`, `i32.gt_u`,
`i32.le_s`, `i32.le_u`, `i32.ge_s`, `i32.ge_u`,
`i32.clz`, `i32.ctz`, `i32.popcnt`,
`i32.add`, `i32.sub`, `i32.mul`, `i32.div_s`, `i32.div_u`,
`i32.rem_s`, `i32.rem_u`,
`i32.and`, `i32.or`, `i32.xor`, `i32.shl`, `i32.shr_s`, `i32.shr_u`,
`i32.rotl`, `i32.rotr`

### i64 Numeric (30 instructions)
Same set as i32 (with `i64.` prefix): `i64.const`, `i64.eqz`, `i64.eq`, etc.

### f32 Numeric (21 instructions)
`f32.const`,
`f32.eq`, `f32.ne`, `f32.lt`, `f32.gt`, `f32.le`, `f32.ge`,
`f32.abs`, `f32.neg`, `f32.ceil`, `f32.floor`, `f32.trunc`,
`f32.nearest`, `f32.sqrt`,
`f32.add`, `f32.sub`, `f32.mul`, `f32.div`, `f32.min`, `f32.max`,
`f32.copysign`

### f64 Numeric (21 instructions)
Same set as f32 (with `f64.` prefix).

### Conversions (25 instructions)
`i32.wrap_i64`,
`i32.trunc_f32_s`, `i32.trunc_f32_u`, `i32.trunc_f64_s`, `i32.trunc_f64_u`,
`i64.extend_i32_s`, `i64.extend_i32_u`,
`i64.trunc_f32_s`, `i64.trunc_f32_u`, `i64.trunc_f64_s`, `i64.trunc_f64_u`,
`f32.convert_i32_s`, `f32.convert_i32_u`,
`f32.convert_i64_s`, `f32.convert_i64_u`,
`f32.demote_f64`,
`f64.convert_i32_s`, `f64.convert_i32_u`,
`f64.convert_i64_s`, `f64.convert_i64_u`,
`f64.promote_f32`,
`i32.reinterpret_f32`, `i64.reinterpret_f64`,
`f32.reinterpret_i32`, `f64.reinterpret_i64`

### Non-trapping Float-to-Int Conversions (8 instructions)
`i32.trunc_sat_f32_s`, `i32.trunc_sat_f32_u`,
`i32.trunc_sat_f64_s`, `i32.trunc_sat_f64_u`,
`i64.trunc_sat_f32_s`, `i64.trunc_sat_f32_u`,
`i64.trunc_sat_f64_s`, `i64.trunc_sat_f64_u`

### Sign-Extension Operators (5 instructions)
`i32.extend8_s`, `i32.extend16_s`,
`i64.extend8_s`, `i64.extend16_s`, `i64.extend32_s`

**Total MVP + widely-adopted proposals**: ~185 instructions.

## Appendix B: File Organization

```
hermes/
├── external/
│   └── wabt/                          # Vendored wabt (binary reader subset)
├── include/hermes/
│   ├── WasmFrontend/
│   │   ├── WasmModuleInfo.h           # Module data structures
│   │   ├── WasmTypes.h                # ValType, FuncType, etc.
│   │   └── WasmCompile.h             # Top-level compileWasmModule() API
│   └── WasmIRGen/
│       └── WasmIRGen.h                # Wasm → Hermes IR translator
├── lib/
│   ├── WasmFrontend/
│   │   ├── BinaryReaderHermesIRGen.h  # wabt delegate → WasmIRGen adapter
│   │   ├── BinaryReaderHermesIRGen.cpp
│   │   ├── WasmCompile.cpp           # Entry point: bytes → Hermes bytecode
│   │   └── CMakeLists.txt
│   ├── WasmIRGen/
│   │   ├── WasmIRGen.cpp             # Stack-to-SSA translation
│   │   ├── WasmHelpers.cpp           # Phase 1 helper functions
│   │   └── CMakeLists.txt
│   └── VM/
│       └── JSLib/
│           ├── WebAssembly.cpp        # JS API implementation
│           ├── WebAssemblyModule.cpp
│           ├── WebAssemblyInstance.cpp
│           ├── WebAssemblyMemory.cpp
│           ├── WebAssemblyTable.cpp
│           └── WebAssemblyGlobal.cpp
├── test/
│   └── wasm/                          # Lit-based Wasm tests
│       ├── basic-arithmetic.js
│       ├── memory.js
│       ├── control-flow.js
│       └── imports-exports.js
└── unittests/
    ├── WasmFrontend/                  # Unit tests for reader integration
    │   └── WasmFrontendTest.cpp
    └── WasmIRGen/                     # Unit tests for IR generation
        └── WasmIRGenTest.cpp
```
