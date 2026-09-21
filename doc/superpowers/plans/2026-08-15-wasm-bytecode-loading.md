# Wasm bytecode loading and trust gates — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop untrusted JS from ever causing `.hbc` execution while keeping precompiled `.hbc` the primary way to load a WebAssembly module.

**Architecture:** Two default-off `RuntimeConfig` gates (`EnableUntrustedBytecodeFromJS`, `EnableWasmBytecodeContentSniffing`). The `WebAssembly.Module`/`compile`/`instantiate` entry points stop content-sniffing by default (treat bytes as `.wasm`). Two new static factories — `WebAssembly.Module.fromHermesBytecode(bytes)` (gated) and `WebAssembly.Module.fromHermesURL(url)` (embedder-provided, trusted) — carry the explicit `.hbc` paths. The embedder provides trusted bytecode-by-URL through a JSI ICast interface that installs a resolver `std::function` onto `vm::Runtime`, which the VM builtin calls. The Worker fix is deferred (the vulnerable typed-array worker lives on `static_h`, not in this tree yet); the same gate is designed to cover it post-rebase.

**Tech Stack:** C++17, Hermes VM (`lib/VM`), JSI/Hermes API (`API/hermes`), LLVM `cl::opt` CLI flags, lit + FileCheck tests, googletest (`unittests/API`).

## Global Constraints

- Design spec: `doc/superpowers/specs/2026-08-15-wasm-bytecode-loading-design.md` (authoritative; every task traces to it).
- Both gates default `false`. Names verbatim: `EnableUntrustedBytecodeFromJS`, `EnableWasmBytecodeContentSniffing`.
- `.hbc` is trusted by design; never harden the bytecode loader.
- The URL/embedder route is `.hbc`-only, never `.wasm`, never sniffed, and NOT config-gated (authorized by the embedder providing bytes).
- `vm::Runtime` (in `lib/VM`) must not depend on `jsi::` types; the VM↔embedder bridge is a `std::function` taking/returning std C++ types.
- GC-safe coding rules apply to any `lib/VM` builtin (invoke the `gc-safe-coding` skill before writing VM builtins): `Locals`/`PinnedValue`, no raw pointers across safepoints.
- Build: `cmake --build cmake-build-asan --target hermes hermesc -j 14`.
- Wasm tests: `LIT_OPTS="-j8" LIT_FILTER="wasm|Wasm" cmake --build cmake-build-asan --target check-hermes -j 14`; run the full unfiltered suite before the final commit.
- Every commit builds and passes the suite. Commit messages end with the two trailers used on this branch (`Co-Authored-By:` and `Claude-Session:`).
- Adding predefined strings and Wasm builtins is bytecode-visible; note each in the review ledger's §4.1 list (handled in Task 8), not with a version bump here.

---

### Task 1: The two RuntimeConfig gates + CLI options

Adds the two flags end to end: config field → `vm::Runtime` bitfield → CLI `cl::opt` → wiring. No behavior change yet; this is the foundation every later task reads.

**Files:**
- Modify: `public/hermes/Public/RuntimeConfig.h` (RUNTIME_FIELDS macro, ~line 51)
- Modify: `include/hermes/VM/Runtime.h` (bitfield fields near `enableEval`, ~line 909; public accessors)
- Modify: `lib/VM/Runtime.cpp` (ctor init list, ~line 282)
- Modify: `include/hermes/VM/RuntimeFlags.h` (`cl::opt<bool>` in `struct VMOnlyRuntimeFlags`, near `ES6Proxy`/`Intl`, ~line 126)
- Modify: `lib/VM/RuntimeFlags.cpp` (`buildRuntimeConfig` `.withEnable…` chain, ~line 33 — used by test-runner/StaticHInit)
- Modify: `tools/hermes/hermes.cpp` (the **manual** `RuntimeConfig::Builder()` chain, ~line 120 — this is what the `%hermes` binary the lit tests use actually reads; `hermes.cpp` does NOT call `buildRuntimeConfig`)
- Test: none here (no observable effect until Tasks 2/3); the deliverable is verified by a clean build and `hermes --help` listing the two `-X…` flags.

**Interfaces:**
- Produces:
  - `RuntimeConfig::getEnableUntrustedBytecodeFromJS() -> bool` (default `false`)
  - `RuntimeConfig::getEnableWasmBytecodeContentSniffing() -> bool` (default `false`)
  - `vm::Runtime::enableUntrustedBytecodeFromJS` (public `const bool : 1`)
  - `vm::Runtime::enableWasmBytecodeContentSniffing` (public `const bool : 1`)
  - CLI flags `-Xenable-untrusted-bytecode-from-js`, `-Xenable-wasm-bytecode-content-sniffing`

- [ ] **Step 1: Add the two config fields**

In `public/hermes/Public/RuntimeConfig.h`, inside `RUNTIME_FIELDS(F)`, after the `EnableEval`/`OptimizedEval` group:

```c
  /* Whether JS APIs may load untrusted (JS-supplied) Hermes bytecode:      \
     WebAssembly.Module.fromHermesBytecode and a Worker bytecode script. */ \
  F(constexpr, bool, EnableUntrustedBytecodeFromJS, false)                  \
                                                                            \
  /* Whether WebAssembly.Module/compile/instantiate may content-sniff       \
     .hbc out of their bytes instead of always treating them as .wasm. */    \
  F(constexpr, bool, EnableWasmBytecodeContentSniffing, false)              \
```

- [ ] **Step 2: Add the `vm::Runtime` bitfields + accessors**

In `include/hermes/VM/Runtime.h`, next to `const bool enableEval : 1;` (~909):

```cpp
  /// Whether JS APIs may load untrusted (JS-supplied) Hermes bytecode.
  const bool enableUntrustedBytecodeFromJS : 1;
  /// Whether the WebAssembly entry points may content-sniff .hbc.
  const bool enableWasmBytecodeContentSniffing : 1;
```

These are read directly as `runtime.enableUntrustedBytecodeFromJS` (mirrors `runtime.test262`, already read in `WebAssembly.cpp`), so no separate getter is needed.

- [ ] **Step 3: Initialize them in the Runtime ctor**

In `lib/VM/Runtime.cpp`, in the member init list next to `enableEval(runtimeConfig.getEnableEval()),` (~282):

```cpp
      enableUntrustedBytecodeFromJS(
          runtimeConfig.getEnableUntrustedBytecodeFromJS()),
      enableWasmBytecodeContentSniffing(
          runtimeConfig.getEnableWasmBytecodeContentSniffing()),
```

- [ ] **Step 4: Add the CLI opts**

In `include/hermes/VM/RuntimeFlags.h`, inside `struct VMOnlyRuntimeFlags`, next to the `ES6Proxy`/`Intl` opts (~126):

```cpp
  llvh::cl::opt<bool> EnableUntrustedBytecodeFromJS{
      "Xenable-untrusted-bytecode-from-js",
      llvh::cl::desc(
          "Allow JS APIs to load untrusted Hermes bytecode "
          "(WebAssembly.Module.fromHermesBytecode, Worker bytecode)"),
      llvh::cl::init(
          vm::RuntimeConfig::getDefaultEnableUntrustedBytecodeFromJS()),
      llvh::cl::cat(RuntimeCategory)};

  llvh::cl::opt<bool> EnableWasmBytecodeContentSniffing{
      "Xenable-wasm-bytecode-content-sniffing",
      llvh::cl::desc(
          "Allow WebAssembly.Module/compile/instantiate to content-sniff "
          ".hbc bytecode instead of always treating input as .wasm"),
      llvh::cl::init(
          vm::RuntimeConfig::getDefaultEnableWasmBytecodeContentSniffing()),
      llvh::cl::cat(RuntimeCategory)};
```

- [ ] **Step 5: Wire the opts into the config**

In `lib/VM/RuntimeFlags.cpp`, in the `buildRuntimeConfig` `.withEnable…` chain next to `.withEnableEval(flags.EnableEval)` (~33):

```cpp
      .withEnableUntrustedBytecodeFromJS(flags.EnableUntrustedBytecodeFromJS)
      .withEnableWasmBytecodeContentSniffing(
          flags.EnableWasmBytecodeContentSniffing)
```

- [ ] **Step 6: Wire the opts into the `hermes` CLI config (CRITICAL — this is what `%hermes` reads)**

`tools/hermes/hermes.cpp` builds its `RuntimeConfig` by hand and does NOT call `buildRuntimeConfig`, so Step 5 alone leaves `%hermes -Xenable-untrusted-bytecode-from-js` unrecognized and Tasks 3/7 fail. In the `vm::RuntimeConfig::Builder()` chain (~line 120, next to `.withES6Proxy(flags.ES6Proxy)`), add:

```cpp
          .withEnableUntrustedBytecodeFromJS(
              flags.EnableUntrustedBytecodeFromJS)
          .withEnableWasmBytecodeContentSniffing(
              flags.EnableWasmBytecodeContentSniffing)
```

`flags` there is a `Flags : public cli::VMOnlyRuntimeFlags`, so the two opts added to `VMOnlyRuntimeFlags` in Step 4 are read as `flags.EnableUntrustedBytecodeFromJS` / `flags.EnableWasmBytecodeContentSniffing`.

- [ ] **Step 7: Build and verify the flags are recognized**

Run: `cmake --build cmake-build-asan --target hermes hermesc -j 14 && cmake-build-asan/bin/hermes --help 2>&1 | grep -E "enable-untrusted-bytecode-from-js|enable-wasm-bytecode-content-sniffing"`
Expected: clean build, both flags listed. (The generated `getDefault…`/`withEnable…`/`getEnable…` accessors come from the `_HERMES_CTORCONFIG_STRUCT` macro automatically.)

- [ ] **Step 8: Commit**

```bash
git add public/hermes/Public/RuntimeConfig.h include/hermes/VM/Runtime.h \
  lib/VM/Runtime.cpp include/hermes/VM/RuntimeFlags.h lib/VM/RuntimeFlags.cpp \
  tools/hermes/hermes.cpp
git commit -m "Add RuntimeConfig gates for JS bytecode loading and Wasm sniffing

Two default-off flags, EnableUntrustedBytecodeFromJS and
EnableWasmBytecodeContentSniffing, plumbed to vm::Runtime and exposed as
-Xenable-untrusted-bytecode-from-js / -Xenable-wasm-bytecode-content-sniffing.
No behavior change yet.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01Wt8qjqk6tfD8MVtfsbmebd"
```

---

### Task 2: Close and gate the content-sniff in the spec entries (§3.3)

Refactors `createModuleFromBytes` to take an explicit mode, so `WebAssembly.Module(bytes)`/`compile`/`instantiate` treat bytes as `.wasm` by default and only sniff/execute `.hbc` when both gates permit. This is the core security fix.

**Files:**
- Modify: `lib/VM/JSLib/WebAssembly/WebAssembly.cpp` (`createModuleFromBytes` ~579; call sites in `wasmCompile` ~689, `wasmInstantiate` ~890, `wasmModuleConstructor` ~963)
- Test: `test/wasm/e2e-sniff-gate.wat` + `test/wasm/e2e-sniff-gate-driver.js_` (new)

**Interfaces:**
- Produces: `enum class WasmBytesMode { SpecEntry, TrustedBytecode, UntrustedBytecode };` and the new signature
  `createModuleFromBytes(Runtime&, const uint8_t* data, size_t size, WasmBytesMode mode, std::string& errorMsg)`.
- Consumes: `runtime.enableWasmBytecodeContentSniffing`, `runtime.enableUntrustedBytecodeFromJS` (Task 1).

- [ ] **Step 1: Write the failing test**

`test/wasm/e2e-sniff-gate.wat`:

```
;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; By default WebAssembly.Module treats its bytes as .wasm; handed a .hbc
;; image it raises a CompileError instead of executing it as bytecode.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-sniff-gate-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module (func (export "f") (result i32) (i32.const 7)))

;; CHECK: hbc bytes to WebAssembly.Module (default): CompileError
;; CHECK-NEXT: done
```

`test/wasm/e2e-sniff-gate-driver.js_`:

```js
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var hbc = hermescli.loadFile(hermescli.getScriptArgs()[0]);
try {
  new WebAssembly.Module(hbc);
  print('hbc bytes to WebAssembly.Module (default): loaded (WRONG)');
} catch (e) {
  print('hbc bytes to WebAssembly.Module (default): ' + e.name);
}
print('done');
```

- [ ] **Step 2: Run it to verify it fails**

Run: `LIT_OPTS="-j8" LIT_FILTER="e2e-sniff-gate" cmake --build cmake-build-asan --target check-hermes -j 14`
Expected: FAIL — today the `.hbc` bytes are sniffed and executed, so the driver prints `loaded (WRONG)`.

- [ ] **Step 3: Introduce the mode enum and refactor the function**

In `WebAssembly.cpp`, above `createModuleFromBytes`:

```cpp
/// How a byte buffer handed to a WebAssembly entry point is interpreted.
enum class WasmBytesMode {
  /// Spec entry (Module/compile/instantiate). Treat as .wasm unless the
  /// content-sniffing gate is on AND the bytes are .hbc, which additionally
  /// requires the untrusted-bytecode gate; a detected-but-ungated .hbc is
  /// refused with a CompileError.
  SpecEntry,
  /// Trusted bytecode from the embedder (fromHermesURL). Always loaded as
  /// bytecode; never sniffed; not gated.
  TrustedBytecode,
  /// Explicit untrusted bytecode from JS (fromHermesBytecode). Always loaded
  /// as bytecode; the caller has already checked the untrusted gate.
  UntrustedBytecode,
};
```

Replace the head of `createModuleFromBytes` (the `isBytecodeStream` branch) with a mode-driven decision. New signature and prologue:

```cpp
static std::unique_ptr<WasmModuleData> createModuleFromBytes(
    Runtime &runtime,
    const uint8_t *data,
    size_t size,
    WasmBytesMode mode,
    std::string &errorMsg) {
  std::shared_ptr<hbc::BCProviderBase> bcProvider;

  bool loadAsBytecode;
  switch (mode) {
    case WasmBytesMode::TrustedBytecode:
    case WasmBytesMode::UntrustedBytecode:
      loadAsBytecode = true;
      break;
    case WasmBytesMode::SpecEntry:
      // Only ever treat spec-entry bytes as bytecode when the embedder has
      // explicitly opted into BOTH sniffing and untrusted bytecode; otherwise
      // the bytes are .wasm (or a refused .hbc). This is the §3.3 fix.
      if (runtime.enableWasmBytecodeContentSniffing &&
          hbc::BCProviderFromBuffer::isBytecodeStream(
              llvh::ArrayRef<uint8_t>(data, size))) {
        if (!runtime.enableUntrustedBytecodeFromJS) {
          errorMsg =
              "refusing to load Hermes bytecode: untrusted bytecode from JS "
              "is disabled";
          return nullptr;
        }
        loadAsBytecode = true;
      } else {
        loadAsBytecode = false;
      }
      break;
  }

  if (loadAsBytecode) {
    auto llvmBuf = llvh::MemoryBuffer::getMemBufferCopy(
        llvh::StringRef(reinterpret_cast<const char *>(data), size));
    auto ret = hbc::BCProviderFromBuffer::createBCProviderFromBuffer(
        std::make_unique<OwnedMemoryBuffer>(std::move(llvmBuf)));
    if (!ret.first) {
      errorMsg = ret.second.empty()
          ? "invalid HBC bytecode" : std::string(ret.second);
      return nullptr;
    }
    bcProvider = std::shared_ptr<hbc::BCProviderBase>(std::move(ret.first));
  } else {
    auto compiledData = hermes::compileWasmToModuleData(
        data, size, errorMsg, runtime.test262);
    if (!compiledData) {
      return nullptr;
    }
    bcProvider = compiledData->bytecodeProvider;
  }

  // ... unchanged: runBytecode + descriptor extraction ...
```

(Keep the rest of the function — `runBytecode`, descriptor extraction — exactly as-is.)

- [ ] **Step 4: Update the three spec call sites to pass `SpecEntry`**

At each of `wasmCompile`, `wasmInstantiate`, `wasmModuleConstructor`, change
`createModuleFromBytes(runtime, data, size, errorMsg)` to
`createModuleFromBytes(runtime, data, size, WasmBytesMode::SpecEntry, errorMsg)`.

- [ ] **Step 5: Build and run the test to verify it passes**

Run: `cmake --build cmake-build-asan --target hermes hermesc -j 14 && LIT_OPTS="-j8" LIT_FILTER="e2e-sniff-gate" cmake --build cmake-build-asan --target check-hermes -j 14`
Expected: PASS — the driver now prints `CompileError`.

- [ ] **Step 6: Prove the check can fail (mutation)**

Temporarily change `SpecEntry`'s condition to `if (true && …isBytecodeStream…)` (ignore the sniffing gate). Rebuild, rerun the test: it must FAIL (`loaded (WRONG)`). Revert.

- [ ] **Step 7: Commit**

```bash
git add lib/VM/JSLib/WebAssembly/WebAssembly.cpp test/wasm/e2e-sniff-gate.wat \
  test/wasm/e2e-sniff-gate-driver.js_
git commit -m "Stop WebAssembly entry points from content-sniffing .hbc by default

createModuleFromBytes takes an explicit WasmBytesMode. The spec entries
(Module/compile/instantiate) now treat bytes as .wasm unless BOTH
EnableWasmBytecodeContentSniffing and EnableUntrustedBytecodeFromJS are set;
a detected-but-ungated .hbc is refused with a CompileError. Closes review
finding 3.3.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01Wt8qjqk6tfD8MVtfsbmebd"
```

---

### Task 3: `WebAssembly.Module.fromHermesBytecode(bytes)`

The explicit, gated, no-sniff bytecode entry.

**Files:**
- Modify: `include/hermes/VM/PredefinedStrings.def` (add `STR(fromHermesBytecode, "fromHermesBytecode")` near `STR(exports…)`, ~604)
- Modify: `lib/VM/JSLib/WebAssembly/WebAssembly.cpp` (new `wasmModuleFromHermesBytecode`; register it as a static method on `moduleCons`, next to `Module.exports` ~2617)
- Test: `test/wasm/e2e-from-hermes-bytecode.wat` + driver (new)

**Interfaces:**
- Consumes: `WasmBytesMode::UntrustedBytecode` (Task 2), `extractBufferSourceBytes` and `finalizeWasmModuleData`/module-object construction helpers already used by `wasmModuleConstructor`.
- Produces: JS `WebAssembly.Module.fromHermesBytecode(bytes) -> Module`.

- [ ] **Step 1: Write the failing test**

`test/wasm/e2e-from-hermes-bytecode.wat`:

```
;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; WebAssembly.Module.fromHermesBytecode loads caller-supplied .hbc, but only
;; when EnableUntrustedBytecodeFromJS is set; otherwise it throws.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-from-hermes-bytecode-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s
;; RUN: %hermes -Xhermes-internal-test-methods %S/e2e-from-hermes-bytecode-driver.js_ -- %t.hbc | %FileCheck --check-prefix=OFF --match-full-lines %s

(module (func (export "f") (result i32) (i32.const 7)))

;; CHECK: gated on: f() = 7
;; OFF: gated off: TypeError
```

`test/wasm/e2e-from-hermes-bytecode-driver.js_`:

```js
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var hbc = hermescli.loadFile(hermescli.getScriptArgs()[0]);
try {
  var mod = WebAssembly.Module.fromHermesBytecode(hbc);
  var inst = new WebAssembly.Instance(mod);
  print('gated on: f() = ' + inst.exports.f());
} catch (e) {
  print('gated off: ' + e.name);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `LIT_OPTS="-j8" LIT_FILTER="e2e-from-hermes-bytecode" cmake --build cmake-build-asan --target check-hermes -j 14`
Expected: FAIL — `fromHermesBytecode` does not exist (`TypeError` on both runs).

- [ ] **Step 3: Add the predefined string**

In `include/hermes/VM/PredefinedStrings.def`, near `STR(exports, "exports")`:

```c
STR(fromHermesBytecode, "fromHermesBytecode")
```

- [ ] **Step 4: Implement the builtin**

Invoke the `gc-safe-coding` skill first. Model the body on `wasmModuleConstructor`: read the buffer with the existing `extractBufferSourceBytes` helper, gate, call `createModuleFromBytes(..., WasmBytesMode::UntrustedBytecode, ...)`, then build the Module object exactly as the constructor does (factor the shared tail into a helper if the constructor's is not already reusable). Skeleton:

```cpp
/// WebAssembly.Module.fromHermesBytecode(bytes) -> Module.
/// Loads caller-supplied Hermes bytecode explicitly (no sniffing). Gated by
/// EnableUntrustedBytecodeFromJS: these bytes are untrusted JS input.
static CallResult<HermesValue>
wasmModuleFromHermesBytecode(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  if (!runtime.enableUntrustedBytecodeFromJS) {
    return runtime.raiseTypeError(
        "WebAssembly.Module.fromHermesBytecode is disabled "
        "(EnableUntrustedBytecodeFromJS is off)");
  }
  // ... extract bytes (extractBufferSourceBytes), then:
  std::string errorMsg;
  auto moduleData = createModuleFromBytes(
      runtime, data, size, WasmBytesMode::UntrustedBytecode, errorMsg);
  if (!moduleData) {
    // raise CompileError with errorMsg, as wasmModuleConstructor does
  }
  // ... build and return the Module object, identical to wasmModuleConstructor
}
```

- [ ] **Step 5: Register it as a static method on the Module constructor**

In `createWebAssemblyObject`, next to the `Module.exports`/`Module.imports` `defineMethod` calls (~2617):

```cpp
  defineMethod(
      runtime,
      lv.moduleCons,
      Predefined::getSymbolID(Predefined::fromHermesBytecode),
      nullptr,
      wasmModuleFromHermesBytecode,
      1);
```

- [ ] **Step 6: Build and run the test to verify it passes**

Run: `cmake --build cmake-build-asan --target hermes hermesc -j 14 && LIT_OPTS="-j8" LIT_FILTER="e2e-from-hermes-bytecode" cmake --build cmake-build-asan --target check-hermes -j 14`
Expected: PASS — both the gated-on (`f() = 7`) and gated-off (`TypeError`) runs.

- [ ] **Step 7: Prove the gate (mutation)**

Temporarily delete the `if (!runtime.enableUntrustedBytecodeFromJS)` guard; rebuild; the `OFF` run must FAIL (it would load instead of throwing). Revert.

- [ ] **Step 8: Commit**

```bash
git add include/hermes/VM/PredefinedStrings.def \
  lib/VM/JSLib/WebAssembly/WebAssembly.cpp \
  test/wasm/e2e-from-hermes-bytecode.wat \
  test/wasm/e2e-from-hermes-bytecode-driver.js_
git commit -m "Add WebAssembly.Module.fromHermesBytecode, gated

Explicit, no-sniff bytecode load from caller-supplied bytes, gated by
EnableUntrustedBytecodeFromJS.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01Wt8qjqk6tfD8MVtfsbmebd"
```

---

### Task 4: VM resolver hook + `WebAssembly.Module.fromHermesURL(url)`

Adds the `vm::Runtime`-level bridge the embedder installs into, and the JS factory that calls it. The embedder side that populates the hook is Task 5; this task tests the VM path with a directly-installed hook via a gtest so it is independently verifiable.

**Test vehicle note:** the *positive* path (resolver/registry returns bytes → Module loads) cannot be exercised from a lit/JS driver, because installing a resolver is a native action — it is tested end-to-end in Task 5's APITest. This task independently tests the *negative* path from lit (`fromHermesURL` exists as a function; with no resolver installed it throws), which needs no native setup.

**Files:**
- Modify: `include/hermes/VM/Runtime.h` (a `std::function` member + setter, near the other callback setters ~236)
- Modify: `lib/VM/Runtime.cpp` (define the setter if not header-inline)
- Modify: `lib/VM/JSLib/WebAssembly/WebAssembly.cpp` (`wasmModuleFromHermesURL`; register as a static method on `moduleCons`)
- Modify: `include/hermes/VM/PredefinedStrings.def` (`STR(fromHermesURL, "fromHermesURL")`)
- Test: `test/wasm/e2e-from-hermes-url-noresolver.wat` + driver (new) — negative path only

**Interfaces:**
- Produces:
  - `using WasmModuleResolver = std::function<bool(const std::string& url, std::string& bytecodeOut)>;` (returns `true` and fills `bytecodeOut` with a COPY of the `.hbc` bytes; `false` = not found).
  - `void Runtime::setWasmModuleResolver(WasmModuleResolver)` / `const WasmModuleResolver& Runtime::getWasmModuleResolver() const`.
  - JS `WebAssembly.Module.fromHermesURL(url) -> Module`.
- Consumes: `WasmBytesMode::TrustedBytecode` (Task 2).

- [ ] **Step 1: Write the failing lit test (negative path)**

`test/wasm/e2e-from-hermes-url-noresolver.wat` + driver asserting `typeof WebAssembly.Module.fromHermesURL === 'function'` and that `WebAssembly.Module.fromHermesURL('app://x')` throws a `TypeError` when no resolver is installed:

```
;; REQUIRES: wasm
;; RUN: %hermes -Xhermes-internal-test-methods %S/e2e-from-hermes-url-noresolver-driver.js_ | %FileCheck --match-full-lines %s
;; CHECK: fromHermesURL is function: true
;; CHECK-NEXT: no resolver: TypeError
;; CHECK-NEXT: done
```
Driver: `print('fromHermesURL is function: ' + (typeof WebAssembly.Module.fromHermesURL === 'function')); try { WebAssembly.Module.fromHermesURL('app://x'); print('no resolver: loaded (WRONG)'); } catch (e) { print('no resolver: ' + e.name); } print('done');`

- [ ] **Step 2: Run it to verify it fails**

Run: `LIT_OPTS="-j8" LIT_FILTER="e2e-from-hermes-url-noresolver" cmake --build cmake-build-asan --target check-hermes -j 14`
Expected: FAIL — `fromHermesURL` does not exist (`typeof … === 'undefined'`).

- [ ] **Step 3: Add the resolver hook to `vm::Runtime`**

In `include/hermes/VM/Runtime.h`, near the other `std::function` callback setters:

```cpp
  /// Resolves a Wasm module URL to trusted Hermes bytecode. Installed by the
  /// embedder (via the API layer). Returns true and fills \p bytecodeOut with
  /// a COPY of the .hbc bytes, or false if the URL is not provided. The VM
  /// never depends on jsi types; the API layer adapts its provider to this.
  using WasmModuleResolver =
      std::function<bool(const std::string &url, std::string &bytecodeOut)>;
  void setWasmModuleResolver(WasmModuleResolver resolver) {
    wasmModuleResolver_ = std::move(resolver);
  }
  const WasmModuleResolver &getWasmModuleResolver() const {
    return wasmModuleResolver_;
  }
```
and the member: `WasmModuleResolver wasmModuleResolver_;` in the private data.

- [ ] **Step 4: Add the predefined string + the builtin**

`STR(fromHermesURL, "fromHermesURL")` in `PredefinedStrings.def`. Then in `WebAssembly.cpp` (invoke `gc-safe-coding` first):

```cpp
/// WebAssembly.Module.fromHermesURL(url) -> Module.
/// Resolves url to trusted Hermes bytecode via the embedder-installed
/// resolver and loads it. Never sniffs, never compiles .wasm, not config-gated
/// (authorized by the resolver being installed and providing bytes).
static CallResult<HermesValue>
wasmModuleFromHermesURL(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  // Coerce arg0 to string (spec-style ToString); reject non-string early.
  // ... get std::string url ...
  const auto &resolver = runtime.getWasmModuleResolver();
  std::string bytecode;
  if (!resolver || !resolver(url, bytecode)) {
    return runtime.raiseTypeError(
        "WebAssembly.Module.fromHermesURL: no module for URL");
  }
  std::string errorMsg;
  auto moduleData = createModuleFromBytes(
      runtime,
      reinterpret_cast<const uint8_t *>(bytecode.data()),
      bytecode.size(),
      WasmBytesMode::TrustedBytecode,
      errorMsg);
  if (!moduleData) {
    // raise CompileError with errorMsg
  }
  // ... build and return the Module object (shared tail with the constructor)
}
```
Register it on `moduleCons` with `defineMethod(... Predefined::fromHermesURL ... wasmModuleFromHermesURL, 1)`.

- [ ] **Step 5: Build and run the lit test to verify it passes**

Run: `cmake --build cmake-build-asan --target hermes hermesc -j 14 && LIT_OPTS="-j8" LIT_FILTER="e2e-from-hermes-url-noresolver" cmake --build cmake-build-asan --target check-hermes -j 14`
Expected: PASS — `fromHermesURL` is a function and throws `TypeError` with no resolver. (The positive path — a resolver/registry returning bytes → a working Module — is covered by Task 5's APITest, which is the only vehicle that can install a provider.)

- [ ] **Step 6: Prove the negative check can fail (mutation)** — make the `!resolver` branch build from an empty buffer instead of throwing; the driver prints `loaded (WRONG)` and the test FAILs. Revert.

- [ ] **Step 7: Commit**

```bash
git add include/hermes/VM/Runtime.h lib/VM/Runtime.cpp \
  include/hermes/VM/PredefinedStrings.def \
  lib/VM/JSLib/WebAssembly/WebAssembly.cpp \
  test/wasm/e2e-from-hermes-url-noresolver.wat \
  test/wasm/e2e-from-hermes-url-noresolver-driver.js_
git commit -m "Add WebAssembly.Module.fromHermesURL and the VM resolver hook

vm::Runtime carries a WasmModuleResolver std::function the embedder installs;
fromHermesURL calls it, loads the returned trusted bytecode (no sniff, not
gated). VM stays free of jsi types.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01Wt8qjqk6tfD8MVtfsbmebd"
```

---

### Task 5: Embedder facility — `IWasmModuleProvider` (registry + resolver)

The JSI ICast interface the embedder uses, which installs the Task 4 hook onto the runtime. Registry (`registerWasmBytecode(url, buffer)`) plus optional resolver, resolver-first precedence.

**Files:**
- Modify: `API/jsi/jsi/hermes-interfaces.h` (new `IWasmModuleProvider`/`IWasmModuleResolver` interfaces, following the `IWorkerSetup`/`ISetWorkerSetup` template at commit `1c9672f6d`)
- Modify: `API/hermes/hermes.cpp` (`HermesRuntimeImpl`: implement the interface, store registry + resolver, install the `std::function` into `impl().runtime_`; add to `castInterface`)
- Test: `unittests/API/APITest.cpp` (new case)

**Interfaces:**
- Consumes: `Runtime::setWasmModuleResolver` (Task 4).
- Produces (JSI, embedder-facing):
  - `IWasmModuleResolver::resolve(const std::string& url, std::string& error) -> std::shared_ptr<const jsi::Buffer>` (pure virtual; `.hbc` only).
  - `IWasmModuleProvider` (ICast on the runtime) with
    `registerWasmBytecode(std::string url, std::shared_ptr<const jsi::Buffer> bytecode)`,
    `setWasmModuleResolver(jsi::ICast* resolver)`,
    `getWasmModuleResolver()`.

- [ ] **Step 1: Write the failing APITest**

In `unittests/API/APITest.cpp`, a case that: builds a `HermesRuntime`, casts to `IWasmModuleProvider`, calls `registerWasmBytecode("app://m", <precompiled .hbc buffer>)`, evaluates `WebAssembly.Module.fromHermesURL("app://m")` and asserts a Module builds and runs; then a second case installing an `IWasmModuleResolver` and asserting resolver-first precedence (resolver answer wins; registry serves what the resolver declines).

- [ ] **Step 2: Run it to verify it fails**

Run: `cmake --build cmake-build-asan --target APITests -j 14 && cmake-build-asan/unittests/API/APITests --gtest_filter='*WasmModuleProvider*'`
Expected: FAIL — interface absent.

- [ ] **Step 3: Add the interfaces**

In `API/jsi/jsi/hermes-interfaces.h`, mirror `IWorkerSetup`/`ISetWorkerSetup` (see commit `1c9672f6d` for the exact ICast+UUID shape). Each interface gets a fresh UUID. `IWasmModuleResolver::resolve` returns `std::shared_ptr<const jsi::Buffer>` of `.hbc` (or nullptr + `error`). `IWasmModuleProvider` exposes `registerWasmBytecode`, `setWasmModuleResolver`, `getWasmModuleResolver`.

- [ ] **Step 4: Implement on `HermesRuntimeImpl`**

Add `IWasmModuleProvider` to its interface list; store `std::unordered_map<std::string, std::shared_ptr<const jsi::Buffer>> wasmRegistry_;` and `jsi::ICast* wasmResolver_ = nullptr;`. On any registration/resolver change, install (or refresh) the VM hook once:

```cpp
runtime_->setWasmModuleResolver(
    [this](const std::string &url, std::string &out) -> bool {
      // Resolver first.
      if (wasmResolver_) {
        if (auto *r = jsi::castInterface<IWasmModuleResolver>(wasmResolver_)) {
          std::string err;
          if (auto buf = r->resolve(url, err)) {
            out.assign(
                reinterpret_cast<const char *>(buf->data()), buf->size());
            return true;
          }
        }
      }
      // Registry fallback.
      auto it = wasmRegistry_.find(url);
      if (it != wasmRegistry_.end()) {
        out.assign(
            reinterpret_cast<const char *>(it->second->data()),
            it->second->size());
        return true;
      }
      return false;
    });
```
Add `IWasmModuleProvider::uuid` to `HermesRuntimeImpl::castInterface`.

- [ ] **Step 5: Build and run the APITest to verify it passes**

Run: `cmake --build cmake-build-asan --target APITests hermes hermesc -j 14 && cmake-build-asan/unittests/API/APITests --gtest_filter='*WasmModuleProvider*'`
Expected: PASS (registry case + resolver-precedence case).

- [ ] **Step 6: Prove precedence (mutation)** — swap the lambda to check the registry before the resolver; the precedence assertion must FAIL. Revert.

- [ ] **Step 7: Commit**

```bash
git add API/jsi/jsi/hermes-interfaces.h API/hermes/hermes.cpp unittests/API/APITest.cpp
git commit -m "Add IWasmModuleProvider embedder facility for trusted Wasm bytecode

Registry (registerWasmBytecode) plus optional IWasmModuleResolver, resolver
first then registry, installed as the vm::Runtime resolver hook. Mirrors
IWorkerSetup in spirit.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01Wt8qjqk6tfD8MVtfsbmebd"
```

---

### Task 6: Worker fix — DEFERRED (not implemented in this plan)

**Do not implement now.** The vulnerable worker is not in this branch's tree.

Rationale: this branch carries an older, string-only worker
(`API/hermes/extensions/Worker.cpp`: `args[1].asString(rt).utf8(rt)`), which
cannot carry bytecode — the magic `0x1F1903C103BC1FC6` is not valid UTF-8, so
`isHermesBytecode` can never fire on it. The vulnerable worker — the
`ArrayBuffer`/`TypedArray`/`DataView` path that copies raw JS-supplied bytes
into `evaluateJavaScript` — lives on `origin/static_h` (see its
`copyBufferBytes` / `startWorker`). This branch cannot rebase onto `static_h`
yet (it is mid-merge between two divergent Wasm lines), so there is nothing to
fix here.

**Follow-up, once this work lands and the branch rebases onto the `static_h`
worker:** gate the worker's binary-script path with `EnableUntrustedBytecodeFromJS`
(defined in Task 1). At the point `startWorker` receives its `std::string
script` from `copyBufferBytes`, if the flag is off and
`isHermesBytecode(script.data(), script.size())` is true, throw a `TypeError`
(refuse bytecode); when on, allow it. Propagate the flag from the parent runtime
into the worker's `RuntimeConfig`. Add a Worker-input test with a `.hbc`
`Uint8Array` (constructible against the typed-array worker: off → refused, on →
runs). This does not disturb the embedder's own trusted
`evaluateJavaScript(appBytecode)`.

Recorded in the design spec's "Worker fix — DEFERRED" section; tracked as a
follow-up task item in `handoff-artifacts/REVIEW.md` (Task 8).

---

### Task 7: Migrate the Wasm lit-test drivers off auto-detection

With sniffing off by default, drivers that did `new WebAssembly.Module(hbcBytes)` break. Move each to an explicit trusted path.

**Files:**
- Modify: the shared `test/wasm/load-hbc.js_` and any driver that loads `.hbc` through `WebAssembly.Module` (identify with the grep below)
- Modify: `test/wasm/*.wat` RUN lines that need a gate flag

**Interfaces:**
- Consumes: `hermescli.loadHBC` (gated behind `-Xhermes-internal-test-methods`, returns the module factory), or `WebAssembly.Module.fromHermesBytecode` under `-Xenable-untrusted-bytecode-from-js`.

- [ ] **Step 1: Enumerate affected drivers**

Run: `grep -rln "new WebAssembly.Module(" test/wasm/*.js_`
For each, decide: if it loads a precompiled `.hbc`, switch to `WebAssembly.Module.fromHermesBytecode(bytes)` and add `-Xenable-untrusted-bytecode-from-js` to the test's RUN line; if it compiles a `.wat`-derived `.wasm`, it is unaffected.

- [ ] **Step 2: Update `load-hbc.js_`**

Change `new WebAssembly.Module(bytes)` to `WebAssembly.Module.fromHermesBytecode(bytes)`, and add `-Xenable-untrusted-bytecode-from-js` to the RUN lines of tests that use it.

- [ ] **Step 3: Run the Wasm suite**

Run: `LIT_OPTS="-j8" LIT_FILTER="wasm|Wasm" cmake --build cmake-build-asan --target check-hermes -j 14`
Expected: all pass. Iterate per failing driver.

- [ ] **Step 4: Run the full unfiltered suite**

Run: `LIT_OPTS="-j8" cmake --build cmake-build-asan --target check-hermes -j 14`
Expected: no unexpected failures.

- [ ] **Step 5: Commit**

```bash
git add test/wasm
git commit -m "Migrate Wasm lit drivers off .hbc content-sniffing

Drivers that loaded precompiled .hbc through WebAssembly.Module now use the
explicit WebAssembly.Module.fromHermesBytecode under
-Xenable-untrusted-bytecode-from-js.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01Wt8qjqk6tfD8MVtfsbmebd"
```

---

### Task 8: Ledger + docs

**Files:**
- Modify: `handoff-artifacts/REVIEW.md` (mark §5.1 / §3.3 resolved; add the new predefined strings + Wasm builtins to the §4.1 bytecode-visible list)
- Modify: `handoff-artifacts/MERGE-TRIAGE.md` if any item is affected
- Modify: `doc/WasmSpecTestStatus.md` if behavior notes need updating

- [ ] **Step 1: Update the review ledger** — §3.3 Critical resolved; §5.1 decision implemented; §4.1 gains `fromHermesBytecode`/`fromHermesURL` predefined strings (bytecode-visible). Record the **deferred Worker follow-up** as an open item: after rebasing onto the `static_h` typed-array worker, gate its binary-script path with `EnableUntrustedBytecodeFromJS` (refuse `isHermesBytecode` input when off). Record commit subjects, not hashes (branch is rebased).

- [ ] **Step 2: Commit** the ledger/doc updates (untracked `handoff-artifacts/` is not committed; only `doc/` changes are).

---

## Self-review

**Spec coverage:**
- Two gates + CLI → Task 1. ✓
- Spec entries default `.wasm`, sniff gated, refuse-when-ungated → Task 2. ✓
- `fromHermesBytecode` gated → Task 3. ✓
- `fromHermesURL` + VM hook → Task 4. ✓
- Embedder registry + resolver + precedence → Task 5. ✓
- Worker fix → **deferred** (Task 6): the vulnerable typed-array worker is on `static_h`, not in this tree; the gate is designed to cover it post-rebase. ✓ (intentionally out of scope here)
- Behavior matrix → spec-entry rows exercised in Task 2; `fromHermesBytecode` in Task 3; `fromHermesURL` negative path in Task 4, positive path in Task 5. Worker row deferred. ✓

**Open items the implementer must resolve (flagged, not placeheld):**
- Task 3/4: the Module-object construction tail is currently inline in `wasmModuleConstructor`; extract a shared helper (`buildModuleObject(runtime, moduleData)`) the three entries call, rather than duplicating.
- Task 5 APITest (and the `fromHermesURL` positive path) needs a precompiled `.hbc` fixture; produce it with `hermesc --wasm -emit-binary` offline and embed the bytes, or generate it in the test's setup.
- Task 1: confirm the two opts land in `struct VMOnlyRuntimeFlags` (so `tools/hermes/hermes.cpp` reads them as `flags.<name>`); both `RuntimeFlags.cpp::buildRuntimeConfig` and the manual builder in `hermes.cpp` must set them, or `%hermes` won't recognize the CLI flags.

**Type consistency:** `WasmBytesMode` (Task 2) is consumed verbatim in Tasks 3/4. `WasmModuleResolver` signature (`bool(const std::string&, std::string&)`) is identical in Task 4 (definition) and Task 5 (lambda). `IWasmModuleProvider`/`IWasmModuleResolver` names match across Tasks 5. `fromHermesBytecode`/`fromHermesURL` predefined-string names match their `defineMethod` registrations.
