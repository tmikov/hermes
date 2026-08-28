# Wasm Implementation Memory

## Wasm Implementation
- Plan at `doc/WasmImplementationPlan.md` (Phase 1 - Correct MVP)
- Design doc at `doc/WasmSupport.md`
- Progress tracked in `.ralph/progress.md`
- New dirs: lib/WasmFrontend/, lib/WasmIRGen/, lib/VM/JSLib/WebAssembly/
- Uses wabt BinaryReaderDelegate for parsing
- i64 via split i32 pairs (Phase 1); Phase 3 uses GC-excluded register slots
- Memory via JSArrayBuffer + typed array views (asm.js pattern). Views stored as Variables in `topLevelVS_` (same scope as closures), accessed via `LoadFrameInst`. `memory.grow` updates the scope variables so subsequent accesses see new views automatically.
- Bounds checking: loads compare result to `undefined` (post-access); OOB stores silently fail (Phase 1 spec deviation). No pre-access range computation. No guard pages (Hermes is an embedded library, can't hijack signals). Phase 2 uses explicit integer comparison in interpreter.
- Phase 1 alignment: trust annotation at compile time (known spec limitation for misaligned + natural annotation; fixed in Phase 2 with raw pointer bytecodes)
- Gated behind `HERMES_ENABLE_WASM` CMake option (default OFF)
- wabt library target is `wabt`. Link via `LINK_LIBS wabt` (not LINK_OBJLIBS)
- Include wabt headers with `#include "wabt/binary-reader.h"`
- Must build with Clang (`-DCMAKE_C_COMPILER=clang-17 -DCMAKE_CXX_COMPILER=clang++-17`)

## CompilerDriver Integration
- `HERMES_ENABLE_WASM` is both a CMake variable and a C++ define (via `add_definitions`)
- CompilerDriver uses `#ifdef HERMES_ENABLE_WASM` to guard wasm-specific code
- `hermescompiler` static library conditionally includes wasm object libs + wabt
- Entry point: `compileWasmModule(buffer, size, Module&, errorMsg)` in `WasmCompile.h`
- Context default constructor: `Context()` — don't pass `CodeGenerationSettings` by value (needs rvalue ref `&&`)
- `-wasm` flag and `.wasm` extension auto-detection in CompilerDriver

## Lit Test Infrastructure
- wabt tools at `${CMAKE_BINARY_DIR}/external/wabt/wabt/{tool}` (not in `HERMES_TOOLS_OUTPUT_DIR`)
- `%wat2wasm` and `%wast2json` substitutions available in lit tests
- Wasm lit tests require `REQUIRES: wasm` and use `.wat` suffix
- `wasm` feature is set when `HERMES_ENABLE_WASM=ON`
- Use single `RUN:` line with `&&` (not separate lines): `%wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s`
- Runtime test pattern: `%wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes %t.hbc | %FileCheck %s`
- `wat2wasm --debug-names` is required to emit the name section (not emitted by default)
- Both `hermescompiler` (for `hermesc`) and `hermesvm_a` (for `hermes`) must link wasm libs

## Lit Test Style (Tightened)
- Place CHECK lines **after** the WAT function they verify (source first, expected output second)
- Use `CHECK-NEXT` for consecutive instructions; `CHECK` only to skip boilerplate (e.g., AllocStack/LoadParam preamble)
- Capture register names and verify data flow: `%[[ADD:.*]] = BinaryAddInst (:any) %[[A]]: any, %[[B]]: any`
- Verify result chains: operation → phi → return using captured names
- First function in a test can be checked exhaustively (param loading); subsequent functions can skip preamble
- FileCheck variable captures: `%[[NAME:.*]]` captures the register number; back-reference with `%[[NAME]]`
- Note: `%[[X:.*]]` captures from `%7 = ...` as `7`, so back-ref `%[[X]]` matches `%7` but NOT `%7: any`. Use `%[[X]]: any` in operand positions.

## wabt BinaryReader API Gotchas
- Subclass `BinaryReaderNop` (not abstract `BinaryReaderDelegate`) — Nop provides default no-ops
- `ReadBinary(data, size, &reader, options)` — set `options.read_debug_names = true` for name section
- Init expression callbacks (`OnI32ConstExpr`, `OnGlobalGetExpr`, etc.) are **shared** across globals, elem segment offsets, elem expressions, and data segment offsets — must track context
- Legacy element segments (flags=0) emit element indices via `BeginElemExpr`/`OnRefFuncExpr`/`EndElemExpr` (not a separate `OnElemSegmentFunctionIndex`)
- `OnF32ConstExpr(uint32_t bits)` and `OnF64ConstExpr(uint64_t bits)` pass raw IEEE 754 bits, need `memcpy` to float/double
- wabt `Type` uses `Type::Enum` with negative values (e.g., I32 = -0x01 = 0x7F)
- wabt segment flags: `SegPassive=1`, `SegExplicitIndex=2`, `SegDeclared=3`, `SegUseElemExprs=4`
- Hand-crafted Wasm binaries: section sizes must be exact byte count of section content (excluding the size byte itself)
- **`BeginFunctionBody(index, size)`**: `index` is the full Wasm function index (num_func_imports + i), NOT the code-section-relative index. Do NOT add importedFunctionCount() again.
- **Wasm name section** is parsed AFTER the code section. Function names are not available when `BeginCodeSection`/`BeginFunctionBody` are called. Hermes `Function` has no name setter, so names assigned at creation time are final.
- **Callback dispatch for shared opcodes**: when the same callback (e.g., `OnI32ConstExpr`) is used in both init expressions and function bodies, use an `inFunctionBody_` flag to dispatch to the correct handler

## Hermes IR Patterns for WasmIRGen
- `Module` has private `operator delete` (from `Value` base) — always stack-allocate in tests (`Module M{Ctx}`)
- `createTopLevelFunction()` must be called before adding other functions to a Module
- `IRBuilder(Module *)` — stateful: set insertion block before creating instructions
- Functions need `createJSThisParam()` first (Hermes calling convention), then `createJSDynamicParam()` for each actual param
- `getJSDynamicParam(i)` returns param at index i (0 = "this", 1+ = user params)
- `AllocStackInst` + `LoadStackInst`/`StoreStackInst` for mutable locals
- `LoadParamInst` to read function parameters
- `LiteralNumber`, `LiteralUndefined`, `LiteralNull` are uniqued by the Module
- `BasicBlock::back()` for last instruction, iterate with range-for over `BasicBlock`
- `Function::getBasicBlockList()` for all blocks, iterate with range-for

## i32 Arithmetic IR Patterns
- `i32.add` → `AsInt32Inst(BinaryAddInst(a, b))` — truncates to int32
- `i32.sub` → `AsInt32Inst(BinarySubtractInst(a, b))`
- `i32.mul` → `CallBuiltinInst(BuiltinMethod::Math_imul, {a, b})` — needed for precision
- Bitwise ops (`and`/`or`/`xor`) → `BinaryAndInst`/`BinaryOrInst`/`BinaryXorInst` (already int32)
- Shifts → `BinaryLeftShiftInst`/`BinaryRightShiftInst`/`BinaryUnsignedRightShiftInst`
- `AsInt32Inst` outputs `:number` type (NOT `:int32`) in IR dump
- `createBinaryOperatorInst(left, right, ValueKind)` for BinaryOperatorInst
- `createCallBuiltinInst(BuiltinMethod::Math_imul, {a, b})` for Math.imul
- wabt: `OnBinaryExpr(Opcode)` for binary operations, `OnCompareExpr(Opcode)` for comparisons
- wabt: `OnConvertExpr(Opcode)` for conversions AND `i32.eqz`/`i64.eqz` (NOT `OnUnaryExpr`)
- wabt: `OnUnaryExpr(Opcode)` for unary math (e.g., `i32.clz`, `i32.ctz`, `i32.popcnt`)
- wabt opcode names: `Opcode::I32Add`, `Opcode::I32Sub`, etc. (from `opcode.def`)
- Boolean-to-i32 conversion: `BinaryOrInst(boolResult, 0)` — `true|0 → 1`, `false|0 → 0`
- Signed comparisons: `AsInt32Inst` on both operands before `BinaryLessThanInst` etc.
- Unsigned comparisons: `AsUint32Inst` on both operands before `BinaryLessThanInst` etc.
- `AsUint32Inst` available via `builder_.createAsUint32Inst(val)`
- **IR dump format**: `LiteralNumber` constants are inlined (e.g., `ReturnInst 42: number`), NOT shown as separate `%N = LiteralNumber 42` instructions. Only `Inst` subclasses get their own lines.
- After unconditional `ReturnInst`, create a dead `BasicBlock` for any subsequent dead code (same pattern needed for `br`)
- wabt: `OnReturnExpr()` and `OnDropExpr()` are zero-arg callbacks

## Control Flow (D.6) Patterns
- Function body is an **implicit block**: `beginFunction` pushes a `ControlEntry::Block` with `contBlock` = exit block. wabt's final `OnEndExpr` pops this via `onEnd()`, branches to the exit block, and `endFunction` emits `ReturnInst`.
- **Unreachable tracking**: `unreachable_` flag set after `br`, `return`, `unreachable`. Instructions are no-ops in unreachable mode. `onEnd` restores reachability if fallthrough or any `br`/`br_if` targeted the continuation.
- **branchTargeted** flag on ControlEntry: set by `onBr`/`onBrIf`. Used by `onEnd` to determine if continuation block is reachable.
- `PhiInst` must be first in block: create it by switching insertion to contBlock, creating phi, then switching back. PhiInst created empty (`createPhiInst()`), then `addEntry(val, bb)` for each incoming edge.
- `getInsertionBlock()` is NOT const — `isCurrentBlockTerminated()` cannot be `const`
- **IR dump block order**: blocks appear in creation order, not logical flow. Exit block (created in `beginFunction`) is BB1, block continuation blocks come after.
- wabt: `OnBlockExpr(Type sig_type)` — `Type::Void` for no result, `Type::I32`/etc. for single result (MVP only has single-result blocks)
- wabt: `OnLoopExpr(Type sig_type)` — same signature as `OnBlockExpr`
- wabt: `OnEndExpr()` is called for every `end` opcode, including the function body's final `end`
- `addBranchPhiOperands` must be tolerant of stack underflow when unimplemented instructions don't push expected values (use `LiteralUndefined` as placeholder)

## Loop (D.7) Patterns
- Loop `ControlEntry` has `contBlock` = loop header (br target) and `endBlock` = post-loop block
- `br` to a loop targets the loop header — no phi operands are passed (no result values)
- Loop fallthrough goes to `endBlock` — phi operands for loop results must be added inline in `onEnd`, not via `addBranchPhiOperands` (which intentionally skips Loop entries)
- `endBlock` reachability: only via fallthrough (not `branchTargeted`), since br/br_if to a loop target the header
- IR dump function signatures include param types: `function wasm_func_N(p0: any): any` (not `(p0)`)

## If/Else (D.8) Patterns
- `onIf` pops condition, creates thenBlock/elseBlock/mergeBlock, emits `CondBranchInst`, pushes `ControlEntry::If`
- `onElse`: branches then-arm to mergeBlock with phi operands, resets stack, sets insertion to elseBlock, nulls `entry.elseBlock`
- `onEnd` for If: `entry.elseBlock != nullptr` means "if without else" — emit branch from elseBlock to mergeBlock
- If without else and without result type is valid; if without else WITH result type is invalid Wasm (caught by wabt validation)
- In unreachable mode, `onIf` pushes a dummy entry so `onEnd`/`onElse` can properly pop it
- wabt callbacks: `OnIfExpr(Type sigType)`, `OnElseExpr()`, `OnEndExpr()`
- WAT files use `;;` for comments — single `;` causes parse errors in wat2wasm

## br_table (D.9) Patterns
- `SwitchInst(input, defaultBlock, caseValues, caseBlocks)` — `caseValues` is `ArrayRef<Literal*>`, `caseBlocks` is `ArrayRef<BasicBlock*>`
- Use trampoline blocks for each unique depth: trampoline adds phi operands and branches to target's contBlock
- Multiple case values with the same depth share a trampoline (`DenseMap<uint32_t, BasicBlock*>`)
- Phi operand handling: peek at stack (don't pop) — same as `onBrIf` since the values are consumed by the branch
- wabt: `OnBrTableExpr(Index numTargets, Index* targetDepths, Index defaultTargetDepth)` — depths array does NOT include the default

## Select (D.10) Pattern
- `select` → `CondBranchInst(cond, trueBlock, falseBlock)` + `PhiInst(val1, trueBlock, val2, falseBlock)` in mergeBlock
- Stack order: push val1, push val2, push cond. Pop: cond, val2, val1. Result: cond ? val1 : val2
- wabt callback: `OnSelectExpr(Index resultCount, Type* resultTypes)`
- **Lit test gotcha**: blocks appear in creation order — exit block (created by `beginFunction`) appears before select's blocks. Use explicit block numbers (BB2, BB3, BB4) in CHECK lines instead of `%BB{{[0-9]+}}` which can false-match the exit block

## Function Calls (D.12) Pattern
- Every function needs a closure (it's just the calling mechanism). All closures are created in the top-level function — there is no reason to create them elsewhere.
- Closures are pre-created once in the top-level function and stored as Variables in `topLevelVS_` via `StoreFrameInst`
- `call funcIndex` → `LoadFrameInst(parentScope, closureVars_[funcIndex])` + `CallInst`
- Per-function `CreateScopeInst` eliminated — Wasm functions have no captured variables
- Each wasm function only has `GetParentScopeInst` to access the top-level environment for loading closures
- Arguments popped from value stack in reverse order (top = last arg)
- `createCallInst(callee, newTarget, thisValue, args)` — pass `LiteralUndefined` for both newTarget and thisValue
- `BaseCallInst::getNumArguments()` includes `this` as argument 0 — a Wasm call with N args returns N+1
- wabt callback: `OnCallExpr(Index funcIndex)` — funcIndex is full Wasm function index (including imports)
- `LoadFrameInst` callee type annotation is `:any` (not `:object` like `CreateFunctionInst`)

## D.13 Callback Wiring Patterns
- `warnUnsupported(name, numInputs, numOutputs)` in WasmIRGen: pops inputs, pushes undefined placeholders, emits stderr warning
- wabt opcode-to-callback mapping: binary arithmetic → `OnBinaryExpr`, comparisons → `OnCompareExpr`, eqz/conversions/truncations/reinterpret → `OnConvertExpr`, unary math/sign-extension → `OnUnaryExpr`, loads → `OnLoadExpr`, stores → `OnStoreExpr`
- Sign-extension opcodes (I32Extend8S, etc.) go through `OnUnaryExpr`, NOT `OnConvertExpr`
- Saturating truncations (I32TruncSatF32S, etc.) go through `OnConvertExpr`
- `wabt::Opcode::GetName()` returns the opcode name string (e.g., "i32.load") — useful for load/store variants
- `OnCallIndirectExpr(sigIndex, tableIndex)` — numInputs = params.size() + 1 (for table index)

## D.14 Integration Patterns (compile .wasm → .hbc → run)
- Top-level function (`global()`) required by `hbc::generateBytecodeModule()` — create with `builder_.createTopLevelFunction("global", true)`
- Set `setExpectedParamCountIncludingThis()` for ALL functions (optimizer assertion); top-level gets 1 (just "this")
- Scope: single `topLevelVS_` with one `Variable` per function (holding pre-created closures). No per-function `VariableScope` or `CreateScopeInst`.
- `GetParentScopeInst` in each wasm function resolves to `topLevelVS_` — used to `LoadFrameInst` closures at call sites
- Top-level body: `CreateScopeInst` + `CreateFunctionInst` + `StoreFrameInst` for each function, then optional start function call via `LoadFrameInst` + `CallInst`, then exports object
- Exports object: `AllocObjectLiteralInst({})` + for each function export: `LoadFrameInst` + `StorePropertyStrictInst(closure, obj, getLiteralString(name))`. Top-level returns the object, not undefined.
- Only function exports handled; memory/table/global exports silently skipped (no runtime objects yet)
- Dead blocks: BCGen verifier requires all blocks to be reachable with terminators. Remove dead blocks in `endFunction()` via `eraseFromParent()`. Do NOT use `UnreachableInst` on dead blocks (causes dominance tree error).
- `Module` must be `std::make_shared<Module>()` for `processWasmFile()` (BCGen shared_ptr interface)
- Default optimization level is `OMax` — unreferenced functions get eliminated. Use `-O0` in `--dump-ir` lit tests (matches JS IRGen test pattern).
- End-to-end test commands: `%hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes %t.hbc` or `%hermes --wasm %t.wasm`

## f64 Arithmetic (E.1) Patterns
- **Do NOT use F-prefixed instructions** (`FBinaryMathInst`, `FUnaryMathInst`, `FCompareInst`) from the frontend — they require number-typed inputs (`left->getType().isNumberType()` assertion). Values from `AllocStackInst(Type::createAnyType())` + `LoadStackInst` have `:any` type. F-prefixed instructions are for the optimizer after type inference.
- Use regular `BinaryOperatorInst` + `UnaryOperatorInst` which accept `:any` typed inputs
- f64 arithmetic: `BinaryAddInstKind`, `BinarySubtractInstKind`, `BinaryMultiplyInstKind`, `BinaryDivideInstKind`
- f64.neg: `UnaryMinusInstKind`
- Math builtins: `CallBuiltinInst(BuiltinMethod::Math_abs/sqrt/ceil/floor/trunc/round/min/max, {args})`
- f64 comparisons: same pattern as i32 — `BinaryStrictlyEqualInstKind` etc. + `BinaryOrInst(cmpResult, 0)` for bool→i32
- f64.promote_f32: no-op in Phase 1 (all values are double)
- f64.nearest → `Math.round` (approximation: Wasm wants ties-to-even, JS rounds ties to +infinity)
- f64.copysign: deferred to Part F (needs helper)
- **Math.fround is NOT a builtin** in Hermes — not in Builtins.def. Adding it requires modifying `Builtins.def` + bumping bytecode format version. Phase 1 leaves f32 ops at f64 precision.
- **IR dump CallBuiltinInst format**: `CallBuiltinInst (:any) [Math.abs]` — type annotation `(:any)` appears between instruction name and builtin name. Use `CallBuiltinInst {{.*}}[Math.` in CHECK patterns.

## f32 Arithmetic (E.2) Patterns
- f32 ops use identical IR to f64 ops (no per-op rounding in Phase 1)
- f32.demote_f64: no-op in Phase 1 (value stays at f64 precision)
- f32 comparisons: identical pattern to f64/i32 — `BinaryOperatorInst` + `BinaryOrInst(cmp, 0)`
- f32.copysign: deferred to Part F (same as f64.copysign)
- f32 constants ARE correctly rounded (via `float` cast in `onF32Const`)
- **Wasm start function must be void**: `(start $func)` requires the function to have no result type. Using `(result i32)` etc. causes a wabt validation error.
- **e2e test pattern**: don't pipe `%hermes` runtime output to FileCheck for empty-output tests. Use separate `%hermes %t.hbc` line (no pipe) for execution, and `%hermesc --dump-ir` piped to FileCheck for IR verification.

## WasmHelpers Infrastructure (F.1)
- **Adding a new private builtin** (full chain): (1) `Builtins.def`: `PRIVATE_BUILTIN(name)`, (2) `PredefinedStrings.def`: `STR(name, "name")`, (3) `NativeFunctions.def`: `NATIVE_FUNCTION(name)`, (4) `HermesBuiltin.cpp`: implement function + register via `defineInternMethod(B::HermesBuiltin_name, P::name, funcName, argCount)`, (5) `WasmHelpers.h/cpp`: add `emit*()` wrapper
- Builtin enum: `BuiltinMethod::HermesBuiltin_name` (from `PRIVATE_BUILTIN(name)`)
- IR emit: `builder_.createCallBuiltinInst(BuiltinMethod::HermesBuiltin_name, {args})`
- **Must bump bytecode version** when modifying `Builtins.def` (file: `BytecodeVersion.h`)
- `PRIVATE_BUILTIN` entries must be unconditional (not `#ifdef`-gated) — they affect the enum numbering which is part of the bytecode format
- `raiseError(TwineChar16)` for generic Error, `raiseTypeError(Handle<>)` for TypeError — no `raiseError(Handle<>)` overload exists
- Private builtins are registered in `HermesBuiltin.cpp::populateHermesInternalBuiltins()` via `defineInternMethod()`, EXCEPT `functionPrototypeApply/Call` which are registered in `Function.cpp` via `runtime.registerBuiltin()`
- Builtin budget: `_count <= 256` (uint8_t index). Current count is well under 256.

## i32 Trapping Division (F.2)
- **x86 SIGFPE gotcha**: On x86, `INT32_MIN / -1` and `INT32_MIN % -1` both cause SIGFPE because `idiv` computes quotient and remainder together. Must check `a == INT32_MIN && b == -1` BEFORE executing `/` or `%` in C++.
- `truncateToInt32()` from `hermes/Support/Conversions.h` converts double→int32 (same as JS ToInt32)
- Unsigned ops: `static_cast<uint32_t>(truncateToInt32(d))` — NOT `truncateToUInt32()` (which is just the same cast)
- Native function args: `NativeArgs args = runtime.getCurrentFrame().getNativeArgs();` then `args.getArg(N).getNumber()`
- Return: `HermesValue::encodeTrustedNumberValue(result)`
- CallBuiltinInst IR dump format: `[HermesBuiltin.wasmI32DivS]` — full `HermesBuiltin.` prefix, not just the short name
- `getBuiltinIndex()` returns `BuiltinMethod::Enum`, compare directly (no `static_cast<uint32_t>` needed)

## i32 Bit Manipulation (F.3)
- `Math.clz32` is NOT in `Builtins.def` — cannot use `CallBuiltinInst(Math.clz32)`. Must use a private builtin helper instead.
- `__builtin_clz(0)` and `__builtin_ctz(0)` are undefined behavior in C — must handle 0 explicitly (return 32 per Wasm spec)
- `__builtin_popcount` is defined for 0 (returns 0) — no special case needed
- Rotate: `shift & 31` can be 0. Shifting uint32_t by 32 is UB in C++. Guard with `shift == 0 ? a : ...`
- Sign-extension (`extend8_s`, `extend16_s`) can be done inline as IR shifts: `(a << N) >> N`. No private builtin needed.
- FileCheck variable captures with type annotations: `%[[VAR:.*]]` captures `6` from `%6 = ...`, but back-reference `%[[VAR]]` won't match `%6: any`. Use `%{{.*}}` instead for simpler matching.

## Conversion Helpers (F.4)
- f32 truncation variants can reuse f64 builtins in Phase 1 (all values are doubles)
- Int-to-float conversions are just `AsInt32Inst`/`AsUint32Inst` (no builtin needed)
- Trapping truncation range checks: signed [-2^31, 2^31 - 1], unsigned [0, 2^32 - 1]
- Saturating: NaN→0, overflow→MAX, underflow→MIN (no trap)
- Reinterpret/bitcast: use `memcpy` for type-punning (not `reinterpret_cast` or union)
- **`AsUint32Inst`** (lowercase 'i') — NOT `AsUInt32Inst`. The Hermes class name uses "int" not "Int".
- e2e tests that need output: can't use imports yet (Part I). Use call-and-drop + IR dump checks.
- `#include <cmath>` for `std::isnan`, `std::trunc`, `std::isfinite`; `#include <cstring>` for `memcpy`

## Copysign Helpers (F.5)
- `std::copysign(a, b)` from `<cmath>` — copies sign of b onto magnitude of a
- f32 copysign narrows to `float` before `std::copysign`, then widens back to `double`
- f64/f32 min/max already done in E.1/E.2 using `Math.min`/`Math.max` (Wasm-compatible semantics)
- Bytecode version now 103

## i64 Split Representation (G.1)
- Each i64 value occupies **two** value stack slots: `[lo32, hi32]` (lo pushed first, hi on top)
- `pushI64(lo, hi)` / `popI64() → {lo, hi}` / `isTopI64()` helpers on WasmIRGen
- Parallel `valueStackIsI64Hi_` vector tracks which slots are the hi32 half
- Must keep `valueStackIsI64Hi_` in sync with `valueStack_` at every `clear()`/`resize()` site (7 locations in WasmIRGen.cpp)
- `onDrop` checks `isTopI64()` to decide whether to pop 1 or 2 slots
- i64 locals and control flow (phi nodes) are handled in G.5, not G.1

## i64 Arithmetic Helpers (G.3)
- Thread-local hi-stash pattern: `wasmI64HiStash_` stores hi32 from binary ops; `wasmI64HiResult()` retrieves it
- `argsToI64(args, loIdx, hiIdx)` reconstructs int64_t from split lo/hi NativeArgs
- `splitI64Result(val)` / `splitU64Result(val)` split result and stash hi32
- i64 bitwise (and, or, xor) done inline in IR — no builtin needed, per-half BinaryAndInst/OrInst/XorInst
- i64 unary (clz, ctz, popcnt) return i64 with hi=0 (result always fits in 6 bits)
- i64 comparisons return single i32 (not split pair)
- Division: check zero-divisor and INT64_MIN/-1 overflow (trap for div, return 0 for rem)
- Shifts mask by 63. Rotates guard shift==0 to avoid C++ UB.
- **GCScope handle limit**: `createHermesBuiltins` needs its own `GCScope{runtime, "name", 128}` with `flushToMarker` per call — adding many builtins exhausts outer scope's 48-handle limit
- **Stale binary gotcha**: ninja incremental build may not relink after header-only changes; may need to delete binary and rebuild
- Bytecode version now 105
- i64 tests must use only `i64.const` (not i64 params) until G.5 implements i64 locals

## i64 Conversion Helpers (G.4a, G.4b)
- Inline IR (G.4a): `i32.wrap_i64` takes lo, discards hi; `i64.extend_i32_s/u` computes hi from sign; sign-ext ops use shift pairs
- Float→i64 trunc builtins (G.4b): take single f64 arg, return lo32 via `splitI64Result`/`splitU64Result`, hi32 stashed
- f32 variants reuse f64 builtins in Phase 1 (all values are doubles)
- Signed i64 trapping range: `[-2^63, 2^63)` — check `t >= 9223372036854775808.0` (exact double = 2^63)
- Unsigned i64 trapping range: `[0, 2^64)` — check `t >= 18446744073709551616.0` (exact double = 2^64)
- Saturating: NaN→0, overflow→MAX, underflow→MIN
- After calling trunc builtin: must call `emitI64HiResult()` and `pushI64(lo, hi)` to push result as i64 pair
- `static_cast<int64_t>(double)` and `static_cast<uint64_t>(double)` are defined behavior in C++ when value is in range

## i64→float Conversions (G.4c)
- i64→float builtins take split lo/hi (2 args), return a single f64 result (not split)
- f32 conversions narrow to `float` then widen back: `static_cast<double>(static_cast<float>(i64_val))`
- Reinterpret uses `memcpy` for type-punning (same as F.4 i32↔f32 bitcast)
- `i64.reinterpret_f64` returns split i64 (lo via return, hi via stash) — same as G.4b pattern
- `f64.reinterpret_i64` takes split lo/hi, returns single f64
- Bytecode version now 106

## i64 Locals and Control Flow (G.5)
- `localTypes_` vector stores Wasm type for each local; `localSlotIndex_` maps local index → first slot in `locals_` vector
- i64 locals occupy 2 consecutive slots in `locals_` (lo, hi); i64 params get 2 `JSDynamicParam` entries (p0_lo, p0_hi)
- `beginFunction` creates `AllocStackInst` + `LoadParamInst` (or zero-init `StoreStackInst`) per slot
- `onLocalGet/Set/Tee` check `localTypes_[idx]` to decide single vs split access
- i64 function returns: callee calls `wasmI64HiStash(hi)` builtin then `ReturnInst(lo)`. Caller gets lo from `CallInst`, hi from `emitI64HiResult()`
- `wasmI64HiStash` is a new private builtin (Builtins.def → PredefinedStrings.def → NativeFunctions.def → HermesBuiltin.cpp)
- Control flow: `createResultPhis(block, resultTypes)` creates 2 phis per i64 result type; `pushResultPhis(entry)` pushes them as i64 pairs
- `peekBranchPhiOperands(entry)` — separate from `addBranchPhiOperands` for br_if (peek, don't pop)
- `setExpectedParamCountIncludingThis` must account for expanded i64 params (each i64 = 2 JS params)
- Bytecode version now 107

## Memory Access (H.1) Patterns
- 8 typed array views stored as Variables in `topLevelVS_`: HEAP8, HEAPU8, HEAP16, HEAPU16, HEAP32, HEAPU32, HEAPF32, HEAPF64
- `MemView` enum in WasmIRGen.h indexes into `memViewVars_[]`
- Top-level function creates: `new ArrayBuffer(pages * 65536)` + `new TypedArray(buffer)` for each view
- `emitNew(ctor, args)` uses `CreateThisInst` + `CallInst(callee=ctor, newTarget=ctor)` + `GetConstructedObjectInst`
- `TryLoadGlobalPropertyInst("ArrayBuffer")` to get constructor from global scope
- `LoadFrameInst(parentScopeInst_, memViewVars_[HEAP32])` to get the view in wasm functions
- Loads: `LoadPropertyInst(view, index)` + OOB check (`=== undefined` → trap)
- Stores: `StorePropertyStrictInst(value, view, index)` (OOB silently fails in Phase 1)
- Index computation: `addr >>> shift` where shift = log2(element_size)
- Int8Array/Int16Array return signed values natively — no explicit `(val << N) >> N` needed for `load8_s`/`load16_s`
- i64.load: two HEAPU32 accesses at `addr>>>2` (lo) and `(addr>>>2)+1` (hi)
- i64 narrow loads: load from appropriate view, hi = `lo >> 31` (signed) or `0` (unsigned)
- Data segments NOT initialized by H.1 — deferred to M.4 (WebAssembly.Instance)

## Memory Size/Grow (H.2) Patterns
- `memory.size`: pure inline IR — `HEAPU8.length >>> 16` (no builtin needed)
- `memory.grow`: hybrid — native builtin creates new ArrayBuffer + copies data; inline IR creates new views + stores them
- `wasmMemoryGrow(heapu8, delta, maxPages)` native function: uses `JSArrayBuffer::create` + `createDataBlock` + `copyDataBlockBytes`
- Returns new ArrayBuffer on success, -1 (number) on failure. On allocation failure, clears exception and returns -1
- Inline IR for grow: compute oldPages → call builtin → compare result to -1 → CondBranch → success block creates 8 new views + StoreFrameInst → fail block is empty → merge block PhiInst(-1 or oldPages)
- Max pages from module info: `moduleInfo_.memories[0].limits.maximum` (or import memory limits)
- `PinnedValue<T>.getHermesValue()` (dot, not arrow) returns the HermesValue encoding of the pinned object
- `PinnedValue<T>->` dereferences to `T*` — `T*` doesn't have `getHermesValue()`
- Bytecode version now 108

## Unaligned Access (H.3) Patterns
- Compile-time check: `alignLog2 < getNaturalAlignLog2(opcode)` → byte-assembly path via HEAPU8
- `getNaturalAlignLog2`: 3 for i64/f64, 2 for i32/f32/i64.load32, 1 for 16-bit, 0 for 8-bit
- `emitUnalignedLoad(addr, N)`: reads N bytes from HEAPU8, assembles via `b0 | (b1 << 8) | ... | (bN-1 << (N-1)*8)`. OOB check on byte 0 only.
- `emitUnalignedStore(addr, val, N)`: decomposes val via `(val >>> i*8) & 0xFF` per byte, stores to HEAPU8
- f64 unaligned: load two 4-byte halves (JS bitwise = 32-bit), reinterpret via `wasmF64ReinterpretI64(lo, hi)`
- f32 unaligned: load 4 bytes, reinterpret via `wasmF32ReinterpretI32(raw)`
- f64 unaligned store: `wasmI64ReinterpretF64(val)` → split lo/hi, byte-store each half
- f32 unaligned store: `wasmI32ReinterpretF32(val)` → byte-store
- i64 unaligned load/store: process lo32 and hi32 halves separately (4 bytes each)
- No new builtins needed — reuses existing reinterpret builtins from F.4/G.4c
- Known limitation: naturally-annotated access at a misaligned runtime address gives incorrect results (spec deviation)
- JS driver pattern: `hermescli.getScriptArgs()[0]` + `hermescli.loadHBC(hermescli.loadFile(path))`

## Export Wrappers (I.1) Patterns
- Each exported Wasm function gets a wrapper IR function (`wasm_export_<name>`)
- Wrapper takes 1 JS param per Wasm param (not split for i64)
- i32 args: `AsInt32Inst(param)` — coerces to int32
- f32/f64 args: pass through directly (already JS Numbers)
- i64 args (Phase 1): `AsInt32Inst(param)` for lo32, `LiteralNumber(0)` for hi32
- void return: `ReturnInst(undefined)`
- i64 return (Phase 1): returns lo32 only (hi32 lost to JS)
- Exports object stores wrapper closures (`CreateFunctionInst`), not internal closures
- `createExportWrapper` is called during `createFunctions()` — builds wrapper body then returns to top-level context
- Wrapper accesses internal closure via `GetParentScopeInst` + `LoadFrameInst(closureVars_[funcIndex])`

## Import Trampolines (I.2) Patterns
- Imports resolved from `globalThis.__wasm_imports__` via `TryLoadGlobalPropertyInst` (not function parameter — `runBytecode` calls global with 0 args)
- Each imported function gets a Variable `import_func_N` in `topLevelVS_` (separate from `closureVars_`)
- Top-level resolves `__wasm_imports__[moduleName][fieldName]` → `StoreFrameInst` into `importFuncVars_[i]`
- Trampoline replaces stub body: `GetParentScopeInst` + `LoadFrameInst(importFuncVar)` + marshal args + `CallInst(jsFunc)` + marshal return
- Arg marshaling (Wasm→JS): i32/f32/f64 pass through; i64 passes only lo32 (Phase 1)
- Return marshaling (JS→Wasm): i32 → `AsInt32Inst`; f64/f32 → pass through; i64 → `AsInt32Inst(lo)` + `HiStash(0)`; void → undefined
- When M.4 is implemented, replace `__wasm_imports__` with proper parameter passing
- No new builtins needed; no bytecode version change
- e2e test pattern: JS driver sets `globalThis.__wasm_imports__` before `hermescli.loadHBC()`

## Table Representation (J.1) Patterns
- Two JS Arrays per table in `topLevelVS_`: `tableFuncVars_[i]` (closures), `tableTypeVars_[i]` (type indices for call_indirect)
- Created via `new Array(initialSize)` using `emitNew` — sparse JS array, uninitialized = `undefined`
- Active element segments: iterate `funcIndices`, `StorePropertyStrictInst` closures + type indices at offset+i
- Type index lookup: `WasmFunction.typeIndex` for defined funcs, `WasmImport.typeIndex` for imported funcs
- `table.get` → `LoadPropertyInst(funcsArr, index)`, `table.set` → `StorePropertyStrictInst`
- `table.size` → `LoadPropertyInst(funcsArr, "length")`, `table.grow` → always -1 (Phase 1)
- `loadTableFuncs(tableIndex)` / `loadTableTypes(tableIndex)` load from parentScopeInst_
- Only `i32.const` offset expressions supported for element segments (global.get deferred)
- `ref.null func` is not yet implemented — emits "unsupported" warning and pushes undefined
- Bytecode version now 109

## call_indirect (J.2) Patterns
- Two-step: builtin validates (bounds, null, type) and returns closure; IR emits `CallInst` on the closure
- `wasmCallIndirect(funcsArr, typesArr, index, expectedTypeIdx)` — 4 args
- Uses `JSArray::at(runtime, index)` for O(1) access; empty = uninitialized
- `JSArray::getLength(arr, runtime)` for bounds check
- `onCallIndirect` pops table index first, then pops args in reverse (same pattern as `onCall`)
- Type index passed as `LiteralNumber(sigIndex)` — the Wasm type index from the instruction immediate
- Error messages: "undefined element" (OOB), "uninitialized element" (null), "type mismatch"

## Wasm Globals (K.1) Patterns
- Globals stored as Variables in `topLevelVS_`, same pattern as memory views/tables/closures
- `globalVars_` vector + `globalSlotIndex_` for i64 split (2 slots per i64 global)
- Access: `GetParentScopeInst` + `LoadFrameInst`/`StoreFrameInst` from `parentScopeInst_`
- Init expressions: evaluated inline in top-level function body (no builtins needed)
- Imported globals zero-initialized in Phase 1 (proper import resolution in M.4/M.7)
- IR dump Variable format: `[%VS0.global_N]` (not `[global_N]`)
- wabt validation: `global.get` in init expressions can ONLY reference imported globals, not defined globals
- `ref.func` in function body requires the function to be declared in an `(elem declare func ...)` section
- No bytecode version change needed (no new builtins)

## Exception Handling (L.1) Patterns
- Wasm exceptions = JSArray `[tagIndex, v0, v1, ...]`; i64 payload values split as lo32, hi32
- Two private builtins: `wasmCreateException(tagIdx, v0, v1, ...)` and `wasmMatchException(caught, tagIdx)`
- `wasmMatchException` returns the JSArray on match, `undefined` on mismatch
- Wasm `try` maps to Hermes `TryStartInst(catchBlock, tryBody)` — uses existing IR exception support
- Multi-catch: first `catch` is primary catch block (CatchInst); subsequent catches chain via `nextCatchBlock`
- `catch_all` is the final fallback — no tag matching, directly handles caught value
- `rethrow` uses stored `caughtValue` from enclosing catch handler
- `delegate` implemented as catch-and-rethrow (semantically equivalent)
- **CRITICAL**: Must call `fixupCatchTargets(currentFunc_)` in `endFunction()` — sets catch target operands on ThrowInst inside try blocks. Without this, backend verifier fails.
- `catch_all` catches everything including traps (Phase 1 spec deviation)
- wabt requires `options.features.enable_exceptions()` in `ReadBinaryOptions` to parse tag sections
- Tags stored in `WasmModuleInfo::tags` vector; `getTagType(tagIndex)` returns the `WasmFuncType`
- Bytecode version 110

## WebAssembly JS API (M.1+) Patterns
- `NativeErrorTypes.def` registers errors as globals — don't use for namespaced errors
- `defineSystemConstructor()` also registers on global object (line 55-56 of JSLibInternal.cpp)
- For namespace-only constructors: create `NativeConstructor` manually, call `Callable::defineNameLengthAndPrototype`, then `JSObject::defineOwnProperty` on the namespace object
- `NativeConstructor::create` returns `PseudoHandle<NativeConstructor>` — must store in `PinnedValue` before passing as Handle
- WebAssembly error types: `runtime.wasmCompileErrorConstructor/Prototype`, `runtime.wasmLinkErrorConstructor/Prototype`, `runtime.wasmRuntimeErrorConstructor/Prototype`
- WebAssembly namespace object created in `lib/VM/JSLib/WebAssembly/WebAssembly.cpp`
- `RUNTIME_HV_FIELD` entries can be `#ifdef`-gated (unlike `PRIVATE_BUILTIN` which affects bytecode format)
- Intl pattern: sub-constructors are in a separate namespace (`intl::createIntlObject`), Wasm keeps it in `vm::` namespace

## WebAssembly.validate (M.2) Patterns
- `validateWasmBinary` in WasmCompile.cpp: uses `BinaryReaderNop` subclass with `OnError` returning `true` to suppress stderr
- wabt `PrintError` calls `delegate_->OnError(error)` — if it returns `true` (handled), no stderr output; `false` → prints to stderr
- **Weak symbol pattern** for VM↔WasmFrontend boundary: `__attribute__((__weak__))` in `WebAssembly.cpp` provides fallback; real impl in WasmFrontend overrides when linked. Avoids cross-library dependency. Pattern already exists in `Runtime.cpp` (`test_wasm_host_timeout`).
- `hermescompiler` does NOT include `hermesVMRuntime_obj` — cannot have WasmFrontend depend on VM symbols
- `hermesvmlean_a` does NOT include `hermesWasmFrontend_obj`/`wabt` — lean VM needs weak fallbacks
- `hermesvm_a` (full) includes BOTH `hermesVMRuntime_obj` and `hermesWasmFrontend_obj` — strong symbols win
- `extractBufferSourceBytes(runtime, arg, data, size)`: extracts bytes from JSArrayBuffer or JSTypedArrayBase; checks `attached()`
- `defineMethod(runtime, obj, symbolID, context, nativeFn, paramCount)` — registers non-enumerable method
- `PredefinedStrings.def`: add new predefined strings near related entries (e.g., `validate` near WebAssembly strings)

## WebAssembly.Module (M.3) Patterns
- `JSWebAssemblyModule` is a `JSObject` subclass wrapping `std::unique_ptr<WasmModuleData>` (native C++ data)
- `WasmModuleData` lives in `include/hermes/WasmFrontend/WasmModuleData.h` — standalone struct (no VM deps) shared by both WasmFrontend and VM
- `WasmModuleData` has virtual destructor for subclassing in M.4 (will add compiled bytecode)
- `CellKinds.def` entries are UNCONDITIONAL — they affect enum numbering globally. `JSWebAssemblyModule.cpp` must always be compiled; only WebAssembly.cpp constructor/methods are `#ifdef HERMES_ENABLE_WASM` gated.
- `HERMES_VM_GCOBJECT(JSWebAssemblyModule)` in `HermesValueTraits.h` is REQUIRED for `PseudoHandle<JSWebAssemblyModule>` to work
- `JSWebAssemblyModule::create` uses `runtime.makeAFixed<JSWebAssemblyModule, HasFinalizer::Yes>(...)` — must have finalizer to clean up native `unique_ptr`
- Module.exports() and Module.imports() are static methods on the CONSTRUCTOR (not prototype), per WebAssembly JS API spec
- `compileWasmToModuleData` uses BinaryReaderHermesIRGen without calling `setIRGen()` — skips function body IR generation, only parses module-level metadata
- `@@toStringTag` = `"Module"` on prototype → `toString()` gives `[object Module]` (not `[object WebAssembly.Module]`)
- `NativeConstructor::create(runtime, parentObj, context, fn, paramCount)` — `parentObj` is usually `runtime.functionPrototype`
- `Callable::defineNameLengthAndPrototype` sets `.name`, `.length`, and `.prototype` on the constructor
- Runtime HV fields: `wasmModuleConstructor` (NativeConstructor), `wasmModulePrototype` (JSObject) — `#ifdef HERMES_ENABLE_WASM` gated
- `JSArray::setElementAt` is `[[nodiscard]]` — must cast to `(void)` when ignoring return

## WebAssembly.Instance (M.4)
- Instance constructor: validates Module arg, sets `__wasm_imports__` on globalThis, runs `runtime.runBytecode()`, captures exports, freezes them
- `WasmModuleData::bytecodeProvider` is `shared_ptr<hbc::BCProviderBase>` — use `BCProviderBase` (not `BCProvider`) to avoid the `using BCProvider = BCProviderBase` alias conflict
- `compileWasmToModuleData` does FULL compilation now (parse → IR → optimize → BCGen → BCProviderFromSrc)
- WasmFrontend CMakeLists needs: `hermesOptimizer hermesHBCBackend hermesBackend` link deps for full compilation
- **Wasm binary section order**: Type, Import, Function, Table, Memory, Global, Export, Start, Element, **Code**, **Data**. Data section is AFTER Code section.
- **`createFunctions()` vs `finalizeModule()` split**: `createFunctions()` runs at `BeginCodeSection` (before data is parsed). `finalizeModule()` runs at `EndModule` (after ALL sections). Data segment init, start function call, export wrapper creation, and ReturnInst are in `finalizeModule()`.
- `EndModule()` in BinaryReaderHermesIRGen: (1) calls `createFunctions()` if no code section (empty modules), (2) always calls `finalizeModule()`.
- `tlEntry_` and `tlScope_` member variables bridge the two phases
- `raiseLinkError()` helper follows same pattern as `raiseCompileError()` — uses `wasmLinkErrorPrototype`
- `JSWebAssemblyInstance` is a trivial JSObject subclass (no special GC fields)
- Hand-crafted Wasm test binaries: data section has flags byte (0x00 = active memory 0), then init expr (`i32.const 0`, `end`), then data size, then data bytes

## WebAssembly.Memory (M.5)
- `JSWebAssemblyMemory` has a `GCPointer<JSArrayBuffer>` field — must register with `mb.addField("buffer", &self->buffer_)` in BuildMeta
- `GCPointer<T>` constructor: `buffer_(runtime, nullptr, runtime.getHeap())` — the null init is important
- `GCPointer<T>::set(runtime, ptr, heap)` — note the 3-arg form, not just `set(runtime, ptr)`
- Prototype forward-declaration in `GlobalObject.cpp` is REQUIRED — without it, `Handle<JSObject>{runtime.wasmMemoryPrototype}` hits null deref assertion
- `JSArrayBuffer::createDataBlock(runtime, buf, 0)` is safe and correctly sets size to 0 — DO NOT skip for empty buffers
- `defineAccessor(runtime, proto, symbolID, ctx, getter, setter, enumerable, configurable)` — getter-only: pass `nullptr` for setter
- `toNumber_RJS(runtime, pinnedValue)` works directly with PinnedValue (implicit Handle conversion)
- `std::floor` from `<cmath>` for integer validation (`x != std::floor(x)` catches NaN and non-integers)
- Memory grow creates NEW ArrayBuffer + copies — old buffer is not explicitly detached (Phase 1 simplification)

## WebAssembly.Table (M.6)
- `JSWebAssemblyTable` has a `GCPointer<JSArray>` field for elements — same pattern as Memory's buffer GCPointer
- `dyn_vmcast<Callable>(*hermesValue)` to check if a HermesValue is callable — do NOT use `vmisa<Callable>(getObject())` (won't compile)
- `runtime.makeHandle(val)` — NOT `Runtime::makeHandle(val)` (not static)
- Table entries initialized to `null` (not `undefined`) per WebAssembly spec
- `JSArray::create(runtime, capacity, capacity)` creates an array with both capacity and length set
- `JSArray::setElementAt(arr, runtime, i, handle)` is `[[nodiscard]]`; cast to `(void)` when ignoring
- `JSArray::getLength(arr, runtime)` returns the length (not `.length` property access)
- 3 WasmIRGen unit tests (CreateFunctionsExportsObject, CreateFunctionsNoExports, CreateFunctionsSkipsNonFunctionExports) are pre-existing failures — not related to M.6

## WebAssembly.Global (M.7)
- `JSWebAssemblyGlobal` stores value as plain `double` (no GC pointer) — no `addField` in BuildMeta
- `StringPrimitive::equals(ASCIIRef)` does NOT exist — `equals` takes `const StringPrimitive*` or `StringView`
- `StringPrimitive::castToASCIIRef()` is private — cannot use it from outside the class
- For string comparison with short ASCII literals: create temporary `StringPrimitive::create(runtime, ASCIIRef(...))` then `equals(vmcast<StringPrimitive>(*res))`
- `mutable` is a C++ keyword — predefined string uses `mutable_` as identifier, `"mutable"` as value
- Runtime HV fields: `wasmGlobalConstructor`, `wasmGlobalPrototype` — `#ifdef HERMES_ENABLE_WASM` gated
- `value` accessor (getter+setter) on prototype; `valueOf()` method delegates to getter
- i32 truncation: `static_cast<int32_t>(static_cast<int64_t>(val))`; f32 narrowing: `static_cast<float>(val)`

## WebAssembly.compile/instantiate (M.8)
- `callPromiseResolve(runtime, value)` / `callPromiseReject(runtime, error)` — look up `Promise.resolve`/`reject` from global scope and call
- Pattern: synchronous compilation, wrap result in `Promise.resolve()`; on error, capture thrown exception, clear it, wrap in `Promise.reject()`
- `instantiateModuleImpl(runtime, mod, importObj)` — shared helper used by both `wasmInstanceConstructor` and `wasmInstantiate`
- Promise `.then()` callbacks run as microtasks after all synchronous code — lit test CHECK lines must account for this ordering
- New predefined strings: `compile`, `instantiate`, `instance`, `Promise`, `resolve`, `reject`
- `TypeError` for non-buffer args is thrown synchronously (not wrapped in Promise) — matches spec behavior

## WebAssembly.Tag and WebAssembly.Exception (M.9)
- `JSWebAssemblyTag`: stores `std::vector<ValType>` for parameter types — needs `HasFinalizer::Yes` + `_finalizeImpl`/`_mallocSizeImpl`
- Do NOT explicitly call `~vector()` in the destructor — `_finalizeImpl` calls `~JSWebAssemblyTag()` which already destroys all members. Explicit call causes double-free.
- `JSWebAssemblyException`: stores `GCPointer<JSWebAssemblyTag>` and `GCPointer<JSArray>` — register both in BuildMeta via `mb.addField()`
- `exception.is(tag)` uses object identity comparison (`exc->getTag(runtime) == tag`), not structural equality
- `exception.getArg(tag, index)` requires tag identity match first, then extracts from payload JSArray
- `parseValTypeString(runtime, str, result)` helper reusable for parsing "i32"/"i64"/"f32"/"f64" strings
- New predefined strings: `Tag`, `Exception`, `getArg`, `parameters`
- CellKinds.def entries for new types are UNCONDITIONAL (same as all other WebAssembly types)

## Bulk Memory Operations (N.1) Patterns
- Data segments array: JS Array of Uint8Arrays in `topLevelVS_`, variable `dataSegVar_`
- Lazy creation via `getOrCreateDataSegVar()` — Data section is parsed AFTER Code section, so variable must be created on first use during function body compilation
- Active segments set to null (dropped) in the array after their byte-by-byte initialization in `finalizeModule()`
- `memory.fill(heapu8, d, val, n)`: gets ArrayBuffer via `getBuffer()` + `getDataBlock()`, then `std::memset`
- `memory.copy(heapu8, d, s, n)`: `std::memmove` for correct overlapping region handling
- `memory.init(heapu8, dataSegs, segIdx, d, s, n)`: 6 args — `std::memcpy` from segment Uint8Array to memory
- `data.drop(dataSegs, segIdx)`: stores null at segment index in the array
- Per Wasm spec: `memory.init` with n=0 succeeds for dropped segments (bounds check against length 0)
- wabt callback signatures: `OnMemoryFillExpr(Index memidx)`, `OnMemoryCopyExpr(Index dest, Index src)`, `OnMemoryInitExpr(Index segment_index, Index memidx)`, `OnDataDropExpr(Index segment_index)`
- Bulk memory feature enabled by default in wabt — no `features.enable_bulk_memory()` needed
- Bytecode version now 111

## Bulk Table Operations (N.2) Patterns
- Element segments array: JS Array of interleaved JSArrays in `topLevelVS_`, variable `elemSegVar_`
- Each segment stored as `[func0, typeIdx0, func1, typeIdx1, ...]` — interleaved func/typeIdx pairs
- Lazy creation via `getOrCreateElemSegVar()` — same pattern as data segments
- Active and declarative segments set to null (dropped) in `finalizeModule()` after initialization
- `table.fill(funcsArr, idx, val, count)`: 4 args, fills funcs array only (no type info — Phase 1 limitation)
- `table.copy(dstFuncs, srcFuncs, dstTypes, srcTypes, dst, src, count)`: 7 args, copies both arrays, backward copy for overlapping same-table regions
- `table.init(funcsArr, typesArr, elemSegs, segIdx, dst, src, count)`: 7 args, reads interleaved pairs from segment
- `elem.drop(elemSegs, segIdx)`: stores null at segment index (same as `data.drop`)
- `JSArray::getLength()` requires `const JSArray*`, not `PinnedValue<JSArray>` — use `*lv.arr` to dereference
- `PinnedValue::getHermesValue().getRaw()` for identity comparison (same-table check in `table.copy`)
- wabt callback signatures: `OnTableFillExpr(Index table_index)`, `OnTableCopyExpr(Index dst, Index src)`, `OnTableInitExpr(Index segment_index, Index table_index)`, `OnElemDropExpr(Index segment_index)`
- `ref.func` and `ref.null` opcodes are currently unsupported — cannot test `table.fill` with inline refs
- Bytecode version now 112

## Unreachable Code Handling (O.2)
- `pop()`/`push()`/`popI64()`/`pushI64()`/`isTopI64()` all check `unreachable_` flag
- When unreachable: `pop` returns `LiteralUndefined` without modifying real stack; `push` is a no-op
- This prevents dead code from consuming live values or adding dead results to the value stack
- `onBlock()`/`onLoop()` create lightweight control entries when unreachable (no BasicBlocks/PhiInsts)
- `onEnd()` early-returns for all control kinds when `outerUnreachable` — pushes `LiteralUndefined` placeholders for result types
- `onElse()` sets `entry.branchTargeted = true` when then-block falls through (so merge block is reachable)
- `endFunction()` clears `unreachable_ = false` before return emission so `pop()` accesses the real stack
- Dead block cleanup in `endFunction()`: blocks without terminators are erased via `eraseFromParent()`

## Spec Test Runner (O.1)
- Spec testsuite at `external/wasm-testsuite/tests/` (257 .wast files; only 33 have lit wrappers in `test/wasm/spec/`)
- Driver: `test/wasm/spec/run-spec-test.py` — converts .wast→JSON+.wasm via wast2json, generates JS harness, runs with hermes
- Batch runner: `test/wasm/spec/run-all-spec-tests.py` — runs all tests with summary table
- Lit wrappers in `test/wasm/spec/<name>.wast` — one per spec test; passing tests use CHECK, failing use XFAIL
- wabt `Type::IsIndex()` returns true for multi-value block signatures (type index reference, not simple type)
- `convertBlockSigType()` handles both simple types and type index refs for block/loop/if/try
- `SwitchInst` requires non-empty case list — `br_table` with no targets (only default) must use BranchInst instead
- i32 spec comparison: use `(result|0) === (expected|0)` to normalize signed/unsigned representation
- f32/f64 values in wast2json: raw IEEE 754 bit patterns as unsigned decimal strings
- i64 values: full unsigned decimal strings (up to 2^64-1); Phase 1 only checks lo32
- `nan:canonical` and `nan:arithmetic` expected values → check `Number.isNaN()`
- `spectest` module provides default imports (functions, memory, table, global) for spec tests
- Crashes on invalid modules: `assert_invalid` should NOT try `new WebAssembly.Module()` — may crash on modules our validator considers valid but spec says invalid

## Unit Test Naming Convention
- `add_hermes_unittest` target names MUST end with `Tests` (plural) — lit's `GoogleTest(".", "Tests")` format uses this suffix to discover test binaries. `Test` (singular) silently skips them.

## Workflow
- Branch: `wasm`, PR target: `static_h`
- After completing a task: update `.ralph/progress.md` (status + context notes), then commit
- Always verify cmake configures cleanly after build system changes
- **Persist new findings to this file** (`.ralph/memory.md`) so future sessions can find them — e.g., gotchas, API quirks, patterns that worked or failed, build issues and their fixes
