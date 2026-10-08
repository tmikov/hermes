# Live Exported Wasm Globals Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a mutable global that a Wasm module defines and exports a live
two-way view, so JS writes through `WebAssembly.Global.prototype.value` reach
the module and the module's `global.set` is visible to JS — as the JS API
requires.

**Architecture:** The module's frame `Variable` stays the single source of
truth. At instantiation the compiler creates a getter closure and, for a
mutable global, a setter closure over the module's top-level scope, and builds
the `WebAssembly.Global` through a new private builtin that stores those
closures in the cell. `Global.prototype.value` and the `wasmGlobalGet`/
`wasmGlobalSet` builtins call the closures when a global is closure-backed.
The module's own `global.get`/`global.set` are untouched and stay plain
`LoadFrameInst`/`StoreFrameInst`, so nothing inside the module pays for this.
Routing construction through a builtin also removes the
`globalThis.WebAssembly.Global` property read that script can currently
interpose on.

**Tech Stack:** C++17 (no exceptions, no RTTI), Hermes IR / `WasmIRGen`,
Hermes VM cells and `Metadata::Builder`, lit + FileCheck e2e tests, wabt's
`wat2wasm`.

**Spec:** `dz/issues/01a079c6-fdd6-71ac-b4f4-1bf5d80e736f.md` (dz issue
`01a079c6-fdd6`, including its comment recording the interposition hole).
Read it before starting; the measured Node-vs-Hermes tables in it are the
acceptance criteria.

## Global Constraints

- **Build with clang.** Configure with `-DCMAKE_C_COMPILER=clang
  -DCMAKE_CXX_COMPILER=clang++`. The default GCC build gives misleading
  codegen.
- **Default build is ASan+Debug at `-O1`**, directory `cmake-build-asan`.
  Every task builds and tests there unless a step says otherwise.
- **C++17, no exceptions, no RTTI.** 80-column lines, 2-space indent.
  Classes `PascalCase`, functions/variables `camelCase`, members `_suffix`.
- **Doc comment required on every declaration.**
- **New runtime code uses `Locals` + `PinnedValue`.** Do not introduce
  `GCScope` or `makeHandle()`. Before writing any code under `lib/VM/`,
  `include/hermes/VM/` or `API/hermes/`, invoke the `gc-safe-coding` skill.
- **Copyright header on every new file:**
  ```cpp
  /**
   * Copyright (c) Meta Platforms, Inc. and affiliates.
   *
   * This source code is licensed under the MIT license found in the
   * LICENSE file in the root directory of this source tree.
   */
  ```
- **Run single lit tests with `LIT_FILTER`**, never a hand-rolled wrapper:
  `LIT_FILTER="<regex>" cmake --build cmake-build-asan --target check-hermes`.
- **Every commit must leave `check-hermes` green.** Task 1 writes a test that
  fails on purpose; it is explicitly *not* committed until Task 5 makes it
  pass. Every other task commits a green tree.
- **Commit message trailer:**
  ```
  Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX
  ```

---

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `test/wasm/e2e-exported-mutable-global.wat` | **Create.** The acceptance test: an exported mutable global must be live in both directions, for i32/f64/i64, while an immutable export and a module-local global keep their current behavior. | 1, 5 |
| `test/wasm/e2e-exported-mutable-global-driver.js_` | **Create.** JS driver for the above. Mirrors `e2e-imported-mutable-global-driver.js_`. | 1, 5 |
| `include/hermes/VM/JSWebAssemblyGlobal.h` | **Modify.** Add the two closure fields, their accessors, and `isLive()`. Existing `value_`/`i64Value_`/`valType_`/`mutable_` stay. | 2 |
| `lib/VM/JSWebAssemblyGlobal.cpp` | **Modify.** Register the closure fields with `Metadata::Builder` so the GC traces them. | 2 |
| `include/hermes/FrontEndDefs/Builtins.def` | **Modify.** Declare `wasmMakeGlobal`. | 3 |
| `lib/VM/JSLib/HermesBuiltin.cpp` | **Modify.** Implement `wasmMakeGlobal`; teach `wasmGlobalGet`/`wasmGlobalSet` to call the closures; have `wasmLinkGlobal` return the matched object for a live global. | 3, 4 |
| `lib/VM/JSLib/WebAssembly/WebAssembly.cpp` | **Modify.** `wasmGlobalValueGetter`/`wasmGlobalValueSetter` call the closures when the global is live. | 4 |
| `include/hermes/WasmIRGen/WasmHelpers.h`, `lib/WasmIRGen/WasmHelpers.cpp` | **Modify.** Add `emitMakeGlobal()`. | 5 |
| `include/hermes/WasmIRGen/WasmIRGen.h`, `lib/WasmIRGen/WasmIRGen.cpp` | **Modify.** Create the accessor closures; replace the global-export construction with the builtin call; stop snapshotting mutable imports in `initializeGlobals()`. | 5 |
| `test/wasm/e2e-global-export-not-interposable.wat` + `-driver.js_` | **Create.** Regression test that replacing `globalThis.WebAssembly.Global` cannot change what a module exports. | 6 |
| `include/hermes/BCGen/HBC/BytecodeVersion.h` | **Modify.** Bump `BYTECODE_VERSION`. | 6 |

**Naming used across tasks** (fixed here so tasks agree):

- Cell: `getGetter(Runtime &)`, `setGetter(Runtime &, Callable *)`,
  `getSetter(Runtime &)`, `setSetter(Runtime &, Callable *)`,
  `isLive(Runtime &) const`.
- Builtin: `wasmMakeGlobal(valTypeCode, isMutable, valueOrGetter, setterOrUndefined)`.
- Helper: `WasmHelpers::emitMakeGlobal(Value *valTypeCode, Value *isMutable, Value *valueOrGetter, Value *setterOrUndefined)`.
- IRGen: `WasmIRGen::createGlobalAccessor(uint32_t globalIndex, bool isSetter, Instruction *tlScope)`.

---

### Task 1: Write the acceptance test and watch it fail

**Files:**
- Create: `test/wasm/e2e-exported-mutable-global.wat`
- Create: `test/wasm/e2e-exported-mutable-global-driver.js_`

**Interfaces:**
- Consumes: nothing.
- Produces: a lit test that Task 5 must turn green. Not committed in this
  task — committing a knowingly-red lit test would break `check-hermes` for
  everyone. Leave both files in the working tree; Task 5 commits them
  alongside the fix.

The expected output below is what **Node v24.13.1 produces** for the same
module. That is the specification. Do not weaken it to match current Hermes.

- [ ] **Step 1: Create the `.wat`**

`test/wasm/e2e-exported-mutable-global.wat`:

```wat
;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A mutable global that this module DEFINES and EXPORTS is shared state: the
;; exported WebAssembly.Global and the module's own storage are one cell, so a
;; global.set inside Wasm must be visible through `.value` and a host write to
;; `.value` must be visible to the next global.get. The export used to be a
;; snapshot taken at instantiation, so writes were silently lost in both
;; directions. An immutable export is still a snapshot, which is correct
;; because its value cannot change.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-exported-mutable-global-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (global $counter (export "counter") (mut i32) (i32.const 5))
  (global $ratio (export "ratio") (mut f64) (f64.const 2.5))
  (global $big (export "big") (mut i64) (i64.const 4294967296))
  ;; An immutable export stays a snapshot.
  (global $konst (export "konst") i32 (i32.const 7))
  ;; A module-local mutable global is not exported and must be unaffected.
  (global $local (mut i32) (i32.const 1))
  ;; The SAME global under a second export name. The loop builds one Global
  ;; per export ENTRY, so these are two objects; they must still name one
  ;; storage cell. (Whether they should be the SAME object is a separate,
  ;; deferred spec discrepancy that Node shares -- do not assert `===`.)
  (export "counter2" (global $counter))

  (func (export "get_counter") (result i32) global.get $counter)
  (func (export "bump_counter") (param i32)
    global.get $counter
    local.get 0
    i32.add
    global.set $counter)

  (func (export "get_ratio") (result f64) global.get $ratio)
  (func (export "scale_ratio") (param f64)
    global.get $ratio
    local.get 0
    f64.mul
    global.set $ratio)

  ;; Return the i64 halves separately, so a lost upper word is visible
  ;; directly rather than only through 64-bit arithmetic.
  (func (export "get_big_lo") (result i32) global.get $big i32.wrap_i64)
  (func (export "get_big_hi") (result i32)
    global.get $big
    i64.const 32
    i64.shr_u
    i32.wrap_i64)
  (func (export "add_big") (param i64)
    global.get $big
    local.get 0
    i64.add
    global.set $big)

  (func (export "get_konst") (result i32) global.get $konst)

  (func (export "get_local") (result i32) global.get $local)
  (func (export "bump_local") (param i32)
    global.get $local
    local.get 0
    i32.add
    global.set $local))

;; CHECK: counter starts at 5
;; CHECK-NEXT: after wasm bump, host sees 15
;; CHECK-NEXT: after host set, wasm sees 100
;; CHECK-NEXT: after another wasm bump, host sees 101
;; CHECK-NEXT: ratio starts at 2.5
;; CHECK-NEXT: after wasm scale, host sees 10
;; CHECK-NEXT: after host set, wasm sees 0.5
;; CHECK-NEXT: big starts at 4294967296 bigint
;; CHECK-NEXT: after wasm add, host sees 4294967297
;; CHECK-NEXT: after host set, wasm sees lo/hi = -1/-1
;; CHECK-NEXT: konst is 7
;; CHECK-NEXT: writing konst threw TypeError
;; CHECK-NEXT: local after bump = 4
;; CHECK-NEXT: counter is a WebAssembly.Global = true
;; CHECK-NEXT: counter has no own properties = true
;; CHECK-NEXT: two export names, one storage cell = true true true
;; CHECK-NEXT: valueOf that mutates then throws: caught TypeError
;; CHECK-NEXT: mutation from the throwing valueOf survived = 777
;; CHECK-NEXT: done
```

- [ ] **Step 2: Create the driver**

`test/wasm/e2e-exported-mutable-global-driver.js_`:

```js
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// JS driver for e2e-exported-mutable-global.wat.

var args = hermescli.getScriptArgs();
var mod = WebAssembly.Module.fromHermesBytecode(hermescli.loadFile(args[0]));
var ex = new WebAssembly.Instance(mod).exports;

// i32: both directions must be visible.
print('counter starts at ' + ex.counter.value);
ex.bump_counter(10);
print('after wasm bump, host sees ' + ex.counter.value);
ex.counter.value = 100;
print('after host set, wasm sees ' + ex.get_counter());
ex.bump_counter(1);
print('after another wasm bump, host sees ' + ex.counter.value);

// f64: same, with a type that is not an integer.
print('ratio starts at ' + ex.ratio.value);
ex.scale_ratio(4);
print('after wasm scale, host sees ' + ex.ratio.value);
ex.ratio.value = 0.5;
print('after host set, wasm sees ' + ex.get_ratio());

// i64: crosses the boundary as a BigInt and must keep both words.
print('big starts at ' + ex.big.value + ' ' + typeof ex.big.value);
ex.add_big(1n);
print('after wasm add, host sees ' + ex.big.value);
ex.big.value = -1n;
print('after host set, wasm sees lo/hi = ' +
      ex.get_big_lo() + '/' + ex.get_big_hi());

// An immutable export is a snapshot, and refuses writes.
print('konst is ' + ex.konst.value);
try {
  ex.konst.value = 9;
  print('writing konst was allowed');
} catch (e) {
  print('writing konst threw ' + e.constructor.name);
}

// A module-local mutable global is unaffected by any of the above.
ex.bump_local(3);
print('local after bump = ' + ex.get_local());

// The export is a genuine Global with no own properties, as the spec
// requires and as an importing module's brand check depends on.
print('counter is a WebAssembly.Global = ' +
      (ex.counter instanceof WebAssembly.Global));
print('counter has no own properties = ' +
      (Object.getOwnPropertyNames(ex.counter).length === 0));

// STORAGE coherence across two export names of one global. Deliberately not
// an identity assertion: `ex.counter === ex.counter2` is false here and in
// Node, and that discrepancy is deferred. What must hold is that all three
// routes reach one cell.
ex.counter.value = 55;
var a = ex.counter2.value === 55;
ex.counter2.value = 66;
var b = ex.get_counter() === 66;
ex.bump_counter(1);
var c = ex.counter.value === 67 && ex.counter2.value === 67;
print('two export names, one storage cell = ' + a + ' ' + b + ' ' + c);

// ToNumber runs exactly once and its side effects are not rolled back: a
// valueOf that mutates the global and THEN throws keeps its mutation, and
// the assignment that triggered it does not commit.
ex.counter.value = 0;
try {
  ex.counter.value = {valueOf: function () {
    ex.counter.value = 777;
    throw new TypeError('from valueOf');
  }};
  print('valueOf that mutates then throws: no exception');
} catch (e) {
  print('valueOf that mutates then throws: caught ' + e.constructor.name);
}
print('mutation from the throwing valueOf survived = ' + ex.get_counter());
print('done');
```

- [ ] **Step 3: Run it and confirm it fails for the right reason**

```bash
LIT_FILTER="e2e-exported-mutable-global" \
  cmake --build cmake-build-asan --target check-hermes
```

Expected: FAIL. The first mismatch must be
`after wasm bump, host sees 5` where `15` was expected — the snapshot. If it
fails earlier or differently, the test is wrong, not the engine; fix the test
before continuing.

- [ ] **Step 4: Do NOT commit**

Leave both files uncommitted. Task 5 commits them. Record in your notes that
the tree has two uncommitted test files so a later `git add -A` does not
surprise you.

---

### Task 2: Give `JSWebAssemblyGlobal` closure-backed storage

**Files:**
- Modify: `include/hermes/VM/JSWebAssemblyGlobal.h:22-131`
- Modify: `lib/VM/JSWebAssemblyGlobal.cpp:32-40`

**Interfaces:**
- Consumes: nothing.
- Produces: `Callable *getGetter(Runtime &) const`,
  `void setGetter(Runtime &, Callable *)`, `Callable *getSetter(Runtime &) const`,
  `void setSetter(Runtime &, Callable *)`, `bool isLive(Runtime &) const`.
  Tasks 3 and 4 call all five.

Invoke the `gc-safe-coding` skill before writing this task.

- [ ] **Step 1: Add the include and the fields**

In `include/hermes/VM/JSWebAssemblyGlobal.h`, add after the existing includes:

```cpp
#include "hermes/VM/Callable.h"
```

Add to the private section, after `bool mutable_{false};`:

```cpp
  /// For a LIVE global, the closure that reads the module's storage; null for
  /// a snapshot global. A live global stores no value of its own: value_ and
  /// i64Value_ are unused and the module's frame Variable is the single
  /// source of truth, which is what makes an exported mutable global a
  /// two-way view rather than a copy taken at instantiation.
  GCPointer<Callable> getter_;

  /// For a live global, the closure that writes the module's storage; null
  /// for a snapshot one. Non-null exactly when getter_ is: a live global is
  /// always mutable (wasmMakeGlobal refuses an immutable live global, and
  /// wasmLinkGlobal's success answer depends on that invariant).
  GCPointer<Callable> setter_;
```

- [ ] **Step 2: Initialize them in the constructor**

Replace the existing constructor body with:

```cpp
  JSWebAssemblyGlobal(
      Runtime &runtime,
      Handle<JSObject> parent,
      Handle<HiddenClass> clazz)
      : JSObject(runtime, *parent, *clazz),
        getter_(runtime, nullptr, runtime.getHeap()),
        setter_(runtime, nullptr, runtime.getHeap()) {}
```

- [ ] **Step 3: Add the accessors**

Add to the public section, next to `isMutable()`:

```cpp
  /// \return the closure that reads a live global's storage, or nullptr if
  /// this global is a snapshot.
  Callable *getGetter(Runtime &runtime) const {
    return getter_.get(runtime);
  }

  /// Set the closure that reads a live global's storage.
  void setGetter(Runtime &runtime, Callable *fn) {
    getter_.set(runtime, fn, runtime.getHeap());
  }

  /// \return the closure that writes a live mutable global's storage, or
  /// nullptr if this global is a snapshot or is immutable.
  Callable *getSetter(Runtime &runtime) const {
    return setter_.get(runtime);
  }

  /// Set the closure that writes a live mutable global's storage.
  void setSetter(Runtime &runtime, Callable *fn) {
    setter_.set(runtime, fn, runtime.getHeap());
  }

  /// \return true if this global reads and writes a module's storage through
  /// closures rather than holding a value of its own. A live global is always
  /// mutable and therefore always has both closures; wasmMakeGlobal refuses
  /// any other combination, and wasmLinkGlobal depends on that.
  bool isLive(Runtime &runtime) const {
    return getter_.get(runtime) != nullptr;
  }
```

- [ ] **Step 4: Register the fields with the GC**

In `lib/VM/JSWebAssemblyGlobal.cpp`, replace `JSWebAssemblyGlobalBuildMeta`
with:

```cpp
void JSWebAssemblyGlobalBuildMeta(
    const GCCell *cell,
    Metadata::Builder &mb) {
  mb.addJSObjectOverlapSlots(
      JSObject::numOverlapSlots<JSWebAssemblyGlobal>());
  JSObjectBuildMeta(cell, mb);
  const auto *self = static_cast<const JSWebAssemblyGlobal *>(cell);
  mb.setVTable(&JSWebAssemblyGlobal::vt);
  // value_ and i64Value_ are plain scalars and are not registered. The two
  // closures are GC references and must be, or a live global's accessor is
  // collected out from under it.
  mb.addField("getter", &self->getter_);
  mb.addField("setter", &self->setter_);
}
```

Note the existing comment `// No GC pointer fields — value_ is a plain
double, not a GC reference.` is now false; the replacement above removes it.

- [ ] **Step 5: Build and run the existing suite**

```bash
cmake --build cmake-build-asan --target hermes hermesc
cmake --build cmake-build-asan --target check-hermes
```

Expected: PASS. This task changes no behavior — it only widens the cell.
The `e2e-exported-mutable-global` test from Task 1 still fails; that is
expected and is why it is not committed yet.

- [ ] **Step 6: Prove the new fields are actually traced**

A green suite does not prove the metadata registration works, because nothing
stores a closure yet. Prove the check can fail: temporarily comment out the
two `mb.addField` lines, rebuild, and confirm that
`cmake --build cmake-build-asan --target check-hermes` still passes — showing
the suite gives no signal here — then restore them. Record in the commit
message that this task has no independent test, and that the fields' tracing
is INTENDED to be demonstrated by Task 5 Step 8. Do not claim that coverage
here: Step 8 may find it cannot make the failure deterministic, and instructs
the implementer to say so rather than assert coverage.

- [ ] **Step 7: Commit**

```bash
git add include/hermes/VM/JSWebAssemblyGlobal.h lib/VM/JSWebAssemblyGlobal.cpp
git commit -m "Give JSWebAssemblyGlobal closure-backed storage fields

A global exported by a module is currently a copy of the module's storage
taken at instantiation, so writes are lost in both directions. Add the two
GC-traced closure fields a live global needs; nothing populates them yet.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX"
```

---

### Task 3: Add the `wasmMakeGlobal` builtin

**Files:**
- Modify: `include/hermes/FrontEndDefs/Builtins.def:375` (after `WASM_BUILTIN(wasmSetFuncInfo)`)
- Modify: `lib/VM/JSLib/HermesBuiltin.cpp` (inside the `#ifdef HERMES_ENABLE_WASM` region that ends at the `#endif // HERMES_ENABLE_WASM` around line 2615, next to `wasmLinkGlobal`)
- Modify: `lib/VM/JSLib/HermesBuiltin.cpp:3660-3901` (the registration block)

**Interfaces:**
- Consumes: `setGetter`/`setSetter`/`setValType`/`setMutable` from Task 2.
- Produces: builtin `HermesBuiltin_wasmMakeGlobal`, called as
  `wasmMakeGlobal(valTypeCode, isMutable, valueOrGetter, setterOrUndefined)`
  and returning a `JSWebAssemblyGlobal`. Task 5 emits calls to it.

Invoke the `gc-safe-coding` skill before writing this task.

**Why a builtin rather than the public constructor.** The current export path
reaches the constructor with `TryLoadGlobalPropertyInst "WebAssembly"` +
`LoadPropertyInst "Global"` — two ordinary property reads. Script can replace
`WebAssembly.Global` and decide what a module's exported globals are; verified,
and Node does not allow it. The neighbouring export kinds already refuse this
(`LinkError: WebAssembly.Memory did not construct a memory for this module's
memory 0`). A builtin has no property read to interpose on.

- [ ] **Step 1: Declare the builtin**

In `include/hermes/FrontEndDefs/Builtins.def`, immediately after
`WASM_BUILTIN(wasmSetFuncInfo)` and before the `JS_BUILTIN(spawnAsync)` block:

```
// wasmMakeGlobal(valTypeCode, isMutable, valueOrGetter, setterOrUndefined):
// build the WebAssembly.Global a module publishes for an exported global,
// without going through globalThis.WebAssembly.Global -- that is an ordinary
// property read, and replacing it let script decide what a module's exports
// ARE. A defined memory and a defined FUNCREF table already refuse that,
// being brand-checked at construction; the externref table export does not,
// and is a separate defect.
// When valueOrGetter is callable the global is LIVE: it stores no value and
// reads and writes the module's frame slot through the two closures, so an
// exported mutable global is a two-way view rather than a snapshot. Otherwise
// it is a snapshot initialized to valueOrGetter, which is correct only for an
// immutable global and is what this builtin requires.
// valTypeCode is a JSWebAssemblyGlobal::ValType.
WASM_BUILTIN(wasmMakeGlobal)
```

Builtin ids are enum entries in file order (`Builtins.h:17-28`), so inserting
here shifts `_firstJS` and the three `JS_BUILTIN` ids. That is a bytecode
change and requires a `BYTECODE_VERSION` bump — Task 6 does it, and dz task
`01a0460c-4361` tracks that the bump must land last.

- [ ] **Step 2: Implement it**

In `lib/VM/JSLib/HermesBuiltin.cpp`, immediately after `wasmLinkGlobal`:

```cpp
/// wasmMakeGlobal(valTypeCode, isMutable, valueOrGetter, setterOrUndefined)
///   -> a JSWebAssemblyGlobal.
///
/// See the note in Builtins.def for why the export path does not use the
/// public constructor. A PRIVATE_BUILTIN is reachable from any bytecode
/// emitting a CallBuiltin with its index, so every argument is checked here
/// rather than asserted: the compiler's contract is not a guarantee about
/// what reaches this function.
CallResult<HermesValue> wasmMakeGlobal(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  if (LLVM_UNLIKELY(!args.getArg(0).isNumber() || !args.getArg(1).isBool()))
    return runtime.raiseTypeError(
        "wasmMakeGlobal: bad type code or mutability");
  // Range-check the DOUBLE before converting. getNumberAs<uint32_t>()
  // asserts its argument is exactly representable and casts unchecked
  // otherwise (HermesValue.h:430-436), so NaN, an infinity or a negative
  // would abort a Debug build and be undefined in a release one -- and any
  // bytecode can call a private builtin with any argument. (wasmLinkGlobal
  // at HermesBuiltin.cpp:2387 has the same shape and predates this; worth
  // its own fix, not this one's.)
  double rawCode = args.getArg(0).getNumber();
  if (LLVM_UNLIKELY(
          !(rawCode >= 0) ||
          rawCode > static_cast<double>(JSWebAssemblyGlobal::ValType::F64) ||
          rawCode != std::floor(rawCode)))
    return runtime.raiseTypeError("wasmMakeGlobal: unknown value type");
  auto valType =
      static_cast<JSWebAssemblyGlobal::ValType>(static_cast<uint8_t>(rawCode));
  bool isMutable = args.getArg(1).getBool();

  struct : public Locals {
    PinnedValue<Callable> getter;
    PinnedValue<Callable> setter;
    PinnedValue<JSWebAssemblyGlobal> glob;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  // Pin both closures BEFORE create() below, which allocates: a raw pointer
  // taken from an argument does not survive a safepoint.
  bool live = false;
  if (auto *getter = dyn_vmcast<Callable>(args.getArg(2))) {
    // LIVE IMPLIES MUTABLE, enforced here rather than assumed. wasmLinkGlobal
    // returns the matched OBJECT for a live global on the reasoning that only
    // an immutable import consumes the returned value and a live global can
    // never satisfy an immutable declaration. An immutable live Global would
    // pass the type/mutability match at HermesBuiltin.cpp:2389 and hand that
    // object to an immutable import as its VALUE -- and for i64 straight into
    // the BigInt splitter, which rejects it. A compiler that never emits the
    // combination is not the same as a builtin that refuses it.
    if (LLVM_UNLIKELY(!isMutable))
      return runtime.raiseTypeError(
          "wasmMakeGlobal: an immutable global must be a snapshot");
    live = true;
    lv.getter = getter;
    auto *setter = dyn_vmcast<Callable>(args.getArg(3));
    if (LLVM_UNLIKELY(!setter))
      return runtime.raiseTypeError(
          "wasmMakeGlobal: a live global needs a setter");
    lv.setter = setter;
  } else if (LLVM_UNLIKELY(isMutable)) {
    // A snapshot cannot be mutable: its writes would go nowhere, which is
    // exactly the bug this builtin exists to fix.
    return runtime.raiseTypeError(
        "wasmMakeGlobal: a mutable global must be live");
  }

  // An i64 snapshot's value is a BigInt, matching Global.prototype.value.
  int64_t initI64 = 0;
  double initValue = 0.0;
  if (!live) {
    if (valType == JSWebAssemblyGlobal::ValType::I64) {
      if (LLVM_UNLIKELY(!args.getArg(2).isBigInt()))
        return runtime.raiseTypeError(
            "wasmMakeGlobal: an i64 global requires a BigInt value");
      initI64 = static_cast<int64_t>(
          args.getArg(2).getBigInt()->truncateToSingleDigit());
    } else {
      if (LLVM_UNLIKELY(!args.getArg(2).isNumber()))
        return runtime.raiseTypeError(
            "wasmMakeGlobal: a snapshot global requires a Number value");
      initValue = args.getArg(2).getNumber();
    }
  }

  Handle<JSObject> globalPrototype{runtime.wasmGlobalPrototype};
  lv.glob = JSWebAssemblyGlobal::create(runtime, globalPrototype);
  // The type must be set before setWasmGlobalNumber, which coerces to it.
  lv.glob->setValType(valType);
  lv.glob->setMutable(isMutable);
  if (live) {
    // live implies mutable, refused above otherwise, so both are present.
    lv.glob->setGetter(runtime, lv.getter.get());
    lv.glob->setSetter(runtime, lv.setter.get());
  } else {
    lv.glob->setI64Value(initI64);
    if (valType != JSWebAssemblyGlobal::ValType::I64)
      setWasmGlobalNumber(lv.glob.get(), initValue);
  }
  return lv.glob.getHermesValue();
}
```

- [ ] **Step 3: Register it**

Registration is a four-argument `defineInternMethod` taking a **predefined
string**, not a two-field table. Next to `wasmSetFuncInfo`
(`HermesBuiltin.cpp:3882-3884`), inside the same `#ifdef HERMES_ENABLE_WASM`
guard:

```cpp
  defineInternMethod(
      B::HermesBuiltin_wasmMakeGlobal, P::wasmMakeGlobal, wasmMakeGlobal, 4);
```

`P::wasmMakeGlobal` does not exist yet. Add it to the WASM block of
`include/hermes/VM/PredefinedStrings.def`, alongside `wasmLinkGlobal` at
line 551 and `wasmSetFuncInfo` at 554:

```
STR(wasmMakeGlobal, "wasmMakeGlobal")
```

That file generates the `Predefined` enum, so the build fails loudly if this
is missed — but it fails in a place that does not name the cause.

- [ ] **Step 4: Extend the disabled-build registration range**

This is a required edit, not a check. The builtins are compiled
unconditionally because `Builtins.def` numbering is independent of
`HERMES_ENABLE_WASM`, so with Wasm off every wasm id must still resolve to
something. They are registered by ONE loop over a contiguous range
(`HermesBuiltin.cpp:3888-3900`):

```cpp
  for (unsigned i = B::HermesBuiltin_wasmTrap;
       i <= B::HermesBuiltin_wasmSetFuncInfo;
       ++i) {
```

`wasmMakeGlobal` was appended AFTER `wasmSetFuncInfo`, so it falls outside
that range and its id resolves to nothing. Move the bound with it:

```cpp
       i <= B::HermesBuiltin_wasmMakeGlobal;
```

The range must stay contiguous — if a future builtin is inserted elsewhere,
this loop breaks silently.

```bash
cmake -B cmake-build-wasm-off -G Ninja -DCMAKE_BUILD_TYPE=Debug \
  -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ \
  -DHERMES_ENABLE_WASM=OFF
cmake --build cmake-build-wasm-off --target hermes
```

Expected: links cleanly.

- [ ] **Step 5: Build and run the suite**

```bash
cmake --build cmake-build-asan --target hermes hermesc
cmake --build cmake-build-asan --target check-hermes
```

Expected: PASS. Nothing emits the builtin yet, so behavior is unchanged.

- [ ] **Step 6: Commit**

```bash
git add include/hermes/FrontEndDefs/Builtins.def lib/VM/JSLib/HermesBuiltin.cpp
git commit -m "Add the wasmMakeGlobal builtin

Builds the WebAssembly.Global for an exported global without reading
globalThis.WebAssembly.Global, and can back one with getter/setter closures
instead of a value. Nothing emits it yet.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX"
```

---

### Task 4: Make every read and write path honor the closures

**Files:**
- Modify: `lib/VM/JSLib/WebAssembly/WebAssembly.cpp:2266-2329` (`wasmGlobalValueGetter`, `wasmGlobalValueSetter`)
- Modify: `lib/VM/JSLib/HermesBuiltin.cpp` (`wasmGlobalGet`, `wasmGlobalSet`, `wasmLinkGlobal`)

**Interfaces:**
- Consumes: `getGetter`/`getSetter`/`isLive` from Task 2.
- Produces: no new symbols. Task 5 relies on all four paths working.

Invoke the `gc-safe-coding` skill before writing this task.

There are exactly four places that read or write a global's value. All four
must go through the closures when the global is live, or the bug moves rather
than disappears.

- [ ] **Step 1: `wasmGlobalValueGetter` calls the getter closure**

In `WebAssembly.cpp`, after the `dyn_vmcast` brand check and before the i64
branch:

```cpp
  // A live global holds no value: its storage is the module's frame slot,
  // reached through the closure. The closure returns the value already in JS
  // form -- a Number, or a BigInt for i64 -- so nothing is coerced here.
  if (Callable *fn = glob->getGetter(runtime)) {
    struct : public Locals {
      PinnedValue<Callable> fn;
    } lv;
    LocalsRAII lraii(runtime, &lv);
    lv.fn = fn;
    auto res = Callable::executeCall0(
        lv.fn, runtime, Runtime::getUndefinedValue());
    if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION))
      return ExecutionStatus::EXCEPTION;
    return res->getHermesValue();
  }
```

`glob` is a raw pointer and `executeCall0` is a safepoint; the code above
returns immediately after it and never touches `glob` again, which is why no
re-derive is needed here. Do not add code between the call and the return.

- [ ] **Step 2: `wasmGlobalValueSetter` calls the setter closure**

In `WebAssembly.cpp`, the existing order — brand check, then the
`!glob->isMutable()` TypeError — stays exactly as it is; an immutable global
is refused before any closure is consulted. Replace the body *after* the
mutability check with:

```cpp
  struct : public Locals {
    PinnedValue<> newVal;
    PinnedValue<Callable> fn;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.newVal = args.getArg(0);
  // Only a BOOL survives past this point. toNumber_RJS below is a safepoint,
  // and a raw Callable* held across it is stale even where it is merely
  // tested for null; lv.fn is what the call uses, and PinnedValue is what the
  // GC updates.
  bool hasSetter = false;
  if (Callable *setterFn = glob->getSetter(runtime)) {
    lv.fn = setterFn;
    hasSetter = true;
  }

  if (glob->getValType() == JSWebAssemblyGlobal::ValType::I64) {
    if (!lv.newVal->isBigInt()) {
      return runtime.raiseTypeError(
          "WebAssembly.Global.prototype.value: an i64 global requires a "
          "BigInt value");
    }
    if (hasSetter) {
      // The closure splits the BigInt into the lo/hi pair the compiler
      // represents i64 with; doing it here would put the compiler's storage
      // layout in the runtime.
      auto res = Callable::executeCall1(
          lv.fn,
          runtime,
          Runtime::getUndefinedValue(),
          lv.newVal.getHermesValue());
      if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION))
        return ExecutionStatus::EXCEPTION;
      return HermesValue::encodeUndefinedValue();
    }
    glob = vmcast<JSWebAssemblyGlobal>(args.getThisArg());
    glob->setI64Value(
        static_cast<int64_t>(lv.newVal->getBigInt()->truncateToSingleDigit()));
    return HermesValue::encodeUndefinedValue();
  }

  auto numRes = toNumber_RJS(runtime, lv.newVal);
  if (LLVM_UNLIKELY(numRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  // toNumber_RJS is a safepoint and `glob` is a raw pointer, so re-derive it
  // rather than trusting the one taken before the call.
  glob = vmcast<JSWebAssemblyGlobal>(args.getThisArg());
  if (hasSetter) {
    // ToNumber has run; the closure narrows to the declared Wasm type in IR
    // with AsInt32Inst for i32 and emitFround for f32 -- the instructions
    // coerceImportedGlobalValue (WasmIRGen.cpp:7736) uses, NOT the module's
    // own global.set, which narrows nothing and stores an already-typed
    // value (WasmIRGen.cpp:7884). A live global and a snapshot one must
    // coerce identically, and setWasmGlobalNumber is what a snapshot does.
    auto res = Callable::executeCall1(
        lv.fn,
        runtime,
        Runtime::getUndefinedValue(),
        HermesValue::encodeTrustedNumberValue(numRes->getDouble()));
    if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION))
      return ExecutionStatus::EXCEPTION;
    return HermesValue::encodeUndefinedValue();
  }
  setWasmGlobalNumber(glob, numRes->getDouble());
  return HermesValue::encodeUndefinedValue();
```

- [ ] **Step 3: `wasmGlobalGet` and `wasmGlobalSet` call the closures**

These two are the cross-module path: module B imports module A's exported
mutable global, so B's `global.get` reaches A's frame slot through A's
closure. In `HermesBuiltin.cpp`, add to `wasmGlobalGet` after its brand check:

```cpp
  // A live global's storage is another module's frame slot. This is the one
  // place a builtin invokes compiler-generated IR: the closure body is a
  // frame load plus, for i64, the BigInt assembly, and calls nothing that can
  // reenter or run user code. Keep it that way -- the safety of every caller
  // of this builtin rests on it.
  //
  // Be precise about what is new here. This builtin ALREADY throws on a bad
  // argument and ALREADY allocates a BigInt for an i64 global, so neither
  // allocation nor an exception is introduced. What is new is interpreted
  // execution and the rooting obligations that come with it.
  if (Callable *fn = glob->getGetter(runtime)) {
    struct : public Locals {
      PinnedValue<Callable> fn;
    } lv;
    LocalsRAII lraii(runtime, &lv);
    lv.fn = fn;
    auto res = Callable::executeCall0(
        lv.fn, runtime, Runtime::getUndefinedValue());
    if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION))
      return ExecutionStatus::EXCEPTION;
    return res->getHermesValue();
  }
```

`wasmGlobalSet` interleaves its type checks with its stores
(`HermesBuiltin.cpp:2476-2495`: the i64 branch checks `isBigInt` then calls
`setI64Value` and returns; the fall-through checks `isNumber` then calls
`setWasmGlobalNumber`). Insert the closure call into **each** branch, after
that branch's type check and in place of its store — so a wrong type is still
refused before any closure runs. Add this local block once, immediately after
the `!glob->isMutable()` check:

```cpp
  // A live global's storage is another module's frame slot; the closure
  // writes it. The type checks below are unchanged and still run first.
  //
  // The raw Callable* is consumed immediately and only a BOOL survives.
  // Unlike the public setter this builtin never calls toNumber_RJS -- it
  // type-checks its argument directly -- so the only safepoint here is the
  // executeCall1 itself. Pinning anyway keeps the two setters the same shape
  // and survives anyone later adding a coercion above the call.
  struct : public Locals {
    PinnedValue<Callable> fn;
    PinnedValue<> arg;
  } lv;
  LocalsRAII lraii(runtime, &lv);
  bool hasSetter = false;
  if (Callable *setterFn = glob->getSetter(runtime)) {
    lv.fn = setterFn;
    hasSetter = true;
  }
```

then in the i64 branch, between the `isBigInt` check and the `setI64Value`
call:

```cpp
    if (hasSetter) {
      lv.arg = val;
      auto res = Callable::executeCall1(
          lv.fn,
          runtime,
          Runtime::getUndefinedValue(),
          lv.arg.getHermesValue());
      if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION))
        return ExecutionStatus::EXCEPTION;
      return HermesValue::encodeUndefinedValue();
    }
```

and the same block between the `isNumber` check and the
`setWasmGlobalNumber` call. `val` is a `HermesValue` local read from
`args.getArg(1)` before any safepoint, which is why it is copied into
`lv.arg` before the call rather than used directly.

- [ ] **Step 4: Give `wasmLinkGlobal` an answer for a live global**

`wasmLinkGlobal` ends with `return HermesValue::encodeTrustedNumberValue(
glob->getValue());`, which reads a field a live global does not use. It must
not call the getter: the whole point of that builtin is that linking reads
internal fields and runs nothing.

It does not need to. `WasmIRGen.cpp:1068` selects what is kept:

```cpp
Value *globalObjValue = imp.globalType.mutable_ ? importVal : linked;
```

so the returned *value* is consumed only for an **immutable** import — and a
live global is always mutable, so `glob->isMutable() != expectedMutable`
already rejects it for an immutable declaration. The value is therefore never
consumed for a live global. No NEW sentinel is needed either — return the
matched `Global` object itself, which is neither `null` nor `undefined` and
so is already distinguishable from both failure answers. Insert it AFTER the
type and mutability match (`HermesBuiltin.cpp:2389`) and BEFORE any backing
value field is read, i.e. immediately before the i64 branch:

```cpp
  // A live global's value is not readable here, and does not need to be: only
  // an IMMUTABLE import consumes the value this builtin returns (see
  // globalObjValue in WasmIRGen::finalizeModule's import loop), a live global
  // is always mutable, and the mutability check above has already refused a
  // mutable Global for an immutable declaration.
  //
  // The matched object IS the success answer -- it is neither of the two
  // failure sentinels, so no third protocol state is introduced. The caller
  // stores importVal for a mutable import regardless, so what is returned
  // here is discarded on exactly the path that can produce a live global.
  if (glob->isLive(runtime))
    return args.getArg(0);
```

Also update the `wasmLinkGlobal` doc comment in `Builtins.def:350-357` and the
`///` block above the function to describe this outcome.

- [ ] **Step 5: Build and run the suite**

```bash
cmake --build cmake-build-asan --target hermes hermesc
cmake --build cmake-build-asan --target check-hermes
```

Expected: PASS. Nothing constructs a live global yet, so every new branch is
still dead — behavior is unchanged.

- [ ] **Step 6: Commit**

```bash
git add lib/VM/JSLib/WebAssembly/WebAssembly.cpp lib/VM/JSLib/HermesBuiltin.cpp \
        include/hermes/FrontEndDefs/Builtins.def
git commit -m "Route every global value access through the closures when live

The four places that read or write a WebAssembly.Global's value now consult
the getter/setter closures for a live global. wasmLinkGlobal gets a fourth
outcome instead, because linking must not run IR. Still dead code: nothing
constructs a live global yet.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX"
```

---

### Task 5: Emit the closures and construct exported globals through the builtin

**Files:**
- Modify: `include/hermes/WasmIRGen/WasmHelpers.h:343-356`
- Modify: `lib/WasmIRGen/WasmHelpers.cpp:579-596`
- Modify: `include/hermes/WasmIRGen/WasmIRGen.h` (declare `createGlobalAccessor`)
- Modify: `lib/WasmIRGen/WasmIRGen.cpp:2043-2140` (the global export loop)
- Commit: `test/wasm/e2e-exported-mutable-global.wat`, `test/wasm/e2e-exported-mutable-global-driver.js_` (written in Task 1)

**Interfaces:**
- Consumes: `HermesBuiltin_wasmMakeGlobal` from Task 3; the closure-aware
  accessors from Task 4.
- Produces: `WasmHelpers::emitMakeGlobal(Value *valTypeCode, Value *isMutable,
  Value *valueOrGetter, Value *setterOrUndefined)` and
  `WasmIRGen::createGlobalAccessor(uint32_t globalIndex, bool isSetter,
  Instruction *tlScope)` returning `Function *`.

- [ ] **Step 1: Add the helper**

`include/hermes/WasmIRGen/WasmHelpers.h`, next to `emitGlobalSet`:

```cpp
  /// Emit the wasmMakeGlobal builtin call that builds the WebAssembly.Global
  /// published for an exported global. \p valueOrGetter is the getter closure
  /// for a live global and the snapshot value otherwise; \p setterOrUndefined
  /// is the setter closure for a live mutable global and undefined otherwise.
  Instruction *emitMakeGlobal(
      Value *valTypeCode,
      Value *isMutable,
      Value *valueOrGetter,
      Value *setterOrUndefined);
```

`lib/WasmIRGen/WasmHelpers.cpp`, next to `emitGlobalSet`:

```cpp
Instruction *WasmHelpers::emitMakeGlobal(
    Value *valTypeCode,
    Value *isMutable,
    Value *valueOrGetter,
    Value *setterOrUndefined) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmMakeGlobal,
      {valTypeCode, isMutable, valueOrGetter, setterOrUndefined});
}
```

- [ ] **Step 2: Add `createGlobalAccessor`**

Model it on `createExportWrapper` (`WasmIRGen.cpp:2353-2400`), which is the
existing template for a function closing over the top-level scope. Declare in
`WasmIRGen.h`:

```cpp
  /// Create the getter (or, with \p isSetter, the setter) closure for a live
  /// exported global. The closure reads or writes the global's frame slot
  /// directly, so the module's own global.get/global.set stay plain frame
  /// accesses and pay nothing for the export being live.
  /// \param globalIndex an index into the module's global index space.
  /// \return the created Function; the caller emits the CreateFunctionInst.
  Function *createGlobalAccessor(
      uint32_t globalIndex,
      bool isSetter,
      Instruction *tlScope);
```

Implement in `WasmIRGen.cpp` next to `createExportWrapper`:

```cpp
Function *WasmIRGen::createGlobalAccessor(
    uint32_t globalIndex,
    bool isSetter,
    Instruction *tlScope) {
  uint32_t numImportedGlobals = moduleInfo_.importedGlobalCount();
  assert(
      globalIndex >= numImportedGlobals &&
      "only a global this module defines gets accessors");
  const WasmGlobalType &gType =
      moduleInfo_.globals[globalIndex - numImportedGlobals].type;
  uint32_t slotIdx = globalSlotIndex_[globalIndex];

  auto *fn = builder_.createFunction(
      ("wasm_global_" + llvh::Twine(globalIndex) +
       (isSetter ? "_set" : "_get"))
          .str(),
      Function::DefinitionKind::ES5Function,
      true /* strictMode */);
  builder_.createJSThisParam(fn);
  if (isSetter)
    builder_.createJSDynamicParam(fn, "v");
  fn->setExpectedParamCountIncludingThis(isSetter ? 2 : 1);

  auto *entryBB = builder_.createBasicBlock(fn);
  builder_.setInsertionBlock(entryBB);
  auto *parentScope = builder_.createGetParentScopeInst(
      topLevelVS_, fn->getParentScopeParam());

  if (!isSetter) {
    auto *lo = builder_.createLoadFrameInst(
        parentScope, globalVars_[slotIdx]);
    if (gType.type == WasmValType::I64) {
      // An i64 global is stored as a lo/hi pair; JS sees a BigInt. This is
      // the same assembly the export path used to do inline.
      auto *hi = builder_.createLoadFrameInst(
          parentScope, globalVars_[slotIdx + 1]);
      builder_.createReturnInst(helpers_.emitI64ToBigInt(lo, hi));
    } else {
      builder_.createReturnInst(lo);
    }
    return fn;
  }

  // Index 0 is `this`; the first declared parameter is 1 (IR.h:1866, and
  // createExportWrapper uses `1 + i` for the same reason). Reading 0 here
  // would silently store the `this` the native callers pass -- undefined --
  // instead of the value.
  auto *param = builder_.createLoadParamInst(fn->getJSDynamicParam(1));
  if (gType.type == WasmValType::I64) {
    // The caller has already refused a non-BigInt. emitBigIntToI64 writes the
    // shared retBuf scratch view and this reads it back with nothing in
    // between that can run JS, so the buffer cannot be clobbered mid-use --
    // the same straight-line pattern initializeGlobals already relies on.
    // retBufIVar_ is always non-null: createFunctions() allocates it for
    // every module with an 8-byte minimum, precisely because a body may do
    // i64 arithmetic even when no signature mentions i64
    // (WasmIRGen.cpp:649-685), so no guard is needed here.
    auto *rbI = builder_.createLoadFrameInst(parentScope, retBufIVar_);
    helpers_.emitBigIntToI64(rbI, param);
    builder_.createStoreFrameInst(
        parentScope,
        builder_.createAsInt32Inst(builder_.createLoadPropertyInst(
            rbI, builder_.getLiteralNumber(0))),
        globalVars_[slotIdx]);
    builder_.createStoreFrameInst(
        parentScope,
        builder_.createAsInt32Inst(builder_.createLoadPropertyInst(
            rbI, builder_.getLiteralNumber(1))),
        globalVars_[slotIdx + 1]);
  } else {
    // The caller has already run ToNumber. Narrow to the declared Wasm type
    // with AsInt32Inst for i32 and emitFround for f32, matching
    // coerceImportedGlobalValue (WasmIRGen.cpp:7736). NOT the module's own
    // global.set, which narrows nothing (WasmIRGen.cpp:7884).
    Value *narrowed = param;
    if (gType.type == WasmValType::I32)
      narrowed = builder_.createAsInt32Inst(param);
    else if (gType.type == WasmValType::F32)
      narrowed = emitFround(builder_.createAsNumberInst(param));
    builder_.createStoreFrameInst(
        parentScope, narrowed, globalVars_[slotIdx]);
  }
  builder_.createReturnInst(builder_.getLiteralUndefined());
  return fn;
}
```

If `createReturnInst`, `getJSDynamicParam`, `createLoadParamInst`,
`getLiteralUndefined` or `emitFround` have different spellings in this tree,
copy the spelling `createExportWrapper` and `coerceImportedGlobalValue` use.
`createGlobalAccessor` changes the builder's insertion block, so the caller
must restore it — Step 3 does.

- [ ] **Step 3: Rewrite the global export loop**

In `WasmIRGen.cpp:2043-2140`, delete the `wasmGlobalCtor` /
`hasGlobalExports` block that loads `WebAssembly.Global` off the global
object, and delete the stale comment at `2050-2051` that says mutable globals
will not reflect later mutations. Keep the imported-mutable re-export branch
(`2081-2089`) exactly as it is. Replace the rest of the loop body with:

```cpp
    uint32_t numImportedGlobals = moduleInfo_.importedGlobalCount();
    WasmGlobalType gType{WasmValType::I32, false};
    if (exp.index < numImportedGlobals) {
      uint32_t idx = 0;
      for (const auto &imp : moduleInfo_.imports) {
        if (imp.kind != WasmExternalKind::Global)
          continue;
        if (idx == exp.index) {
          gType = imp.globalType;
          break;
        }
        ++idx;
      }
    } else {
      gType = moduleInfo_.globals[exp.index - numImportedGlobals].type;
    }

    // Only a MUTABLE global this module DEFINES is published live. An
    // immutable one cannot go stale, and an imported mutable one is
    // re-exported as the object it arrived as, above.
    bool live = gType.mutable_ && exp.index >= numImportedGlobals;

    Value *valueOrGetter = nullptr;
    Value *setterOrUndefined = builder_.getLiteralUndefined();
    if (live) {
      auto *getterFn = createGlobalAccessor(exp.index, false, tlScope);
      auto *setterFn = createGlobalAccessor(exp.index, true, tlScope);
      // createGlobalAccessor emits into its own function; restore the
      // insertion point before continuing to build the instantiate body.
      builder_.setInsertionBlock(tlEntry_);
      // Two arguments, not three: createCreateFunctionInst(scope, code)
      // (IRBuilder.h:320). The existing wrapper construction at
      // WasmIRGen.cpp:2311 is the model.
      valueOrGetter = builder_.createCreateFunctionInst(tlScope, getterFn);
      setterOrUndefined = builder_.createCreateFunctionInst(tlScope, setterFn);
    } else {
      uint32_t slotIdx = globalSlotIndex_[exp.index];
      valueOrGetter = builder_.createLoadFrameInst(
          tlScope, globalVars_[slotIdx]);
      if (gType.type == WasmValType::I64) {
        auto *hi = builder_.createLoadFrameInst(
            tlScope, globalVars_[slotIdx + 1]);
        valueOrGetter = helpers_.emitI64ToBigInt(valueOrGetter, hi);
      }
    }

    auto *globalObj = helpers_.emitMakeGlobal(
        builder_.getLiteralNumber(
            static_cast<double>(globalValTypeCode(gType.type))),
        builder_.getLiteralBool(gType.mutable_),
        valueOrGetter,
        setterOrUndefined);
    builder_.createStorePropertyStrictInst(
        globalObj, exportsObj, builder_.getLiteralString(exp.name));
```

The four-arm `switch` and its `llvm_unreachable` at `2124-2140` go away with
this rewrite. **That does not fix dz issue `01a074ce-ac97`** — a reference-typed
export now reaches `wasmMakeGlobal` with `globalValTypeCode` returning `0xFF`,
which the builtin refuses with `wasmMakeGlobal: unknown value type`. That is a
catchable TypeError instead of an abort, which is strictly better, but the
issue stays open: the correct behavior is a live `externref` Global, and the
descriptor/ValType work is not in this plan's scope. Note this in the commit
message and add a dz comment on `01a074ce-ac97` recording the new symptom.

- [ ] **Step 4: Stop snapshotting mutable imports at instantiation**

Required by this change, not a cleanup. `initializeGlobals()` calls
`wasmGlobalGet` for every mutable import (`WasmIRGen.cpp:7577`). Once an
exported mutable global is closure-backed, module B importing module A's
global would invoke **A's getter closure during B's instantiation**, even
when no Wasm code in B ever reads it.

Do NOT fix this by deleting the `emitGlobalGet` call alone. `resolvedVal`
would then still hold the `Global` OBJECT, which flows on into
`coerceImportedGlobalValue` and then, for i64, into `emitBigIntToI64` at
`WasmIRGen.cpp:7594`, which rejects it. Skip the whole per-import snapshot
instead — an early `continue` at the top of the imported-globals loop:

```cpp
  for (uint32_t i = 0; i < numImportedGlobals; ++i) {
    // A mutable import's frame slot is a link-time snapshot that no valid
    // module can read: Wasm validation refuses a mutable global.get
    // throughout constant-expression context, and every other reader
    // (global.get/global.set, the export loop) takes the object path first.
    // Writing it cost an eager wasmGlobalGet per import; once the exporting
    // module's global is closure-backed, that call would run the EXPORTER's
    // getter inside the IMPORTER's instantiation. Nothing needs it.
    //
    // "No valid module": hermesc --wasm does not validate (01a0460b-1ea9),
    // so an invalid AOT input can still reach a constant expression that
    // reads this slot. It will read the slot's initial value rather than a
    // snapshot; that is invalid-input behavior, not a correctness guarantee
    // being dropped.
    if (importedMutableGlobals_.count(i))
      continue;
    uint32_t slotIdx = globalSlotIndex_[i];
    ...
```

Keep the slot allocation and the `globalSlotIndex_` mapping exactly as they
are — later defined globals and i64 lo/hi pairs are positioned by it.

Expected effect on the golden IR: the `wasmGlobalGet` call and the coercion
and store that follow it disappear from `__wasm_instantiate__` for every
module with a mutable global import. `test/wasm/e2e-imported-mutable-global`
must still pass unchanged; if it does not, the snapshot was not dead and
this step is wrong.

- [ ] **Step 5: Build and run the Task 1 test**

```bash
cmake --build cmake-build-asan --target hermes hermesc
LIT_FILTER="e2e-exported-mutable-global" \
  cmake --build cmake-build-asan --target check-hermes
```

Expected: PASS. If `after wasm bump, host sees 5` still appears, the export
loop is still taking the snapshot branch — check `live`.

- [ ] **Step 6: Prove the test can fail**

A green test proves only that the suite ran. Do NOT try `bool live = false;`
— the loop still passes `gType.mutable_ == true` to `wasmMakeGlobal`, which
refuses a mutable snapshot, so instantiation throws instead of reproducing
the old behavior.
Instead break the wiring while keeping instantiation successful: in
`createGlobalAccessor`'s setter branch, store into a scratch `Variable`
instead of `globalVars_[slotIdx]` (or simply omit the `StoreFrameInst`).
Rebuild and confirm `e2e-exported-mutable-global` fails at
`after host set, wasm sees 100` — JS writes stop reaching the module while
everything else still runs. Restore it.

- [ ] **Step 7: Regenerate the auto-updating tests**

The emitted IR for every global export changed, so `%FileCheckOrRegen` tests
covering it are now stale — at least `test/wasm/compile-globals.wat`,
`test/wasm/irgen-globals.wat` and `test/wasm/e2e-exports.wat`.

```bash
cmake --build cmake-build-asan --target update-lit
git diff --stat test/
```

Review every regenerated diff. Each one should show the
`TryLoadGlobalPropertyInst "WebAssembly"` + `LoadPropertyInst "Global"` +
`CreateThisInst`/`GetConstructedObjectInst` sequence replaced by a single
`CallBuiltinInst [HermesBuiltin.wasmMakeGlobal]`, plus new
`wasm_global_N_get`/`_set` functions for mutable exported globals. Anything
else means something unintended changed — investigate before accepting.

- [ ] **Step 8: Run the whole suite, plus a handle-sanitizer run**

```bash
cmake --build cmake-build-asan --target check-hermes
```

Then prove the closure fields are really traced (the check Task 2 could not
make fail):

```bash
cmake -B cmake-build-sanhandles -G Ninja -DCMAKE_BUILD_TYPE=Debug \
  -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ \
  -DHERMES_ENABLE_WASM=ON \
  -DHERMESVM_SANITIZE_HANDLES=ON
cmake --build cmake-build-sanhandles --target hermes hermesc
LIT_FILTER="wasm" cmake --build cmake-build-sanhandles --target check-hermes
```

`-DHERMES_ENABLE_WASM=ON` is **required**: it defaults to OFF
(`CMakeLists.txt:288`), and `test/lit.cfg:143` only discovers `.wat` files
and defines the `wasm` feature when it is on — so without it this command
runs zero Wasm tests and passes, proving nothing.

Expected: PASS. Then prove the metadata registration is load-bearing:

- Remove one `mb.addField` line from Task 2 and rebuild the sanhandles build.
- Do not rely on `HERMESVM_SANITIZE_HANDLES` alone to force the failure: it
  moves the heap with probability 0.01 per allocation by default
  (`GCSanitizeRate`, `RuntimeFlags.h:44`), so a single short test may never
  collect at the moment the closure is the only reference. Raise it to 1 for
  the run with `-gc-sanitize-handles=1` -- that is the real option name, and
  `cmake-build-asan/bin/hermes -gc-sanitize-handles=1 x.js` accepts it -- or
  add a temporary driver line that allocates in a loop after the exported
  Global is the sole reference and then calls the getter.
- Confirm `e2e-exported-mutable-global` fails or crashes. Restore the line.

If neither approach makes it fail deterministically, say so in the commit
message rather than claiming coverage the run did not demonstrate.

- [ ] **Step 9: Commit**

```bash
git add include/hermes/WasmIRGen/WasmHelpers.h lib/WasmIRGen/WasmHelpers.cpp \
        include/hermes/WasmIRGen/WasmIRGen.h lib/WasmIRGen/WasmIRGen.cpp \
        test/wasm/e2e-exported-mutable-global.wat \
        test/wasm/e2e-exported-mutable-global-driver.js_ test/
git commit -m "Publish an exported mutable global as a live view, not a snapshot

An exported mutable global was a copy taken at instantiation: the module kept
writing its frame slot while JS read and wrote the Global's own field, so
writes were silently lost in both directions and no error was raised. The
module's frame slot is now the single source of truth, reached from JS through
getter/setter closures created over the module's scope, so the module's own
global.get/global.set are unchanged and pay nothing.

Construction moves to the wasmMakeGlobal builtin, which also closes an
interposition hole: the export path read globalThis.WebAssembly.Global, so
replacing it let script decide what a module's exported globals were. The
memory and table export paths already refused that.

A reference-typed global export now raises a catchable TypeError from
wasmMakeGlobal instead of aborting the process, but dz 01a074ce-ac97 stays
open: the correct behavior is a live externref Global.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX"
```

---

### Task 6: Pin the interposition fix with a test, and bump the bytecode version

**Files:**
- Create: `test/wasm/e2e-global-export-not-interposable.wat`
- Create: `test/wasm/e2e-global-export-not-interposable-driver.js_`
- Modify: `include/hermes/BCGen/HBC/BytecodeVersion.h:23`

**Interfaces:**
- Consumes: everything from Tasks 2-5.
- Produces: nothing later tasks use. This is the last task.

- [ ] **Step 1: Write the interposition regression test**

`test/wasm/e2e-global-export-not-interposable.wat`:

```wat
;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; What a module exports is not script's to choose. The global export path
;; used to reach the constructor with an ordinary property read of
;; globalThis.WebAssembly.Global, so replacing that function handed the
;; importer a forged object carrying the module's descriptor and value -- the
;; same class of hole as the retired __wasm_type__ forgery. The memory and
;; table export paths already refused it. Construction now goes through the
;; wasmMakeGlobal builtin, which has no property to interpose on.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-global-export-not-interposable-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (global (export "g") (mut i32) (i32.const 5))
  (global (export "c") i32 (i32.const 7)))

;; CHECK: g is a genuine Global = true
;; CHECK-NEXT: g was not hijacked = true
;; CHECK-NEXT: g.value = 5
;; CHECK-NEXT: c is a genuine Global = true
;; CHECK-NEXT: c.value = 7
;; CHECK-NEXT: done
```

`test/wasm/e2e-global-export-not-interposable-driver.js_`:

```js
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// JS driver for e2e-global-export-not-interposable.wat.

var args = hermescli.getScriptArgs();
var mod = WebAssembly.Module.fromHermesBytecode(hermescli.loadFile(args[0]));

var real = WebAssembly.Global;
WebAssembly.Global = function Fake(desc, v) {
  this.hijacked = true;
  this.desc = desc;
  this.v = v;
};
var ex = new WebAssembly.Instance(mod).exports;
WebAssembly.Global = real;

print('g is a genuine Global = ' + (ex.g instanceof real));
print('g was not hijacked = ' + (ex.g.hijacked === undefined));
print('g.value = ' + ex.g.value);
print('c is a genuine Global = ' + (ex.c instanceof real));
print('c.value = ' + ex.c.value);
print('done');
```

- [ ] **Step 2: Run it**

```bash
LIT_FILTER="e2e-global-export-not-interposable" \
  cmake --build cmake-build-asan --target check-hermes
```

Expected: PASS.

- [ ] **Step 3: Prove this test can fail too**

Do not use `git stash` — the stash stack is shared with the main checkout and
other worktrees. Instead, restore the pre-fix construction in place:

```bash
git show <task-5-commit>~1:lib/WasmIRGen/WasmIRGen.cpp > /tmp/old-irgen.cpp
cp lib/WasmIRGen/WasmIRGen.cpp /tmp/new-irgen.cpp
cp /tmp/old-irgen.cpp lib/WasmIRGen/WasmIRGen.cpp
cmake --build cmake-build-asan --target hermesc
LIT_FILTER="e2e-global-export-not-interposable" \
  cmake --build cmake-build-asan --target check-hermes    # must FAIL
cp /tmp/new-irgen.cpp lib/WasmIRGen/WasmIRGen.cpp
cmake --build cmake-build-asan --target hermesc
```

Expected on the middle run: `g was not hijacked = false`. That is the
evidence the test is load-bearing. The pre-fix behavior is also already
recorded in the dz comment on `01a079c6-fdd6`
(`exports.g.hijacked: true`) — cite whichever you actually ran.

- [ ] **Step 4: Bump `BYTECODE_VERSION`**

Task 3 inserted a builtin before the `JS_BUILTIN` block, which shifts builtin
ids — a bytecode-visible change. In
`include/hermes/BCGen/HBC/BytecodeVersion.h:23`:

```cpp
const static uint32_t BYTECODE_VERSION = 101;
```

dz task `01a0460c-4361` records that this bump must be the last change to
land and is already load-bearing for three earlier commits; this makes four.
Update that task with a comment naming this one:

```bash
dz comment 01a0460c-4361 -m "Also load-bearing for the wasmMakeGlobal builtin \
(live exported globals, dz 01a079c6-fdd6), which inserts a WASM_BUILTIN \
before the JS_BUILTIN block and shifts _firstJS."
```

- [ ] **Step 5: Full suite, both build configurations**

```bash
cmake --build cmake-build-asan --target check-hermes
cmake --build cmake-build-wasm-off --target hermes
```

Expected: `check-hermes` PASS; the WASM-off build links.

- [ ] **Step 6: Close the dz issue**

```bash
dz close 01a079c6-fdd6 --as fixed \
  -m "Fixed by the live-exported-globals series. An exported mutable global is \
now a two-way view backed by getter/setter closures over the module's frame \
slot; construction goes through the wasmMakeGlobal builtin, which also closes \
the globalThis.WebAssembly.Global interposition hole recorded in the comment. \
Covered by test/wasm/e2e-exported-mutable-global.wat and \
test/wasm/e2e-global-export-not-interposable.wat."
```

- [ ] **Step 7: Commit**

```bash
git add test/wasm/e2e-global-export-not-interposable.wat \
        test/wasm/e2e-global-export-not-interposable-driver.js_ \
        include/hermes/BCGen/HBC/BytecodeVersion.h dz/
git commit -m "Pin the global-export interposition fix and bump BYTECODE_VERSION

Adds the regression test for script replacing globalThis.WebAssembly.Global,
and bumps the bytecode version for the wasmMakeGlobal builtin, which shifted
the JS builtin ids.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX"
```

---

## Out of scope

Recorded so nobody folds them in:

- **dz `01a074ce-ac97`** — reference-typed and v128 global exports. Task 5
  turns the abort into a catchable TypeError as a side effect, which is an
  improvement but not the fix. The remaining work is the `ValType` codes, the
  descriptor plumbing and the raw-import path. This plan helps only narrowly:
  for a MUTABLE global a module defines and exports, the value lives in the
  frame slot, so that one case needs no reference-capable value slot.
  Reference-capable storage is still required for immutable exports and for
  globals built by the public constructor, so the cell-layout work does not
  go away.
- **The reference-typed global *import* defect** (recorded in the
  `01a074ce-ac97` investigation comment): `globalValTypeCode` returns `0xFF`
  for reference types and the raw fallback demands `typeof === "number"`, so
  such an import can never link. Untouched here.
- **Object identity of a re-exported immutable import.** Measured: Hermes
  `false`, Node `true`, while Hermes gets the mutable case right. The fix is
  not "publish the imported object for every imported global" — an immutable
  import may be a raw Number or BigInt with no object to publish
  (`rawAllowed = !mutable_`). Filed as `01a079fe-a227`.
- **Two export names of one global returning two objects.** Required to be
  one object by the JS API's per-agent Global object cache; Hermes and Node
  both get it wrong. Deferred; filed as `01a079ff-5dc0`. This plan requires only STORAGE
  coherence across the two names, which Task 1's test asserts.
- **The i64 extended-constant initializer defect** — `(i64.add (i64.const 1)
  (i64.const 2))` yields `2`. i32 is correct. Filed as `01a079fe-eff6`.
- **The externref table export interposition** — the same
  replaceable-constructor hole as globals, on a different path
  (`WasmIRGen.cpp:2250`, `2274`). Task 6's test covers globals only. Filed as
  `01a079ff-5d7b`.
- **Retention as a tested property.** A live Global strongly retains its
  captured `Environment` and can thereby retain other globals, the memory,
  the tables, imports and closures — whatever the optimizer leaves reachable.
  Task 5 Step 8 exercises it under `HERMESVM_SANITIZE_HANDLES` but nothing
  measures heap growth. Follow up if it shows.
