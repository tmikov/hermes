# Wasm through shermes and the SH native backend — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Compile a `.wasm` module with `shermes` into a linkable native object whose JS-visible surface is an ordinary `WebAssembly.Module`.

**Architecture:** The Wasm frontend already produces IR the SH backend can compile; the difference is how the top level is *entered*. `WasmModuleData` gains an `SHUnitCreator` alongside its `BCProviderBase`, the two sites that execute the artifact branch on which is set, and a linked unit registers itself under its `--exported-unit` name so `WebAssembly.Module.fromNativeUnit(name)` can find it. One runtime dependency has to move: the data-segment blob travels in `SHUnit` instead of `RuntimeModule`.

**Tech Stack:** C++17 (no exceptions, no RTTI), CMake + Ninja, lit + FileCheck, gtest, wabt's `wat2wasm`.

**Spec:** `doc/superpowers/specs/2026-09-22-wasm-native-backend-design.md` — read it first.

**Plan revision 2.** Revision 1 was reviewed and found unexecutable: it guarded the new ABI behind a macro generated C never sees, emitted an undefined numeric conversion, called two APIs that do not exist, configured a build with the feature under test switched off, and wrote FileCheck patterns that could not match. All are corrected here. Where revision 1 said "adapt this from a neighbouring test", this revision gives the recipe.

## Global Constraints

- **Build with Wasm ON and with clang.** The feature under test defaults to `OFF` (`CMakeLists.txt:288`), and lit only adds `.wat` to its suffixes when it is on (`test/lit.cfg:143`) — so with it off the Wasm tests are not *collected*, and a green run means nothing:
  ```bash
  cmake -B cmake-build-asan -G Ninja -DCMAKE_BUILD_TYPE=Debug \
    -DHERMES_ENABLE_WASM=ON \
    -DHERMES_ENABLE_ADDRESS_SANITIZER=ON \
    -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ \
    -DCMAKE_CXX_FLAGS="-O1" -DCMAKE_C_FLAGS="-O1"
  ```
  Verify before starting: `grep HERMES_ENABLE_WASM cmake-build-asan/CMakeCache.txt` must show `ON`.
- **ABI is unconditional; implementations are guarded.** `HERMES_ENABLE_WASM` is an `add_definitions` for *repository* compilation (`CMakeLists.txt:550`). It is **not** in the configured public header `lib/config/libhermesvm-config.h.in`, and shermes does not pass it when it compiles generated C (`compile.cpp:238`). So generated C never sees it. Therefore:
  - New `SHUnit` **fields** and new **declarations** in `static_h.h` go in unconditionally. Guarding them would make the runtime and generated C disagree about `SHUnit`'s layout — a silent ABI split.
  - New `.cpp` **implementations** and the backend's **emission** of Wasm-specific code are guarded, because those are compiled inside the repository.
- **Never `cd`** out of the project root. Use `(cd dir; cmd)` in a subshell if unavoidable.
- **80-column lines, 2-space indent, C++17.** Doc comment on every declaration.
- **Copyright header** on every new file:
  ```cpp
  /**
   * Copyright (c) Meta Platforms, Inc. and affiliates.
   *
   * This source code is licensed under the MIT license found in the
   * LICENSE file in the root directory of this source tree.
   */
  ```
- **Do not bump `BYTECODE_VERSION`.** Versions bump at release time.
- **Run a named test and show it failing before implementing it.**
- **Single tests:** `LIT_FILTER='wasm/native-compile\.wat$' cmake --build cmake-build-asan --target check-hermes`. Lit matches the whole path, so anchor with `\.wat$` — a bare `$` after a stem matches nothing.
- **Do not commit** unless the task says to.

---

## Verified toolchain facts

Measured in this tree. Do not re-derive; if one proves false, stop and report.

- `shermes --exported-unit=NAME -c -o FILE.o INPUT` produces an object exporting `sh_export_NAME` (confirmed with `nm`: `T sh_export_testunit`).
- `shermes -o APP -Wc,FILE.o DRIVER.js` links that object in. `-Wc,` options are appended to the C compiler invocation (`compile.cpp:269`) ahead of the libraries.
- **A static constructor in such an object runs, before the driver's JS.** Measured: a `__attribute__((constructor))` in a `-Wc,`-linked object printed before the JS output. This is what makes self-registration work.
- A linked exported unit is **not** initialized by the generated `main`, which calls `_sh_initialize_units` on its own unit only. Its globals stay absent until something initializes it — for a Wasm unit, `fromNativeUnit`.
- `print` is available to driver JS in a shermes-built executable, via `shermes_console`.
- The model symbol ends in `_dbg` or `_rel`, appended in the configured header by `NDEBUG` (`libhermesvm-config.h.in`). A version component lands *before* that suffix.

---

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `lib/CMakeLists.txt` | `HERMESVM_MODEL` gains an SHUnit ABI version | 1, 3 |
| `unittests/VMRuntime/SHUnitABITest.cpp` | asserts the version is in the model | 1 |
| `tools/shermes/shermes.cpp` | `.wasm` input, `-wasm` flag, `wasmUnit` option | 2, 4 |
| `include/hermes/VM/static_h.h` | `SHUnit` blob fields; registry type; entry points | 3, 4 |
| `lib/VM/StaticHWasm.cpp` (new) | data-segment helper; the unit registry | 3, 4 |
| `lib/BCGen/SH/SH.cpp` | emit blob; lower the builtin; emit registration | 3, 4 |
| `include/hermes/Utils/Options.h` | `BytecodeGenerationOptions::wasmUnit` | 4 |
| `include/hermes/WasmFrontend/WasmModuleData.h` | the `unitCreator` alternative | 5 |
| `include/hermes/VM/PredefinedStrings.def` | `fromNativeUnit` | 5 |
| `lib/VM/JSLib/WebAssembly/WebAssembly.cpp` | adapter, both sites, check, `fromNativeUnit` | 5 |
| `test/wasm/native-*` | the e2e subset | 2-7 |

**Task order is a dependency order, not a preference.** Task 2 (the CLI) precedes Task 3 (data segments) because until shermes accepts a `.wasm` at all, no Wasm test can even run.

---

### Task 1: Make a stale generated object fail at link time

Adding fields to `SHUnit` is not safe by appending alone: generated C `calloc`s a `UnitData` embedding the layout it compiled against (`SH.cpp:3162`) while the runtime reads with its own. `_SH_MODEL` catches heap-model mismatches but carries no unit-layout version (`static_h.h:182`), so a mismatched pair links cleanly and corrupts at runtime. Add the version *before* the field that needs it.

**Files:**
- Modify: `lib/CMakeLists.txt` (after line 347, before `configure_file` at 349)
- Create: `unittests/VMRuntime/SHUnitABITest.cpp`
- Modify: `unittests/VMRuntime/CMakeLists.txt`

**Interfaces:**
- Produces: `HERMESVM_MODEL` contains `_u1`; the symbol is `_sh_model..._u1_dbg` (or `_rel`). Task 3 bumps to `_u2`.

- [ ] **Step 1: Write the failing test**

A gtest, not a lit test: the emitted C contains the literal macro call `_SH_MODEL();` (`SH.cpp:2807`), never an expanded name, so no FileCheck over `-emit-c` output can see the symbol.

Create `unittests/VMRuntime/SHUnitABITest.cpp`:

```cpp
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/VM/static_h.h"

#include "gtest/gtest.h"

#include <string>

namespace {

#define SHABI_STR_HELPER(x) #x
#define SHABI_STR(x) SHABI_STR_HELPER(x)

/// Generated C allocates a UnitData that embeds SHUnit by value, so any
/// change to the SHUnit layout makes an existing object file the wrong size.
/// The model symbol already encodes the heap configuration so that a
/// mismatched runtime fails at link time; it must encode the unit layout for
/// the same reason.
TEST(SHUnitABITest, ModelEncodesUnitLayoutVersion) {
  std::string model = SHABI_STR(HERMESVM_MODEL);
  EXPECT_NE(model.find("_u"), std::string::npos)
      << "HERMESVM_MODEL is \"" << model
      << "\", which carries no SHUnit layout version";
}

} // namespace
```

Add `SHUnitABITest.cpp` to the source list in `unittests/VMRuntime/CMakeLists.txt`, keeping alphabetical order.

- [ ] **Step 2: Run it and watch it fail**

```bash
cmake --build cmake-build-asan --target VMRuntimeTests
cmake-build-asan/unittests/VMRuntime/VMRuntimeTests --gtest_filter='SHUnitABITest.*'
```

Expected: FAIL, with the message printing the current model string (something like `_s22_p8_dbg`) and no `_u` in it.

- [ ] **Step 3: Add the version component**

In `lib/CMakeLists.txt`, after the `_p${CMAKE_SIZEOF_VOID_P}` append (line 347) and **before** `configure_file` (line 349):

```cmake
# SHUnit ABI version. Generated C allocates a UnitData that embeds SHUnit by
# value, so any change to the SHUnit layout makes previously generated object
# files the wrong size. Bump this whenever SHUnit's layout changes: a stale
# object then fails to link against the new runtime instead of reading past
# its own allocation.
string(APPEND HERMESVM_MODEL "_u1")
```

- [ ] **Step 4: Rebuild and confirm it passes**

```bash
cmake --build cmake-build-asan --target VMRuntimeTests
cmake-build-asan/unittests/VMRuntime/VMRuntimeTests --gtest_filter='SHUnitABITest.*'
```

Expected: PASS.

- [ ] **Step 5: Prove the guard actually guards**

The gtest proves the version is *present*; this proves it *bites*. Build a JS executable, then change `_u1` to `_u1x` in `lib/CMakeLists.txt`, rebuild **only shermes**, and recompile:

```bash
mkdir -p cmake-build-asan/abitest
cmake-build-asan/bin/shermes -o cmake-build-asan/abitest/app \
    unittests/VMRuntime/SHUnitABITest.cpp.js 2>/dev/null || true
# use any small JS file, e.g.:
echo "print('x');" > cmake-build-asan/abitest/t.js
cmake-build-asan/bin/shermes -o cmake-build-asan/abitest/app cmake-build-asan/abitest/t.js
# now edit lib/CMakeLists.txt: _u1 -> _u1x
cmake --build cmake-build-asan --target shermes
cmake-build-asan/bin/shermes -o cmake-build-asan/abitest/app2 cmake-build-asan/abitest/t.js
```

Expected: the second compile fails to link, naming an undefined `_sh_model..._u1x_dbg`. Revert to `_u1`, rebuild, and record the observed error text in the commit message.

- [ ] **Step 6: Full suite, then commit**

```bash
cmake --build cmake-build-asan --target check-hermes
git add lib/CMakeLists.txt unittests/VMRuntime/SHUnitABITest.cpp \
        unittests/VMRuntime/CMakeLists.txt
git commit -m "SH: version the SHUnit layout in the model symbol

Generated C allocates a UnitData embedding SHUnit by value, so a layout
change makes an existing object file the wrong size. The model symbol already
encodes the heap configuration for exactly this reason; it did not encode the
unit layout, so a stale object linked cleanly and read past its allocation.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX"
```

---

### Task 2: Accept a `.wasm` input in shermes

Must come before Task 3: until this lands, shermes parses `.wasm` bytes as JavaScript and no Wasm test can run at all.

**Files:**
- Modify: `tools/shermes/shermes.cpp`
- Test: `test/wasm/native-compile.wat` (create)

**Interfaces:**
- Produces: `shermes foo.wasm` and `shermes -wasm foo.bin` build a Module via `compileWasmModule` and run the normal optimization and SH pipeline. `--exported-unit` is mandatory for Wasm input.

- [ ] **Step 1: Write the failing test**

`test/wasm/native-compile.wat`:

```
;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; shermes compiles a .wasm the way hermesc does: the Wasm frontend populates
;; a Module, which then goes through the ordinary optimization and SH
;; pipeline. Nothing about the IR is backend-specific.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=addmod -emit-c -o - %t.wasm | %FileCheck %s
;; RUN: not %shermes -emit-c -o - %t.wasm 2>&1 | %FileCheck --check-prefix=NOUNIT %s
;; RUN: not %shermes -exported-unit=a -dump-ast -o - %t.wasm 2>&1 | %FileCheck --check-prefix=NOAST %s

(module
  (func (export "add") (param i32 i32) (result i32)
    (i32.add (local.get 0) (local.get 1))))

;; The Wasm top level is emitted, and the unit is named. Function bodies are
;; emitted BEFORE the CREATE_THIS_UNIT define (SH.cpp:3126 then 3148), so the
;; instantiate function must be checked first.
;; CHECK: __wasm_instantiate__
;; CHECK: #define CREATE_THIS_UNIT sh_export_addmod

;; Wasm input without --exported-unit is refused: the only supported output
;; is a linkable unit.
;; NOUNIT: --exported-unit

;; And an AST/sema-only output mode is refused rather than falling through to
;; a backend that asserts on it.
;; NOAST: not supported for WebAssembly
```

Note: no `--match-full-lines`. These are substring checks; full-line matching would require reproducing entire generated C lines exactly.

- [ ] **Step 2: Run it and watch it fail**

```bash
LIT_FILTER='wasm/native-compile\.wat$' cmake --build cmake-build-asan --target check-hermes
```

Expected: FAIL — shermes parses the `.wasm` bytes as JavaScript and reports a syntax error.

- [ ] **Step 3: Add the includes and the flag**

In `tools/shermes/shermes.cpp`, near the other includes:

```cpp
#ifdef HERMES_ENABLE_WASM
#include "hermes/WasmFrontend/WasmCompile.h"
#include "hermes/WasmFrontend/WasmModuleInfo.h"
#endif
```

Beside the other `cl::opt` declarations (near `ExportedUnit` at line 286):

```cpp
static cl::opt<bool> WasmMode(
    "wasm",
    cl::desc("The input is a WebAssembly binary module."),
    cl::init(false),
    cl::cat(CompilerCategory));
```

- [ ] **Step 4: Detect and validate the input**

In `compileFromCommandLineOptions`, after the `--exported-unit` validation block that ends at `shermes.cpp:931`:

```cpp
#ifdef HERMES_ENABLE_WASM
  // Auto-detect by extension, as CompilerDriver.cpp:1026 does, and accept an
  // explicit flag for inputs that are not named .wasm.
  bool wasmInput = cli::WasmMode ||
      (cli::InputFilenames.size() == 1 &&
       llvh::sys::path::extension(cli::InputFilenames[0]) == ".wasm");
  if (wasmInput) {
    if (cli::InputFilenames.size() != 1) {
      llvh::errs() << "Error: exactly one input file is required for Wasm\n";
      return false;
    }
    // A Wasm module's top level returns a module object whose instantiate()
    // needs an import object the embedder supplies, so a generated main has
    // nothing useful to do with it. The only supported output is a unit.
    if (cli::ExportedUnit.empty()) {
      llvh::errs() << "Error: Wasm input requires --exported-unit=NAME\n";
      return false;
    }
    // There is no AST and no sema for a Wasm module. shermesCompile asserts
    // that the output level is at least IR (compile.cpp:528), so these modes
    // must be rejected here rather than falling through to it.
    if (cli::OutputLevel.getNumOccurrences() &&
        cli::OutputLevel < OutputLevelKind::CFG) {
      llvh::errs()
          << "Error: this output mode is not supported for WebAssembly input\n";
      return false;
    }
  }
#else
  bool wasmInput = false;
#endif
```

- [ ] **Step 5: Replace the filling step only**

`compileFromCommandLineOptions` declares `Module M(context)` at `shermes.cpp:954`, fills it at 959-984, then verifies, optimizes, builds `genOptions` and calls `shermesCompile`. The Wasm branch replaces **only the filling**, so the optimizer at `shermes.cpp:1011` is reached by construction rather than by remembering to call it.

Wrap lines 959-984 — from `for (llvh::StringRef filename : cli::InputFilenames) {` through `generateIRFromESTree(&M, semCtx, flowContext, ast);` — as the `else` arm:

```cpp
  if (wasmInput) {
#ifdef HERMES_ENABLE_WASM
    std::unique_ptr<llvh::MemoryBuffer> fileBuf =
        memoryBufferFromFile(cli::InputFilenames[0], "input file", true);
    if (!fileBuf)
      return false;
    wasm::WasmModuleInfo moduleInfo;
    std::string errorMsg;
    if (!compileWasmModule(
            reinterpret_cast<const uint8_t *>(fileBuf->getBufferStart()),
            fileBuf->getBufferSize(),
            M,
            moduleInfo,
            errorMsg)) {
      llvh::errs() << "Error: " << errorMsg << '\n';
      return false;
    }
#endif
  } else {
    // ... existing lines 959-984, unchanged ...
  }
```

Leave the error-count check at `shermes.cpp:988` outside the branch; it applies to both arms. Note that the early `return true` for AST/sema output modes (line 980) stays inside the JS arm — Step 4 rejects those modes for Wasm, so nothing falls through.

- [ ] **Step 6: Run the test**

```bash
cmake --build cmake-build-asan --target shermes
LIT_FILTER='wasm/native-compile\.wat$' cmake --build cmake-build-asan --target check-hermes
```

Expected: PASS, all three RUN lines.

- [ ] **Step 7: Prove the optimizer is really reached**

Comparing `-emit-c` line counts does **not** establish this: `-O` also sets `genOptions.optimizationEnabled` (`shermes.cpp:1044`), which SH uses independently in lowering (`SH.cpp:2665`, `SH.cpp:2697`), so the C can differ even with the frontend optimizer bypassed. Compare IR instead, before any SH lowering:

```bash
cmake-build-asan/bin/wat2wasm test/wasm/native-compile.wat -o /tmp/claude-1001/-home-tmikov-work-hermes-wasm/d7ffdcee-5f57-46aa-b100-5267155814b0/scratchpad/nc.wasm
cmake-build-asan/bin/shermes -exported-unit=m -O0 -dump-ir -o - /tmp/.../nc.wasm > /tmp/.../o0.ir
cmake-build-asan/bin/shermes -exported-unit=m -O  -dump-ir -o - /tmp/.../nc.wasm > /tmp/.../oM.ir
diff /tmp/.../o0.ir /tmp/.../oM.ir
```

Expected: a non-empty diff. Then temporarily make the Wasm arm skip the optimizer (early-return to `shermesCompile` directly), re-run, and confirm the diff becomes empty. Restore. Record both observations.

- [ ] **Step 8: Full suite, then commit**

```bash
cmake --build cmake-build-asan --target check-hermes
git add tools/shermes/shermes.cpp test/wasm/native-compile.wat
git commit -m "shermes: compile a .wasm input to a linkable unit

Requires --exported-unit: a Wasm module's top level returns a module object
whose instantiate() needs an import object, so there is nothing a generated
main could usefully do with it. AST and sema output modes are rejected rather
than reaching a backend that asserts on them.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX"
```

---

### Task 3: Carry the data-segment blob in the unit

`wasmDataSegmentInit` (`HermesBuiltin.cpp:1286`) reaches the blob by walking the stack to the caller's `CodeBlock` and then its `RuntimeModule`. SH frames are `NativeJSFunction`s, so `getCalleeCodeBlock()` returns null and it raises "Cannot be called from native code". This is the only such dependency in the Wasm path. Precedent: `_sh_get_template_object`, whose comment at `SH.cpp:1716` states the same problem and solution.

**Files:**
- Modify: `include/hermes/VM/static_h.h` (fields + declaration, both **unconditional**)
- Modify: `lib/CMakeLists.txt` (`_u1` → `_u2`)
- Modify: `unittests/VMRuntime/SHUnitABITest.cpp` (no change needed; it checks `_u`)
- Create: `lib/VM/StaticHWasm.cpp`
- Modify: `lib/VM/CMakeLists.txt` (line 186 Wasm block)
- Modify: `lib/BCGen/SH/SH.cpp`
- Test: `test/wasm/native-data-segment-emit.wat` (create)

**Interfaces:**
- Consumes: Task 1's `_u1`; Task 2's `.wasm` input.
- Produces: `SHUnit::binary_data` (`const unsigned char *`), `SHUnit::binary_data_size` (`uint32_t`), and
  `void _sh_wasm_data_segment_init(SHRuntime *shr, SHUnit *unit, SHLegacyValue heapu8, uint32_t blobOffset, uint32_t length, uint32_t dest)`.

- [ ] **Step 1: Write the failing test**

`test/wasm/native-data-segment-emit.wat`:

```
;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A data segment's bytes must reach native code through the SHUnit, not
;; through the caller's RuntimeModule.
;;
;; wasmDataSegmentInit walks the stack to the caller's CodeBlock to find the
;; blob. An SH frame is a NativeJSFunction and has no CodeBlock, so that
;; builtin raises "Cannot be called from native code" and every module with a
;; nonempty data segment fails. The backend therefore emits the blob into the
;; unit and calls a unit-aware helper instead.
;;
;; This checks the emitted C. native-data-segment.wat checks the behaviour.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=dseg -emit-c -o - %t.wasm | %FileCheck %s

(module
  (memory 1)
  (data (i32.const 0) "hello")
  (func (export "first") (result i32)
    (i32.load8_u (i32.const 0))))

;; The segment copy goes through the unit-aware helper. Function bodies are
;; emitted before the unit buffers, so this comes first.
;; CHECK: _sh_wasm_data_segment_init(shr, shUnit,

;; The bytes are emitted as a unit-scope array, and the unit points at them.
;; CHECK: static const unsigned char s_binary_data[]
;; CHECK: .binary_data = s_binary_data
;; CHECK-SAME: .binary_data_size = 5
```

- [ ] **Step 2: Run it and watch it fail**

```bash
LIT_FILTER='wasm/native-data-segment-emit\.wat$' cmake --build cmake-build-asan --target check-hermes
```

Expected: FAIL at the first CHECK — no `_sh_wasm_data_segment_init` in the output.

- [ ] **Step 3: Add the fields, unconditionally**

In `include/hermes/VM/static_h.h`, in `SHUnit` immediately after `unit_name` (line 160) and before the `runtime_ext` comment (line 163). **No `#ifdef`** — see the Global Constraints:

```c
  /// Binary data blob: Wasm data-segment bytes concatenated in segment order.
  /// Null for units that have none, which is every unit not compiled from a
  /// Wasm module. Declared unconditionally because generated C does not see
  /// HERMES_ENABLE_WASM and must agree with the runtime about this layout.
  const unsigned char *binary_data;
  /// Size of binary_data in bytes.
  uint32_t binary_data_size;
```

- [ ] **Step 4: Bump the ABI version**

In `lib/CMakeLists.txt`, change the Task 1 line to:

```cmake
string(APPEND HERMESVM_MODEL "_u2")
```

- [ ] **Step 5: Declare the entry point, unconditionally**

In `include/hermes/VM/static_h.h`, with the other `_sh_` declarations. Again **no `#ifdef`**:

```c
/// Copy \p length bytes of \p unit's binary data, starting at \p blobOffset,
/// into the typed array \p heapu8 at offset \p dest. This is the SH
/// counterpart of the wasmDataSegmentInit builtin, which cannot be used from
/// native code because it locates the blob through the calling CodeBlock's
/// RuntimeModule and an SH frame has no CodeBlock.
///
/// Throws a JS error, via _sh_throw_current, if \p heapu8 is not an attached
/// typed array or if either range is out of bounds. Does nothing when
/// \p length is zero. Declared unconditionally so that generated C can call
/// it; defined only when Wasm is enabled.
SHERMES_EXPORT void _sh_wasm_data_segment_init(
    SHRuntime *shr,
    SHUnit *unit,
    SHLegacyValue heapu8,
    uint32_t blobOffset,
    uint32_t length,
    uint32_t dest);
```

- [ ] **Step 6: Implement it**

This is VM runtime code — **invoke the `gc-safe-coding` skill before writing it.**

Read `wasmDataSegmentInit` (`HermesBuiltin.cpp:1286-1334`) and `wasmTypedArrayArg` (`HermesBuiltin.cpp:534-545`) first. The helper must keep both checks the builtin makes and that `wasmTypedArrayArg` makes: `!arr`, `!arr->attached(runtime)`, and the byte-length minimum. `getDataBlock()` requires an attached buffer (`JSArrayBuffer.h:102`).

Create `lib/VM/StaticHWasm.cpp`:

```cpp
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/VM/JSTypedArray.h"
#include "hermes/VM/Runtime.h"
#include "hermes/VM/StaticHUtils.h"
#include "hermes/VM/static_h.h"

#include <cstring>

using namespace hermes;
using namespace hermes::vm;

extern "C" void _sh_wasm_data_segment_init(
    SHRuntime *shr,
    SHUnit *unit,
    SHLegacyValue heapu8,
    uint32_t blobOffset,
    uint32_t length,
    uint32_t dest) {
  Runtime &runtime = getRuntime(shr);

  // Validate the view before the zero-length short-circuit, so that a bad
  // view is rejected regardless of length -- matching wasmTypedArrayArg,
  // which the builtin calls before it checks length.
  auto *arr = dyn_vmcast<JSTypedArrayBase>(HermesValue::fromRaw(heapu8.raw));
  if (LLVM_UNLIKELY(!arr || !arr->attached(runtime))) {
    (void)runtime.raiseTypeError("Wasm memory view is not a typed array");
    _sh_throw_current(shr);
  }

  if (length == 0)
    return;

  if (LLVM_UNLIKELY(
          (uint64_t)blobOffset + length > unit->binary_data_size)) {
    (void)runtime.raiseError(
        "wasmDataSegmentInit: out of bounds binary data access");
    _sh_throw_current(shr);
  }

  if (LLVM_UNLIKELY((uint64_t)dest + length > arr->getLength())) {
    (void)runtime.raiseError(
        "wasmDataSegmentInit: out of bounds memory access");
    _sh_throw_current(shr);
  }

  // Nothing between here and the memcpy allocates, so the raw data pointers
  // cannot be invalidated by a GC.
  JSArrayBuffer *memBuf = arr->getBuffer(runtime);
  std::memcpy(
      memBuf->getDataBlock() + dest, unit->binary_data + blobOffset, length);
}
```

These are the real signatures: `attached(PointerBase &)` (`JSTypedArray.h:126`,
and `Runtime` derives from `PointerBase`, so `attached(runtime)` compiles),
`getLength()` (`:67`) and `getBuffer(Runtime &)` (`:84`).

- [ ] **Step 7: Add it to the build**

In `lib/VM/CMakeLists.txt`, inside the existing `if (HERMES_ENABLE_WASM)` block at line 186, add `StaticHWasm.cpp`. (Only the *definition* is guarded; the declaration in Step 5 is not.)

- [ ] **Step 8: Emit the blob**

In `lib/BCGen/SH/SH.cpp`, beside the other `s_*` buffer emissions and before the `struct UnitData` at line 3150:

```cpp
  // Wasm data-segment bytes. Emitted for every unit, empty for those with
  // none, so the UnitData initializer needs no special case. A zero-length
  // array is not valid ISO C, hence the one-element fallback.
  {
    auto binaryData = M->getBinaryDataStorage();
    OS << "static const unsigned char s_binary_data["
       << (binaryData.empty() ? 1 : binaryData.size()) << "] = {";
    if (binaryData.empty()) {
      OS << "0";
    } else {
      for (size_t i = 0, e = binaryData.size(); i < e; ++i) {
        if (i % 16 == 0)
          OS << "\n   ";
        OS << ' ' << (unsigned)binaryData[i] << ',';
      }
      OS << "\n";
    }
    OS << "};\n";
  }
```

and in the `UnitData` initializer, after `.unit_name = "sh_compiled"` (`SH.cpp:3184`):

```cpp
       << ", .binary_data = s_binary_data, .binary_data_size = "
       << M->getBinaryDataStorage().size()
```

- [ ] **Step 9: Lower the builtin to the helper**

In `generateCallBuiltinInst` (`SH.cpp:2115`), beside the existing `Math_sqrt` special case.

`CallBuiltinInst` argument 0 is the **`this` argument** (`Instrs.h:1350`); the four real arguments are at 1-4, matching `WasmHelpers::emitDataSegmentInit`'s `(heapu8, blobOffset, length, dest)`. `shUnit` is the in-scope variable name in generated functions — `_sh_get_template_object` is passed it at `SH.cpp:1718`.

Use `_sh_to_uint32_double` (`static_h.h:1480`), **not** a C cast: a direct `double`→`uint32_t` conversion is undefined for negative values, and a negative offset is reachable — an imported-memory segment bypasses the frontend's local-memory bounds checks and its original signed offset reaches the helper (`WasmIRGen.cpp:2120`, `WasmIRGen.cpp:2252`). The builtin converts through `truncateToInt32` for the same reason (`HermesBuiltin.cpp:1292`).

```cpp
#ifdef HERMES_ENABLE_WASM
    if (inst.getBuiltinIndex() ==
        BuiltinMethod::HermesBuiltin_wasmDataSegmentInit) {
      // The builtin finds the blob through the calling CodeBlock's
      // RuntimeModule, which an SH frame does not have.
      os_.indent(2);
      os_ << "_sh_wasm_data_segment_init(shr, shUnit, ";
      generateValue(*inst.getArgument(1));
      for (unsigned i = 2; i <= 4; ++i) {
        os_ << ", _sh_to_uint32_double(_sh_ljs_get_double(";
        generateValue(*inst.getArgument(i));
        os_ << "))";
      }
      os_ << ");\n";
      // The helper returns void; the instruction's result is undefined.
      os_.indent(2);
      generateRegister(inst);
      os_ << " = _sh_ljs_undefined();\n";
      return;
    }
#endif
```

- [ ] **Step 10: Run the test**

```bash
cmake --build cmake-build-asan --target shermes hermesvm
LIT_FILTER='wasm/native-data-segment-emit\.wat$' cmake --build cmake-build-asan --target check-hermes
```

Expected: PASS.

- [ ] **Step 11: Prove each half can fail**

Individually, reverting each:

1. Delete the `.binary_data = s_binary_data` clause (Step 8). Expected: the `.binary_data` CHECK fails.
2. Delete the `#ifdef HERMES_ENABLE_WASM` block (Step 9). Expected: the `_sh_wasm_data_segment_init` CHECK fails.

Record both messages.

- [ ] **Step 12: Full suite, then commit**

```bash
cmake --build cmake-build-asan --target check-hermes
git add include/hermes/VM/static_h.h lib/CMakeLists.txt lib/VM/StaticHWasm.cpp \
        lib/VM/CMakeLists.txt lib/BCGen/SH/SH.cpp \
        test/wasm/native-data-segment-emit.wat
git commit -m "Wasm: carry data-segment bytes in the SHUnit

The builtin locates the blob through the calling CodeBlock's RuntimeModule.
An SH frame is a NativeJSFunction and has no CodeBlock, so every module with
a nonempty data segment raised 'Cannot be called from native code'. The
backend now emits the bytes into the unit and calls a unit-aware helper, as
_sh_get_template_object already does for the same reason.

The fields and the declaration are unconditional: generated C is compiled
without HERMES_ENABLE_WASM, so guarding them would make it disagree with the
runtime about SHUnit's layout.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX"
```

---

### Task 4: The unit registry and self-registration

**Files:**
- Modify: `include/hermes/VM/static_h.h` (registry type and entry points, **unconditional**)
- Modify: `lib/VM/StaticHWasm.cpp`
- Modify: `include/hermes/Utils/Options.h`
- Modify: `tools/shermes/shermes.cpp` (line 1061-1063 area)
- Modify: `lib/BCGen/SH/SH.cpp` (before the `emitMain` block at line 3201)
- Test: `test/wasm/native-register-emit.wat`, `test/shermes/no-wasm-register.js` (create)

**Interfaces:**
- Produces: `SHWasmUnitReg`, `_sh_wasm_register_unit(SHWasmUnitReg *)`, `_sh_wasm_find_unit(const char *)`.

- [ ] **Step 1: Write the two failing tests**

`test/wasm/native-register-emit.wat`:

```
;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A Wasm unit registers itself under its --exported-unit name, so that
;; WebAssembly.Module.fromNativeUnit() can find it with no embedder code.
;;
;; The name comes from the exported-unit option, NOT from SHUnit::unit_name,
;; which the backend hardcodes to "sh_compiled" (SH.cpp:3184).

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=mymod -emit-c -o - %t.wasm | %FileCheck %s

(module
  (func (export "nop")))

;; CHECK: static SHWasmUnitReg s_wasm_reg
;; CHECK-SAME: "mymod"
;; CHECK-SAME: sh_export_mymod
;; CHECK: _sh_wasm_register_unit(&s_wasm_reg)
```

`test/shermes/no-wasm-register.js`:

```javascript
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// --exported-unit also serves ordinary JS units, which must NOT acquire a
// Wasm registration. Without this, every exported JS unit would advertise
// itself to WebAssembly.Module.fromNativeUnit().

// REQUIRES: shermes
// RUN: %shermes -exported-unit=plainjs -emit-c -o - %s | %FileCheck %s

print('hi');

// CHECK-NOT: _sh_wasm_register_unit
```

- [ ] **Step 2: Run both**

```bash
LIT_FILTER='wasm/native-register-emit\.wat$' cmake --build cmake-build-asan --target check-hermes
LIT_FILTER='shermes/no-wasm-register\.js$' cmake --build cmake-build-asan --target check-hermes
```

Expected: the first FAILS, the second PASSES vacuously. The second is a guard against the over-broad implementation and **must still pass** after Step 5.

- [ ] **Step 3: Declare the registry, unconditionally**

In `include/hermes/VM/static_h.h`, after the `SHUnitCreator` typedef (line 67). No `#ifdef` — generated C must see `SHWasmUnitReg`:

```c
/// One entry in the process-global registry of natively compiled Wasm units.
/// The node is allocated statically inside the unit's own object file, so
/// registration allocates nothing and can run from a static constructor.
typedef struct SHWasmUnitReg {
  /// The --exported-unit name. Not SHUnit::unit_name, which is a fixed
  /// string.
  const char *name;
  /// The unit creator, i.e. sh_export_<name>.
  SHUnitCreator creator;
  /// Next entry. Set by _sh_wasm_register_unit; initialize to NULL.
  struct SHWasmUnitReg *next;
} SHWasmUnitReg;

/// Add \p reg to the process-global Wasm unit registry. Safe from a static
/// constructor: the list head is zero-initialized before any constructor
/// runs. Aborts if another unit is already registered under the same name --
/// two independently linked shared libraries can collide where a single
/// static link cannot. The registry keeps \p reg, so the image it lives in
/// must stay loaded; there is no unregistration.
SHERMES_EXPORT void _sh_wasm_register_unit(SHWasmUnitReg *reg);

/// \return the creator registered under \p name, or NULL.
/// Lookup is not a static-initialization-time operation: a unit whose
/// constructor has not yet run will not be found. See the design document.
SHERMES_EXPORT SHUnitCreator _sh_wasm_find_unit(const char *name);
```

- [ ] **Step 4: Implement the registry**

Append to `lib/VM/StaticHWasm.cpp`, adding `#include <mutex>`, `#include <cstdio>`, `#include <cstdlib>`:

```cpp
namespace {
/// Head of the registry. In .bss, so NULL before any static constructor runs.
SHWasmUnitReg *s_wasmUnits = nullptr;

/// Guards insertion AND lookup: a dlopen on another thread can push while a
/// lookup walks the list. Function-local so it is initialized on first use,
/// which is valid from a static constructor.
std::mutex &wasmUnitsMutex() {
  static std::mutex m;
  return m;
}
} // namespace

extern "C" void _sh_wasm_register_unit(SHWasmUnitReg *reg) {
  std::lock_guard<std::mutex> lock(wasmUnitsMutex());
  for (SHWasmUnitReg *p = s_wasmUnits; p; p = p->next) {
    if (std::strcmp(p->name, reg->name) == 0) {
      fprintf(
          stderr,
          "SH: duplicate Wasm unit registration for \"%s\"\n",
          reg->name);
      abort();
    }
  }
  reg->next = s_wasmUnits;
  s_wasmUnits = reg;
}

extern "C" SHUnitCreator _sh_wasm_find_unit(const char *name) {
  // The lock is released before returning, so the caller runs the unit --
  // which re-enters the VM -- outside it.
  std::lock_guard<std::mutex> lock(wasmUnitsMutex());
  for (SHWasmUnitReg *p = s_wasmUnits; p; p = p->next) {
    if (std::strcmp(p->name, name) == 0)
      return p->creator;
  }
  return nullptr;
}
```

- [ ] **Step 5: Emit the registration, for Wasm units only**

In `include/hermes/Utils/Options.h`, after `emitMain`:

```cpp
  /// Whether this unit was compiled from a WebAssembly module, in which case
  /// the SH backend emits a self-registration block so that
  /// WebAssembly.Module.fromNativeUnit() can find it. --exported-unit also
  /// serves ordinary JS units, which must not be registered.
  bool wasmUnit = false;
```

In `tools/shermes/shermes.cpp`, beside the existing `genOptions.unitName` assignment at lines 1061-1063 (**not** in the frontend branch — `genOptions` is not declared until line 1043):

```cpp
  genOptions.wasmUnit = wasmInput;
```

In `lib/BCGen/SH/SH.cpp`, after the emitted accessor functions and **before** `if (options.emitMain) {` at line 3201:

```cpp
    if (options.wasmUnit) {
      assert(
          !options.emitMain &&
          "a Wasm unit is always an exported unit, never a main");
      OS << "\nstatic SHWasmUnitReg s_wasm_reg = {\"" << options.unitName
         << "\", sh_export_" << options.unitName << ", NULL};\n"
         << "__attribute__((constructor)) static void "
            "_sh_wasm_reg_ctor(void) {\n"
         << "  _sh_wasm_register_unit(&s_wasm_reg);\n"
         << "}\n";
    }
```

MSVC has no `__attribute__((constructor))`. Emit the `.CRT$XCU` form under `#ifdef _MSC_VER` in the generated C; if that needs more than a few lines, emit `#error` on MSVC and file a `dz` issue. Do not leave it silently broken.

- [ ] **Step 6: Run both tests**

```bash
cmake --build cmake-build-asan --target shermes hermesvm
LIT_FILTER='wasm/native-register-emit\.wat$' cmake --build cmake-build-asan --target check-hermes
LIT_FILTER='shermes/no-wasm-register\.js$' cmake --build cmake-build-asan --target check-hermes
```

Expected: both PASS. If the second now fails, `wasmUnit` is being set for JS units.

- [ ] **Step 7: Prove the duplicate check fires**

```bash
S=cmake-build-asan/regtest && mkdir -p $S
cat > $S/dup.c <<'EOF'
#include <hermes/VM/static_h.h>
static SHUnit *fake(void) { return 0; }
static SHWasmUnitReg a = {"dup", fake, 0};
static SHWasmUnitReg b = {"dup", fake, 0};
int main(void) {
  _sh_wasm_register_unit(&a);
  _sh_wasm_register_unit(&b);
  return 0;
}
EOF
clang $S/dup.c -Iinclude -Icmake-build-asan/lib -Lcmake-build-asan/lib \
    -lhermesvm -o $S/dup && $S/dup; echo "exit=$?"
```

Expected: `SH: duplicate Wasm unit registration for "dup"` and a non-zero exit. Adjust include/lib paths as the build requires. Delete `$S/dup*` afterwards — this is evidence, not a kept test.

- [ ] **Step 8: Full suite, then commit**

```bash
cmake --build cmake-build-asan --target check-hermes
git add include/hermes/VM/static_h.h lib/VM/StaticHWasm.cpp \
        include/hermes/Utils/Options.h tools/shermes/shermes.cpp \
        lib/BCGen/SH/SH.cpp test/wasm/native-register-emit.wat \
        test/shermes/no-wasm-register.js
git commit -m "Wasm: self-registering native units

A linked Wasm unit advertises itself under its --exported-unit name so JS can
reach it without embedder code. The node is statically allocated in the unit's
own object, so registration allocates nothing and runs from a constructor.

Registration requires the unit's object to be in the link. A JS name lookup
creates no reference to sh_export_<name>, so a unit inside a static archive is
never extracted; link the object directly, or force inclusion.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX"
```

---

### Task 5: Run a native unit from `WebAssembly.Module`

**Files:**
- Modify: `include/hermes/WasmFrontend/WasmModuleData.h`
- Modify: `include/hermes/VM/PredefinedStrings.def`
- Modify: `lib/VM/JSLib/WebAssembly/WebAssembly.cpp`
- Test: `test/wasm/native-e2e-add.wat` + `native-e2e-add-driver.js_` (create)

**Interfaces:**
- Consumes: `_sh_wasm_find_unit` (Task 4); a unit from Task 2.
- Produces: `WebAssembly.Module.fromNativeUnit(name)`.

- [ ] **Step 1: Write the failing e2e test**

`test/wasm/native-e2e-add-driver.js_`:

```javascript
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Driver for native-e2e-add.wat. The Wasm unit is linked into this
// executable and reached by name; no bytecode is involved anywhere. Note
// there is no hermescli.loadFile() preamble -- that is what the bytecode
// drivers need and this one does not.

var mod = WebAssembly.Module.fromNativeUnit('addmod');
var inst = new WebAssembly.Instance(mod, {});
print('add(2,40): ' + inst.exports.add(2, 40));

// Descriptors come from running the unit's top level, the same route the
// bytecode path uses.
print('exports: ' + WebAssembly.Module.exports(mod).map(function (e) {
  return e.name + ':' + e.kind;
}).join(','));

// A second instance must not alias the first. Isolation comes from
// __wasm_instantiate__ creating its own scope per call.
var inst2 = new WebAssembly.Instance(mod, {});
print('second instance distinct: ' + (inst2.exports.add !== inst.exports.add));

// An unknown name is a TypeError, not a crash.
try {
  WebAssembly.Module.fromNativeUnit('nosuchunit');
  print('MISSING THROW');
} catch (e) {
  print('unknown unit: ' + (e instanceof TypeError));
}

print('done');
```

`test/wasm/native-e2e-add.wat`:

```
;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A natively compiled Wasm unit is an ordinary WebAssembly.Module.
;;
;; The unit object is linked directly via -Wc,: a name lookup from JS creates
;; no reference to sh_export_addmod, so an archived unit would never be
;; extracted and never register.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=addmod -c -o %t-mod.o %t.wasm
;; RUN: %shermes -o %t.exe -Wc,%t-mod.o %S/native-e2e-add-driver.js_
;; RUN: %t.exe | %FileCheck --match-full-lines %s

(module
  (func (export "add") (param i32 i32) (result i32)
    (i32.add (local.get 0) (local.get 1))))

;; CHECK: add(2,40): 42
;; CHECK-NEXT: exports: add:function
;; CHECK-NEXT: second instance distinct: true
;; CHECK-NEXT: unknown unit: true
;; CHECK-NEXT: done
```

- [ ] **Step 2: Run it and watch it fail**

```bash
LIT_FILTER='wasm/native-e2e-add\.wat$' cmake --build cmake-build-asan --target check-hermes
```

Expected: FAIL with `fromNativeUnit is not a function`.

- [ ] **Step 3: Add the artifact alternative**

In `include/hermes/WasmFrontend/WasmModuleData.h`. `SHUnit` and `SHUnitCreator` are **global**, not in `namespace hermes` (`static_h.h:31`, `static_h.h:67`), so the forward declaration goes **before** `namespace hermes` opens (line 16) and the alias qualifies with `::`:

```cpp
// Mirrored rather than included: this struct is shared with the Wasm
// frontend and must not depend on VM headers. SHUnit is declared at global
// scope in static_h.h, so this declaration must be too.
struct SHUnit;

namespace hermes {

/// Creator for a natively compiled SH unit; matches SHUnitCreator.
using SHUnitCreatorFn = ::SHUnit *(*)();
```

and inside `WasmModuleData`:

```cpp
  /// Creator for a natively compiled unit, set instead of bytecodeProvider
  /// when the module came from a linked SH unit. Exactly one of the two is
  /// non-null in a fully compiled module.
  SHUnitCreatorFn unitCreator = nullptr;
```

No `reinterpret_cast` is needed anywhere: the alias is the same type as `SHUnitCreator`.

- [ ] **Step 4: Add the include and the adapter**

In `lib/VM/JSLib/WebAssembly/WebAssembly.cpp`, add the include — `getSHRuntime` lives in `StaticHUtils.h:21` and this file does not include it:

```cpp
#include "hermes/VM/StaticHUtils.h"
```

Then, above `createModuleFromBytes`. **Invoke `gc-safe-coding` first** — the returned value is an unrooted GC pointer:

```cpp
/// Run \p creator's unit main and return the module info object it produces,
/// converting an SH exception into ExecutionStatus::EXCEPTION.
///
/// The bare _sh_unit_init reaches sh_unit_run, which calls _legacyCall with
/// no guard installed; a throw from there reaches _sh_throw_current, which
/// aborts when no handler exists and otherwise longjmps past the live
/// LocalsRAII and GCScope objects in this file. The guarded form installs its
/// own jmpbuf and reports by return value instead.
static CallResult<HermesValue> runNativeWasmUnit(
    Runtime &runtime,
    SHUnitCreatorFn creator) {
  SHLegacyValue resOrExc;
  if (!_sh_unit_init_guarded(getSHRuntime(runtime), creator, &resOrExc)) {
    // _sh_catch returns _sh_get_clear_thrown_value, so the runtime no longer
    // holds the exception. Put it back for the CallResult caller.
    // setThrownValue returns ExecutionStatus (Runtime.h:676), so return it
    // rather than discarding it and writing the status out by hand.
    return runtime.setThrownValue(HermesValue::fromRaw(resOrExc.raw));
  }
  return HermesValue::fromRaw(resOrExc.raw);
}
```

`Runtime::setThrownValue` is declared at `Runtime.h:676` and returns `ExecutionStatus`.

- [ ] **Step 5: Extract the shared tail, with BOTH assignments**

Do this **before** editing the execution sites. Extract `createModuleFromBytes`'s tail — from the comment "Run the lightweight top-level to extract descriptors" (`WebAssembly.cpp:667`) through the end of the function — into:

```cpp
/// Run an already-selected artifact's top level and build the WasmModuleData
/// from the module info object it returns. Exactly one of \p bcProvider and
/// \p creator is non-null.
/// \param errorMsg [out] set on failure.
/// \return the populated data, or nullptr on failure.
static std::unique_ptr<WasmModuleData> buildModuleDataFromArtifact(
    Runtime &runtime,
    std::shared_ptr<hbc::BCProviderBase> bcProvider,
    SHUnitCreatorFn creator,
    std::string &errorMsg) {
  assert(
      (bool)bcProvider != (bool)creator &&
      "exactly one artifact must be supplied");

  CallResult<HermesValue> runRes = ExecutionStatus::EXCEPTION;
  if (creator) {
    runRes = runNativeWasmUnit(runtime, creator);
  } else {
    auto bcCopy = bcProvider;
    runRes = runtime.runBytecode(
        std::move(bcCopy),
        RuntimeModuleFlags{},
        "wasm-module",
        Runtime::makeNullHandle<Environment>());
  }
  if (LLVM_UNLIKELY(runRes == ExecutionStatus::EXCEPTION)) {
    errorMsg = "failed to run Wasm module top-level";
    return nullptr;
  }
  if (!runRes->isObject()) {
    errorMsg = "Wasm module top-level did not return an object";
    return nullptr;
  }

  auto moduleData = std::make_unique<WasmModuleData>();
  // Both assignments. The original code set only bytecodeProvider
  // (WebAssembly.cpp:685); lifting it verbatim would leave every native
  // module with neither artifact set.
  moduleData->bytecodeProvider = std::move(bcProvider);
  moduleData->unitCreator = creator;

  struct : public Locals {
    PinnedValue<JSObject> moduleInfoObj;
  } lv;
  LocalsRAII lraii(runtime, &lv);
  lv.moduleInfoObj.castAndSetHermesValue<JSObject>(*runRes);

  if (LLVM_UNLIKELY(
          extractDescriptorsFromModuleInfo(
              runtime, lv.moduleInfoObj, *moduleData) ==
          ExecutionStatus::EXCEPTION)) {
    errorMsg = "failed to extract descriptors from module info";
    return nullptr;
  }
  return moduleData;
}
```

Pin `*runRes` before anything that allocates — `make_unique` does not allocate on the JS heap, but `extractDescriptorsFromModuleInfo` does, which is why the `LocalsRAII` precedes it. Read the original tail and preserve whatever rooting it already had.

`createModuleFromBytes` then ends with:

```cpp
  return buildModuleDataFromArtifact(
      runtime, std::move(bcProvider), nullptr, errorMsg);
```

- [ ] **Step 6: Branch the instantiation site and fix the check**

At `WebAssembly.cpp:785`, the completeness check:

```cpp
  if (!moduleData->bytecodeProvider && !moduleData->unitCreator) {
    raiseLinkError(runtime, "module was not fully compiled");
    return ExecutionStatus::EXCEPTION;
  }
  assert(
      !(moduleData->bytecodeProvider && moduleData->unitCreator) &&
      "a module carries exactly one artifact");
```

And at `WebAssembly.cpp:812`, replace the `runBytecode` call with the same branch shape used in Step 5:

```cpp
  CallResult<HermesValue> runRes = ExecutionStatus::EXCEPTION;
  if (moduleData->unitCreator) {
    runRes = runNativeWasmUnit(runtime, moduleData->unitCreator);
  } else {
    auto bcProvider = moduleData->bytecodeProvider;
    runRes = runtime.runBytecode(
        std::move(bcProvider),
        RuntimeModuleFlags{},
        "wasm-module",
        Runtime::makeNullHandle<Environment>());
  }
```

- [ ] **Step 7: Add the predefined string and the method**

In `include/hermes/VM/PredefinedStrings.def`, beside the other WebAssembly entries near line 633:

```c
STR(fromNativeUnit, "fromNativeUnit")
```

In `lib/VM/JSLib/WebAssembly/WebAssembly.cpp`, the implementation. It is **not** config-gated: the unit was linked into the binary, a stronger authorization than the embedder-installed resolver that leaves `fromHermesURL` ungated.

`StringPrimitive::createStdString` does not exist. The working sequence, taken
from `wasmModuleFromHermesURL` (`WebAssembly.cpp:1111-1114`), is
`appendUTF16String` on the `PinnedValue<StringPrimitive>` into a
`SmallVector<char16_t, N>`, then `convertUTF16ToUTF8WithReplacements`.

```cpp
/// WebAssembly.Module.fromNativeUnit(name) -- look up a natively compiled
/// Wasm unit that registered itself under \p name when its object was linked
/// in.
///
/// Not gated on enableUntrustedBytecodeFromJS: script can name a linked unit
/// but cannot introduce one, which is stronger than the embedder-installed
/// resolver behind the ungated fromHermesURL.
static CallResult<HermesValue> wasmModuleFromNativeUnit(
    void *context,
    Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  struct : public Locals {
    PinnedValue<StringPrimitive> nameStr;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  auto nameRes = toString_RJS(runtime, args.getArgHandle(0));
  if (LLVM_UNLIKELY(nameRes == ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;
  lv.nameStr = std::move(*nameRes);

  // Exactly what fromHermesURL does (WebAssembly.cpp:1111-1114):
  // appendUTF16String on the string itself, then the UTF-8 conversion.
  llvh::SmallVector<char16_t, 32> u16;
  lv.nameStr->appendUTF16String(u16);
  std::string name;
  convertUTF16ToUTF8WithReplacements(name, u16);

  // A unit name cannot contain a NUL (isValidSHUnitName permits only
  // alphanumerics and underscore), so an embedded NUL can never match a real
  // unit. Reject rather than letting c_str() silently truncate.
  if (name.find('\0') != std::string::npos) {
    return runtime.raiseTypeError(
        "WebAssembly.Module.fromNativeUnit(): invalid unit name");
  }

  SHUnitCreatorFn creator = _sh_wasm_find_unit(name.c_str());
  if (!creator) {
    return runtime.raiseTypeError(
        TwineChar16("WebAssembly.Module.fromNativeUnit(): no unit named '") +
        TwineChar16(name.c_str()) + "'");
  }

  std::string errorMsg;
  auto moduleData =
      buildModuleDataFromArtifact(runtime, nullptr, creator, errorMsg);
  if (!moduleData)
    return runtime.raiseTypeError(TwineChar16(errorMsg.c_str()));
  // Wrap exactly as createAndBuildModule does; read WebAssembly.cpp:1010 and
  // mirror its JSWebAssemblyModule construction and error reporting.
  return wrapModuleData(runtime, std::move(moduleData));
}
```

`wrapModuleData` stands for whatever `createAndBuildModule` does after it obtains its `moduleData` — read that function and either call the same code or factor it out alongside `buildModuleDataFromArtifact`.

Register it next to `fromHermesBytecode` at `WebAssembly.cpp:3166` (the symbol is at 3169):

```cpp
  defineMethod(
      runtime,
      lv.moduleCons,
      Predefined::getSymbolID(Predefined::fromNativeUnit),
      nullptr,
      wasmModuleFromNativeUnit,
      1);
```

`lv.moduleCons` is the pinned local the surrounding code uses; copy the
neighbouring call at `WebAssembly.cpp:3166` verbatim and change only the
symbol and the function.

- [ ] **Step 8: Run the test**

```bash
cmake --build cmake-build-asan --target shermes hermesvm hermes
LIT_FILTER='wasm/native-e2e-add\.wat$' cmake --build cmake-build-asan --target check-hermes
```

Expected: PASS, all five CHECK lines.

- [ ] **Step 9: Prove the check and the branch can fail**

1. Revert Step 6's completeness check to `if (!moduleData->bytecodeProvider)`. Expected: FAIL with "module was not fully compiled". This is the check that would have rejected every native module.
2. Remove `moduleData->unitCreator = creator;` from Step 5. Expected: FAIL the same way.

Record both, then restore.

- [ ] **Step 10: Full suite, then commit**

```bash
cmake --build cmake-build-asan --target check-hermes
git add include/hermes/WasmFrontend/WasmModuleData.h \
        include/hermes/VM/PredefinedStrings.def \
        lib/VM/JSLib/WebAssembly/WebAssembly.cpp \
        test/wasm/native-e2e-add.wat test/wasm/native-e2e-add-driver.js_
git commit -m "Wasm: run a natively compiled unit from WebAssembly.Module

A module carries either a bytecode provider or a unit creator, and the two
sites that execute it branch on which. Everything downstream -- descriptors,
instantiate, imports, exports, tables, traps -- is unchanged, because both
artifacts produce the same module info object.

The unit is entered through _sh_unit_init_guarded. The bare form calls
_legacyCall with no handler installed, so a throw would abort or longjmp past
the live LocalsRAII and GCScope objects here.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX"
```

---

### Task 6: The e2e subset

Each test is given here rather than delegated to "adapt a neighbour": the bytecode drivers open files with `hermescli.getScriptArgs()` and `loadFile()` before looking up a module, and the native drivers have no such preamble. Replacing only the factory call leaves dead file I/O behind.

**Files:** create each `.wat` and its `-driver.js_` under `test/wasm/`.

**Interfaces:** consumes Tasks 1-5; produces no new API.

- [ ] **Step 1: The data-segment behaviour test — do this one first**

It is the only path with a measured blocker. `test/wasm/native-data-segment.wat`:

```
;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=dsegmod -c -o %t-mod.o %t.wasm
;; RUN: %shermes -o %t.exe -Wc,%t-mod.o %S/native-data-segment-driver.js_
;; RUN: %t.exe | %FileCheck --match-full-lines %s

(module
  (memory (export "mem") 1)
  (data (i32.const 0) "hello")
  (data (i32.const 16) "\00\ff\80")
  (func (export "at") (param i32) (result i32)
    (i32.load8_u (local.get 0))))

;; An ASCII segment and one with bytes above 0x7f: the rejected string-literal
;; encoding would have handled the first and doubled the second.
;; CHECK: at(0): 104
;; CHECK-NEXT: at(4): 111
;; CHECK-NEXT: at(17): 255
;; CHECK-NEXT: at(18): 128
;; CHECK-NEXT: done
```

`test/wasm/native-data-segment-driver.js_`:

```javascript
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

var e = new WebAssembly.Instance(
    WebAssembly.Module.fromNativeUnit('dsegmod'), {}).exports;
[0, 4, 17, 18].forEach(function (i) {
  print('at(' + i + '): ' + e.at(i));
});
print('done');
```

- [ ] **Step 2: Run it, then prove it catches the real defect**

```bash
LIT_FILTER='wasm/native-data-segment\.wat$' cmake --build cmake-build-asan --target check-hermes
```

Expected: PASS. Then temporarily revert Task 3 Step 9's lowering block and re-run: expected FAIL with "Cannot be called from native code". Restore, and record the message — this is the proof Task 3 fixed a real defect.

- [ ] **Step 3: A negative data-segment offset**

An imported-memory segment bypasses the frontend's local-memory bounds checks and its original signed offset reaches the helper (`WasmIRGen.cpp:2120`, `2252`), which is why Task 3 uses `_sh_to_uint32_double` rather than a C cast. Cover it.

`test/wasm/native-data-segment-neg.wat`:

```
;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=dnegmod -c -o %t-mod.o %t.wasm
;; RUN: %shermes -o %t.exe -Wc,%t-mod.o %S/native-data-segment-neg-driver.js_
;; RUN: %t.exe | %FileCheck --match-full-lines %s

(module
  (import "env" "mem" (memory 1))
  (data (i32.const -1) "x"))

;; A negative offset must produce a JS error, not undefined behaviour.
;; CHECK: threw: true
;; CHECK-NEXT: done
```

with a driver that instantiates with `{env: {mem: new WebAssembly.Memory({initial: 1})}}` inside a `try`, prints `'threw: ' + true` in the `catch` and `'threw: false'` if it does not throw, then `'done'`.

- [ ] **Step 4: The remaining four**

For each, write the `.wat` with Task 5's RUN-line shape and a driver whose **only** module acquisition is `WebAssembly.Module.fromNativeUnit('<unit>')`. Take the module body and the expected values from the listed source, and carry over its assertion logic — not its file-loading preamble.

| new test | module body from | note |
|---|---|---|
| `native-multivalue.wat` | `test/wasm/compile-retbuf-loads.wat` | that file is an **IR** test with no driver; write a driver that calls `outer()` and `outerIndirect()` and prints their results |
| `native-call-indirect.wat` | `test/wasm/e2e-call-indirect.wat` | its driver's first lines are `getScriptArgs`/`loadFile`; drop them |
| `native-try.wat` | `test/wasm/e2e-try-delegate.wat` | **needs `%wat2wasm --enable-exceptions`** (see `e2e-try-delegate.wat:33`); the module also has two `return_call` functions the existing test already omits |
| `native-bulk-memory.wat` | `test/wasm/e2e-bulk-memory.wat` | drop the loading preamble as above |

- [ ] **Step 5: The trap test**

`cli-run-trap.wat` is **not** a template: it has no JS driver and checks a failing process and stderr (`cli-run-trap.wat:20`), whereas these tests check stdout of a successful run. Write `test/wasm/native-trap.wat` fresh: a module with a `(func (export "boom") (unreachable))`, and a driver that calls it inside `try`/`catch` and prints whether it caught.

Its header comment must state what it does **not** cover: the trap is raised inside `__wasm_instantiate__`'s callee, reached via `Callable::executeCall1`, which is already guarded at `Callable.cpp:60-62`. It does not exercise `runNativeWasmUnit`.

- [ ] **Step 6: Run them all**

```bash
LIT_FILTER='wasm/native-' cmake --build cmake-build-asan --target check-hermes
```

Expected: all PASS. A failure here is a real backend divergence — investigate rather than adjust the test.

- [ ] **Step 7: Full suite, then commit**

```bash
cmake --build cmake-build-asan --target check-hermes
git add test/wasm/native-*
git commit -m "Wasm: end-to-end tests for the native backend

Modules are taken from the bytecode tests of the same name so a divergence
between backends shows up as a diff. The drivers are written fresh: the
bytecode drivers load a .hbc by path, which the native path does not do.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX"
```

---

### Task 7: Settle the adapter's test coverage

`runNativeWasmUnit` is reached only when `unit_main` itself throws. Task 6's trap test does not reach it. A gtest cannot call it either: Task 5 declares it `static` in `WebAssembly.cpp`, which has internal linkage and is compiled only under `HERMES_ENABLE_WASM`.

**Files:** either `test/wasm/native-unit-throws.*` plus a small C shim, or a `dz` issue.

- [ ] **Step 1: Try the one route that can work**

Compile a **JS** unit whose top level throws, register it under a Wasm unit name with a test-only C shim, and reach it through `fromNativeUnit`:

```bash
S=cmake-build-asan/regtest && mkdir -p $S
echo 'throw new Error("boom");' > $S/throwing.js
cmake-build-asan/bin/shermes --exported-unit=throwmod -c -o $S/throwmod.o $S/throwing.js
cat > $S/shim.c <<'EOF'
#include <hermes/VM/static_h.h>
extern SHUnit *sh_export_throwmod(void);
static SHWasmUnitReg reg = {"throwmod", sh_export_throwmod, 0};
__attribute__((constructor)) static void r(void) {
  _sh_wasm_register_unit(&reg);
}
EOF
```

Then a driver that calls `WebAssembly.Module.fromNativeUnit('throwmod')` inside `try`/`catch`, and link driver + `throwmod.o` + `shim.o` with `-Wc,`.

This reaches `runNativeWasmUnit` because the *unit main* throws, which is exactly the path under test. The module info object will not be a Wasm one, so expect the error from the unit, not a descriptor failure.

- [ ] **Step 2: Assert two things, not one**

That the exception surfaces as a catchable JS error, **and** that the runtime is still usable afterwards — a subsequent `fromNativeUnit` on a good unit must work. The second is what proves `setThrownValue` restored state rather than leaving the runtime wedged.

- [ ] **Step 3: If Step 1 does not work, record it honestly**

```bash
dz add --title "runNativeWasmUnit's exception path is untested" --json
```

Body: the adapter guards `unit_main`, which a well-formed Wasm module does not make throw; the alternative to guarding is `abort()`, so the adapter stays; and what was tried. Then amend the spec's verification section to say the adapter is reasoned but untested, citing the issue.

- [ ] **Step 4: Full suite, then commit**

```bash
cmake --build cmake-build-asan --target check-hermes
git add -A
git commit -m "Wasm: settle the native unit adapter's test coverage

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX"
```

---

### Task 8: `unreachable` in non-tail position must compile natively

**Added during execution, not part of the original plan.** Task 6 surfaced a real frontend defect
and its two affected tests are currently XFAIL. This task fixes the defect and un-XFAILs them.

**The defect.** `shermes` rejects any Wasm module with `unreachable` in a non-tail position:

```
$ cat u.wat
(module (func (export "f") (result i32) (unreachable) (i32.const 1)))
$ hermesc --wasm -emit-binary -out u.hbc u.wasm    # exit 0
$ shermes --exported-unit=u -c -o u.o u.wasm       # exit 1: "IRGen produced invalid IR"
```

The verifier says `PhiInst: NoType instruction has output in function "wasm_func_0" in %BB1, %4`.
The IR shows why — `createResultPhis` (`WasmIRGen.cpp:6358`) creates a result phi for the
continuation block when the control entry is set up, before it is known whether anything will
branch there. When the body ends in `unreachable` and no `br` targeted the label, that block ends
up with **no predecessors**, so its phi has **zero incoming operands**:

```
%BB0:  ...wasmTrap...
       UnreachableInst
%BB1:  %4 = PhiInst (:notype)     <- zero operands, no predecessors
       ReturnInst undefined
```

`hermesc --wasm` survives only because it does not run `verifyModule` before optimization; DCE
deletes `%BB1` first. `shermes` verifies pre-optimization (`shermes.cpp:993`) and rejects it.

This matters because `unreachable` in non-tail position is ubiquitous in real
Rust/C/C++-produced Wasm — every panic and abort path emits one.

**The fix has exact precedent.** `deleteUnreachableBasicBlocks(Function *)` already exists
(`include/hermes/IR/IRUtils.h:23`) and the JS frontend already calls it at the end of function
generation, immediately before `fixupCatchTargets` (`lib/IRGen/ESTreeIRGen-func.cpp:1496-1499`).
`WasmIRGen::endFunction()` calls `fixupCatchTargets(currentFunc_)` at `WasmIRGen.cpp:3556` but
never deletes unreachable blocks. The Wasm frontend is simply missing the other half.

**Files:**
- Modify: `lib/WasmIRGen/WasmIRGen.cpp` (`endFunction`, before line 3556)
- Modify: `test/wasm/native-trap.wat`, `test/wasm/native-try.wat` (remove `XFAIL`)
- Modify: `dz` issue `01a0d820-0df6` (close as fixed)
- Test: `test/wasm/native-unreachable-nontail.wat` + driver (create)

**Interfaces:** consumes Tasks 1-6. Produces no new API.

- [ ] **Step 1: Write the failing test**

`test/wasm/native-unreachable-nontail.wat`, with a driver that calls each export inside
`try`/`catch` and prints what happened. Cover three shapes, because they exercise different
control-flow paths:

```
(module
  ;; unreachable followed by dead code in the same block
  (func (export "tail") (result i32) (unreachable) (i32.const 1))
  ;; unreachable inside a block whose continuation nothing branches to
  (func (export "inBlock") (result i32)
    (block (result i32) (unreachable) (i32.const 2)))
  ;; a live path alongside the unreachable one, so the phi has one real operand
  (func (export "mixed") (param i32) (result i32)
    (block (result i32)
      (local.get 0)
      (if (result i32) (then (i32.const 7)) (else (unreachable) (i32.const 8))))))
```

Expected: `tail` and `inBlock` trap; `mixed(1)` returns 7. Write the CHECK lines accordingly.

- [ ] **Step 2: Run it and watch it fail**

```bash
LIT_FILTER='wasm/native-unreachable-nontail\.wat$' cmake --build cmake-build-asan --target check-hermes
```

Expected: FAIL at the `shermes -c` step with `IRGen produced invalid IR`. Capture the text.

- [ ] **Step 3: Add the missing call**

In `WasmIRGen::endFunction()`, immediately before `fixupCatchTargets(currentFunc_);`:

```cpp
  // Delete blocks left unreachable by `unreachable` in non-tail position.
  // createResultPhis() creates a continuation block's result phis when the
  // control entry is set up, before it is known whether anything will branch
  // there; if nothing does, those phis are left with zero operands, which the
  // verifier rejects as a NoType instruction with an output. ESTreeIRGen does
  // the same thing at the end of every function for the same reason.
  //
  // This must run BEFORE fixupCatchTargets, matching ESTreeIRGen-func.cpp,
  // so that catch targets are not assigned to blocks about to be deleted.
  deleteUnreachableBasicBlocks(currentFunc_);
```

Add `#include "hermes/IR/IRUtils.h"`.

- [ ] **Step 4: Confirm the test passes**

```bash
cmake --build cmake-build-asan --target shermes
LIT_FILTER='wasm/native-unreachable-nontail\.wat$' cmake --build cmake-build-asan --target check-hermes
```

- [ ] **Step 5: Un-XFAIL the two tests Task 6 marked**

Remove the `XFAIL: *` line and the dz pointer comment from `test/wasm/native-trap.wat` and
`test/wasm/native-try.wat`, then run them. Both must now pass as ordinary tests:

```bash
LIT_FILTER='wasm/native-' cmake --build cmake-build-asan --target check-hermes
```

Expected: 12 tests, 12 expected passes, 0 expected failures. If either still fails, the root cause
was not what this task assumed — STOP and report rather than re-XFAILing it.

- [ ] **Step 6: Prove the fix is what makes them pass**

Revert only Step 3's one line, rebuild `shermes`, and confirm all three of
`native-unreachable-nontail`, `native-trap` and `native-try` fail. Restore. Record the output.

- [ ] **Step 7: Confirm the bytecode path is unchanged**

`deleteUnreachableBasicBlocks` now runs for `hermesc --wasm` too, since both share `WasmIRGen`.
That should be a no-op-or-better, but verify:

```bash
LIT_FILTER='wasm/' cmake --build cmake-build-asan --target check-hermes
```

Report the counts. Then run the Wasm spec suite if it is cheap to do so, and say either way.

- [ ] **Step 8: Close the dz issue and commit**

```bash
dz close 01a0d820-0df6 --as fixed --json
cmake --build cmake-build-asan --target check-hermes
git add -A
git commit -m "Wasm: delete unreachable blocks before verifying

createResultPhis creates a continuation block's result phis when the control
entry is set up, before it is known whether anything will branch there. When
the body ends in \`unreachable\` and no br targeted the label, the block has no
predecessors and its phis have zero operands, which the verifier rejects.

hermesc survived this because it does not verify before optimization; DCE
deleted the block first. shermes verifies earlier, so every module with
\`unreachable\` in non-tail position failed to compile natively -- which is most
real Rust and C++ output. ESTreeIRGen already makes this call at the end of
every function; the Wasm frontend was missing it.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX"
```

---

## Out of scope

Per the spec: a JS unit consuming a Wasm module compiled in the same run; `new WebAssembly.Module(bytes)` inside a shermes-built binary; a standalone executable from a `.wasm`; and the full Wasm spec suite natively, which turns one process into hundreds of builds and is priced separately.
