# Wasm Linking ABI on Internal Slots — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the WebAssembly linking ABI off script-visible JS properties onto native-only state, so the objects crossing a module boundary cannot be forged, mutated or enumerated by script.

**Architecture:** A canonical Exported Function per Wasm function carries its backing closure and interned type id in named internal properties. Tables keep three parallel arrays written through two native funnel helpers. The `__wasm_*` publications are deleted and every read of them is replaced by a brand-checking native builtin.

**Tech Stack:** C++17, Hermes VM (`lib/VM/JSLib/WebAssembly/`), Wasm IR generator (`lib/WasmIRGen/`), lit tests under `test/wasm/`.

**Design doc:** `doc/superpowers/specs/2026-08-23-wasm-internal-slots-design.md`. Read it before Task 1.

## Global Constraints

- Branch is `wasm-old-rebased`. Do **not** rebase, merge, switch branches, or open a PR. Commit to this branch.
- Build and test with the ASan build at `cmake-build-asan` (already configured). Single tests via `LIT_FILTER="wasm" cmake --build cmake-build-asan --target check-hermes` — never a hand-rolled wrapper script.
- Full-suite baseline at the start of this plan (`59c96e2da`) is **4483 passed / 16 expectedly failed / 0 unexpectedly failed**. No task may finish below it.
- **Every task ends green.** Where an intermediate state would break cross-module linking, the task carries explicit temporary scaffolding, called out in that task.
- **Mutation proof is a hard gate on every task.** For each new assertion, break the fix in a targeted way, show a *named* check fails, restore. Report the mutation and the check that caught it. An assertion that cannot be made to fail is not coverage — fix the test.
- Interned type ids (`typeIdVars_`) only. A module-local type index is wrong across modules: the same signature may be numbered differently (spurious trap) and different signatures identically (missed trap).
- Reference types on this branch: `ref.func` in a function body and `call_ref` are unsupported and must stay so. Use `table.get` to obtain a funcref in tests.
- `V128` remains unsupported and keeps its existing diagnostic everywhere.
- Runtime code follows `CLAUDE.md`'s GC rules — `Locals`/`PinnedValue`, no raw pointers or `PseudoHandle`s across safepoints. Invoke the `gc-safe-coding` skill before editing `lib/VM/`.
- Golden IR churn is expected. `cmake --build cmake-build-asan --target update-lit` regenerates, but **read the diff** — do not regenerate blindly.
- Out of scope for the whole plan: the `BYTECODE_VERSION` bump (queued last on this branch), the Worker gate (after the `static_h` rebase), `ref.func`/`call_ref`, V128.

---

### Task 1: The canonical Exported Function

Creates one Exported Function per Wasm function index, carrying its closure and type id in internal properties. No ABI is removed yet; this only makes the object exist and be canonical.

**Files:**
- Modify: `include/hermes/VM/InternalProperties.def`
- Modify: `lib/WasmIRGen/WasmIRGen.cpp` (`createFunctions()` ~`:412`, export publication ~`:1955-1990`, `createExportWrapper()` ~`:2251`)
- Modify: `include/hermes/WasmIRGen/WasmIRGen.h`
- Test: `test/wasm/e2e-exported-function-identity.wat` + `-driver.js_`

**Interfaces:**
- Produces: `exportedFuncVars_` — a `std::vector<Variable *>` in the top-level scope, one per function index (null where no wrapper is needed), holding the canonical Exported Function. Tasks 2–4 read this.
- Produces: internal properties `WasmFuncClosure` and `WasmFuncTypeId` on each wrapper. Task 2's funnel and Task 3's brand check read them.

- [ ] **Step 1: Add the internal property names**

In `include/hermes/VM/InternalProperties.def`, alongside `NAMED_PROP(NativeState)`:

```cpp
NAMED_PROP(WasmFuncClosure)
NAMED_PROP(WasmFuncTypeId)
```

- [ ] **Step 2: Write the failing test**

`test/wasm/e2e-exported-function-identity.wat` — a module exporting one function under two names and also placing it in a table:

```wasm
(module
  (table (export "tbl") 1 funcref)
  (elem (i32.const 0) $f)
  (func $f (export "a") (export "b") (result i32) (i32.const 7)))
```

Driver asserts all three views are the *same object*:

```js
print('a === b: ' + (inst.exports.a === inst.exports.b));
print('a === tbl.get(0): ' + (inst.exports.a === inst.exports.tbl.get(0)));
```

Expected after this task: `a === b: true`. The `tbl.get(0)` line is pinned by Task 2 — in this task assert only the export-identity line, and add the table line in Task 2 rather than writing an assertion that cannot yet hold.

- [ ] **Step 3: Run it and confirm it fails**

`LIT_FILTER="exported-function-identity" cmake --build cmake-build-asan --target check-hermes`
Expected: FAIL, `a === b: false` — today `createExportWrapper` is called once per *export entry* (`:1963-1972`).

- [ ] **Step 4: Create wrappers per function index**

In `createFunctions()`, after `computeEscapableFuncs()`, allocate `exportedFuncVars_` for every index in `escapableFuncs_ ∪ {exported function indices} ∪ {imported function indices}`. In the instantiate body, create each wrapper once via the existing `createExportWrapper` machinery, then stamp it:

- `WasmFuncClosure` ← `closureVars_[funcIndex]` (for an imported index this is the import trampoline)
- `WasmFuncTypeId` ← `typeIdVars_[canonicalTypeIndex_[typeIdx]]`

Change the export publication loop to look up `exportedFuncVars_[exp.index]` instead of calling `createExportWrapper` per entry.

- [ ] **Step 5: Run the test and the suite**

Expected: `a === b: true`; suite ≥ 4483/16/0.

- [ ] **Step 6: Mutation proof**

Revert Step 4's lookup to a per-entry `createExportWrapper` call; confirm `e2e-exported-function-identity.wat` fails on the `a === b` check by name. Restore.

- [ ] **Step 7: Commit**

---

### Task 2: The table's third array and the write funnel

Adds `exported_`, routes every writer through two helpers, and makes `onTableGet` yield the wrapper. This is where H16's wrong-signature call and the `table.set`/`table.fill` stale-types defects are fixed.

**Files:**
- Modify: `include/hermes/VM/JSWebAssemblyTable.h` (add `exported_` beside `elements_`/`types_`)
- Modify: `include/hermes/FrontEndDefs/Builtins.def`
- Modify: `lib/VM/JSLib/HermesBuiltin.cpp`
- Modify: `include/hermes/WasmIRGen/WasmHelpers.h`, `lib/WasmIRGen/WasmHelpers.cpp`
- Modify: `lib/WasmIRGen/WasmIRGen.cpp` — `onTableGet` `:6982`, `onTableSet` `:6993`, `onTableGrow` `:7014`, `onTableFill` `:7072`, `onTableCopy` `:7085`, `onTableInit` `:7104`, element-segment init `:6892`, import adoption `:1241`, export publication `:2237`
- Test: `test/wasm/e2e-table-slot-invariant.wat` + `-driver.js_`; extend `test/wasm/e2e-exported-function-identity.wat`

**Interfaces:**
- Consumes: `exportedFuncVars_`, `WasmFuncClosure`, `WasmFuncTypeId` from Task 1.
- Produces: `PRIVATE_BUILTIN(wasmTableSetSlot)` — `(table, index, exportedFnOrNull) -> undefined`; derives closure and type id from the wrapper's internal properties, writes all three arrays. `PRIVATE_BUILTIN(wasmTableCopySlots)` — `(dstArrays, dstIdx, srcArrays, srcIdx, n) -> undefined`. Tasks 3 and 4 call both.

**Temporary scaffolding (removed in Task 4):** this task also publishes `__wasm_exported__` next to `__wasm_funcs__`/`__wasm_types__`, so a module importing another module's table adopts all three arrays and cross-module tests stay green. Task 4 deletes all three publications together.

- [ ] **Step 1: Write the failing tests**

`test/wasm/e2e-table-slot-invariant.wat`, built from the reproduced defect (`handoff-artifacts/h16b.wat`):

```wasm
(module
  (type $t0 (func (result i32)))
  (table (export "tbl") 2 funcref)
  (elem (i32.const 0) $a $b)
  (func $a (result i32) (i32.const 7))
  (func $b (param i32 i32 i32) (result i32) (local.get 0))
  (func (export "callAsT0") (param i32) (result i32)
    (call_indirect (type $t0) (local.get 0)))
  ;; wasm-side copy: table.get then table.set
  (func (export "copySlot") (param i32 i32)
    (local.get 1) (local.get 0) (table.get 0) (table.set 0))
  ;; wasm-side fill
  (func (export "fillSlot") (param i32 i32)
    (local.get 0) (local.get 1) (table.get 0) (i32.const 1) (table.fill 0)))
```

Assertions:

```
callAsT0(0) -> 7
callAsT0(1) -> trap                       ; correct type check, unchanged
copySlot(0, 1); callAsT0(1) -> 7          ; was: type mismatch  (table.set)
fillSlot(1, 0); callAsT0(1) -> 7          ; was: type mismatch  (table.fill)
copySlot(1, 0); callAsT0(0) -> trap       ; $b under $a's slot must NOT be callable
```

The last line is the security assertion: it must trap, not return `undefined`.

- [ ] **Step 2: Run and confirm each line fails as predicted**

Expected today: lines 3 and 4 report `call_indirect: type mismatch`; line 5 returns `undefined` instead of trapping.

- [ ] **Step 3: Add `exported_` to the table**

Mirror `elements_`/`types_` exactly in `JSWebAssemblyTable.h` — same `GCPointer<JSArray>` shape, same getter/setter naming. Add `tableExportVars_` in `WasmIRGen.h` mirroring `tableTypeVars_`.

- [ ] **Step 4: Add the two funnel builtins**

`PRIVATE_BUILTIN(wasmTableSetSlot)` and `PRIVATE_BUILTIN(wasmTableCopySlots)` in `Builtins.def`; implement in `HermesBuiltin.cpp` next to the existing `wasmTable*` builtins; emit from `WasmHelpers`. `wasmTableSetSlot` with a null value clears all three slots. A non-null value that lacks `WasmFuncClosure` is a `TypeError` — Task 3 relies on this.

- [ ] **Step 5: Convert every writer**

Route all nine writer sites listed under **Files** through the two helpers. No site may write an array directly. Change `onTableGet` to read `exported_[i]`.

- [ ] **Step 6: Run the tests and the suite**

All five assertions pass; `e2e-table-import-advanced` still passes (cross-module sharing); suite ≥ 4483/16/0.

Note what Step 5 gets for free: once `onTableGet` yields the wrapper, a funcref
travelling anywhere else — a funcref result through the return buffer's reference
slots (added by `e042066fa`), a funcref global, an argument handed to an import
trampoline — is *already* a wrapper, because those all carry whatever the value
stack held. No separate conversion code is needed at those sites. Confirm this
rather than assume it: `test/wasm/mv-ref-result.wat` and
`e2e-mv-ref-wasm-to-wasm.wat` must still pass, and what they return must now be a
wrapper. If either regresses, the value-stack assumption in the design is wrong
and the fix belongs here, not in Task 6.

- [ ] **Step 7: Extend the identity test**

Add the `a === tbl.get(0)` line deferred from Task 1; it must now hold.

- [ ] **Step 8: Mutation proof**

At minimum: (a) make `wasmTableSetSlot` skip the types write → the `copySlot` line fails; (b) make it skip the `exported_` write → the identity test fails; (c) restore `onTableGet` to read `elements_[i]` → identity fails; (d) make the security line's slot writable without a type update → that line fails. Name the failing check each time. Restore.

- [ ] **Step 9: Commit**

---

### Task 3: JS-side Table methods

Brings `WebAssembly.Table.prototype` in line with the spec and the new representation.

**Files:**
- Modify: `lib/VM/JSLib/WebAssembly/WebAssembly.cpp` — `Table` constructor ~`:1607-1760`, `wasmTableSetMethod` `:1861`, `wasmTableGrowMethod` ~`:1920`, and the `get` method
- Test: `test/wasm/e2e-table-js-methods.js`

**Interfaces:** Consumes `wasmTableSetSlot`/`wasmTableCopySlots` and the internal properties.

- [ ] **Step 1: Confirm the spec predicate**

Before writing the TypeError test, check the JS-API text for `ToWebAssemblyValue` on `funcref`: the design doc states from memory that it requires an Exported Function or null. **Verify this**; if the spec differs, follow the spec and record the correction in `handoff-artifacts/REVIEW.md`.

- [ ] **Step 2: Write the failing test**

```js
tbl.set(0, function () { return 1; });   // spec: TypeError
tbl.set(0, null);                        // allowed
tbl.set(0, inst.exports.f);              // allowed; tbl.get(0) === inst.exports.f
tbl.get(0)(...)                          // callable, correct results, no abort
tbl.grow(1); tbl.get(newIdx)             // null, not a stale closure
```

- [ ] **Step 3: Run and confirm it fails**

Today the plain-function line is accepted and `call_indirect` will call it.

- [ ] **Step 4: Implement**

`get` returns `exported_[i]`. `set` brand-checks for `WasmFuncClosure` and raises `TypeError` naming the method, else calls the funnel. The constructor allocates three arrays. `grow` extends all three with nulls.

- [ ] **Step 5: Run tests and suite; then mutation proof**

Remove the brand check → the TypeError line fails. Return `elements_[i]` from `get` → the identity line fails. Restore.

- [ ] **Step 6: Commit**

---

### Task 4: Delete the table publication; add `wasmLinkTable`

The task that actually closes the abort and the forgeable type ids.

**Files:**
- Modify: `lib/VM/JSLib/WebAssembly/WebAssembly.cpp` `:1749,1756`
- Modify: `lib/WasmIRGen/WasmIRGen.cpp` `:2237,2242` (publication), `:1092-1115`, `:1188-1241`, `:6834-6836` (reads)
- Modify: `Builtins.def`, `HermesBuiltin.cpp`, `WasmHelpers.*`
- Test: `test/wasm/e2e-table-abi-private.js`; move `handoff-artifacts/h16d.js` in as a regression test

**Interfaces:** Produces `PRIVATE_BUILTIN(wasmLinkTable)` — `(importVal, expectedMin, expectedMax) -> [funcs, types, exported]`, `dyn_vmcast<JSWebAssemblyTable>` or LinkError.

- [ ] **Step 1: Write the failing tests**

```js
Object.getOwnPropertyNames(tbl)        // spec: [] — today lists 5 names
tbl.__wasm_funcs__                     // undefined
tbl.__wasm_funcs__[0](5n)              // TypeError on undefined, NOT a VM abort
new WebAssembly.Instance(mod, {e: {t: {__wasm_funcs__: [], __wasm_types__: []}}})
                                       // LinkError — a literal must not link
```

- [ ] **Step 2: Run and confirm — expect the abort**

The third line aborts the VM today (`Assertion 'isDouble()' failed`). An aborting test run *is* the failure signal here; note it explicitly.

- [ ] **Step 3: Add `wasmLinkTable` and convert the four reads**

Replace the `__wasm_funcs__`-existence branch at `:1092-1115` with the brand check. Preserve its meaning: it distinguishes a genuine Table from a forged literal, and both Wasm-exported and JS-constructed Tables are genuine.

- [ ] **Step 4: Delete all three publications**

Including Task 2's temporary `__wasm_exported__`.

- [ ] **Step 5: Run tests and suite**

`e2e-table-import-advanced` is the critical regression: cross-module sharing must survive the switch to internal fields.

- [ ] **Step 6: Golden IR**

The link path changes shape. Run `update-lit`, then **read** the diff and confirm each change is the expected builtin call rather than a lost check.

- [ ] **Step 7: Mutation proof**

Weaken `dyn_vmcast` to an unchecked `vmcast` → the forged-literal test must fail (it will crash rather than LinkError; that still counts, note it). Re-add one publication → the `getOwnPropertyNames` test fails. Restore.

- [ ] **Step 8: Commit**

---

### Task 5: Memory and Global

Same treatment for the other two kinds; absorbs merge items 23/24.

**Files:**
- Modify: `lib/VM/JSLib/WebAssembly/WebAssembly.cpp` (7 publication sites for `__wasm_type__`/`__wasm_min__`/`__wasm_max__`)
- Modify: `lib/WasmIRGen/WasmIRGen.cpp` (10 read sites)
- Modify: `Builtins.def`, `HermesBuiltin.cpp`, `WasmHelpers.*`
- Test: `test/wasm/e2e-wasm-abi-conformance.js`

**Interfaces:** Produces `wasmLinkMemory(importVal, expectedMin, expectedMax)` and `wasmLinkGlobal(importVal, expectedType, expectedMutable)`, both brand-checking, both raising LinkError in the message style of `2f7b135e6`.

- [ ] **Step 1: Write the failing conformance test**

For each of Memory, Table, Global: `Object.keys`, `getOwnPropertyNames`, `getOwnPropertySymbols` are all empty and `JSON.stringify` yields `{}`. Plus a forged literal for each kind must raise LinkError.

- [ ] **Step 2: Run and confirm it fails** — today the metadata names are listed.

- [ ] **Step 3: Implement the two builtins, convert the reads, delete the publications**

Limits now come from the internal fields at use time, which is what dissolves H7 (limits were snapshots never updated by `grow`) — add an assertion that a grown memory's limit is observed by a later check.

- [ ] **Step 4: Run tests and suite; regenerate and read goldens**

- [ ] **Step 5: Mutation proof, then commit**

---

### Task 6: Retire the J4 coercion

Only after an escape-route test exists.

**Files:**
- Modify: `lib/WasmIRGen/WasmIRGen.cpp` `:740-768` (parameter typing), `:2764`, `computeEscapableFuncs()` `:380-410`
- Test: `test/wasm/e2e-no-closure-escape.js`

- [ ] **Step 1: Write the escape-route enumeration test FIRST**

Every route by which a function value can reach script — table slots, `Table.prototype.get`, export wrappers, funcref results including multi-value return slots, funcref globals, import trampoline arguments — is exercised, and each result is asserted to be a wrapper (has the brand, calling it with JS values behaves per spec) and never a raw closure. This test must pass *before* the coercion is removed.

- [ ] **Step 2: Remove the coercion**

Float params return to `:number` for all functions; `escapableFuncs_`'s role in parameter typing goes away. Keep the set if other code still uses it; delete it if not.

- [ ] **Step 3: Prove the removal is safe, not just green**

Re-run the J4 crash repro (float param, JS-supplied non-number). It must be unreachable because no route yields a closure — not merely because a coercion is still present somewhere.

- [ ] **Step 4: Update the comments**

`:740-761` describes the interim and says "REVISIT THEN". Replace with what is now true; do not leave a stale rationale.

- [ ] **Step 5: Run the suite; mutation proof; commit**

---

## Closing out

- [ ] Update `handoff-artifacts/REVIEW.md`: §4.4 and §5.8 resolved; H16 closed with all five symptoms; J4 closed; note the `table.fill` instance found during design.
- [ ] Update `handoff-artifacts/MERGE-TRIAGE.md`: rows 23/24 satisfied by the brand checks.
- [ ] Confirm the full suite one final time and record the numbers.
