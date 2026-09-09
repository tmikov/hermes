# Wasm Reference Types Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `externref` and `funcref` work end to end in Hermes's
WebAssembly implementation — globals, opcodes, element expressions, the
JS/Wasm conversion boundaries, exception payloads, and multi-value reference
results.

**Architecture:** A Wasm value slot cannot currently hold a JS reference.
`JSWebAssemblyGlobal`'s two scalar fields collapse into one traced
`GCHermesValue`; `ValType` gains two enumerators; the link protocol stops
using values as sentinels; two unsupported opcodes are implemented; and
multi-value reference results move to private per-activation storage. The
module side already stores JS values in frame slots, so most of the work is
at the JS-facing boundaries.

**Tech Stack:** C++17 (no exceptions, no RTTI), Hermes VM cells and
`Metadata::Builder`, Hermes IR / `WasmIRGen`, wabt, lit + FileCheck, gtest.

**Spec:** `doc/superpowers/specs/2026-09-08-wasm-reference-types-design.md`
— revision 7, approved after seven review rounds. **Read it before starting
and read the named section before each task.** The plan gives files, call
sites, verification and traps; the spec gives the design and the reasoning,
which is not duplicated here.

## Global Constraints

- **Build with clang:** `-DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++`.
- **Default build is ASan+Debug at `-O1`**, directory `cmake-build-asan`.
- **C++17, no exceptions, no RTTI.** 80-column lines, 2-space indent.
  Members take a trailing underscore. Doc comment on every declaration.
- **Runtime code uses `Locals` + `PinnedValue`.** No `GCScope`, no
  `makeHandle()`. No raw GC pointer across a safepoint. Every `CallResult`
  checked. Invoke the `gc-safe-coding` skill before writing runtime code.
- **`isWasmExportedFunction` ALLOCATES.** It reaches
  `HiddenClass::findPropertyNoMap`, which initializes a missing property map.
  Root or re-derive across every call. This is spec section 3's central
  correction and the single most likely source of a latent bug in this work.
- **Every validation added is funcref-only.** A check on an externref path is
  a bug, not extra safety: any JS value is a valid externref.
- **Do NOT bump `BYTECODE_VERSION`.** Task 2 adds a builtin; record it on dz
  `01a0460c-4361`, which keeps the branch's single deferred bump last.
- **Run single lit tests with `LIT_FILTER`**, never a hand-rolled wrapper.
- **Every commit leaves `check-hermes` green.** Baseline at the start of this
  plan: **4540 expected passes / 16 expected failures / 197 unsupported /
  0 unexpected.**
- **Commit message trailer:**
  ```
  Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01D6XJdZGuztqfdD4WwqpJAX
  ```

## How to extract a task brief

Each task below is handed to a fresh implementer with no other context. The
extraction MUST prepend, to every brief:

1. the **Global Constraints** section above, verbatim;
2. the verification recipe — build `cmake-build-asan`, run `check-hermes`,
   compare against the stated baseline, and use `LIT_FILTER` for single
   tests;
3. the path to the spec and the task's named section.

A task's own text says "Build, run the suite, commit" on the assumption that
this happened. Without it the brief is incomplete and the implementer will
guess at the build directory.

## Ordering constraints that are not negotiable

Two steps must precede others or the result is a silent miscompile:

- **Task 6's nullable annotation precedes Task 6's opcode.** With
  `Type::createObject()`, `InstSimplify` folds a strict comparison against
  `null` to `false`, so `ref.is_null` on a typed local returns 0 forever, in
  optimized builds only. A test using a literal `ref.null` passes anyway.
- **Task 2's predicate builtin precedes the GENERATED-IR checks in Tasks 5,
  8 and 9.** Those three emit IR that must brand-check and have nothing else
  to call. Task 3's check is native C++ and simply calls the existing
  `isWasmExportedFunction` helper — the builtin delegates to that same helper,
  so a native caller creates no second notion of the brand.
- **Task 1 precedes Tasks 3 and 4 completely.** Task 1 migrates the storage
  representation *and every one of its readers and writers*; it does not leave
  a half-changed accessor for a later task to finish. See Task 1's own note.

**A third constraint, learned from review:** two tasks must update existing
tests that assert today's wrong behaviour. Task 7 removes a warning
`irgen-unsupported-warn.wat` requires; Task 8 rejects a value
`e2e-mv-ref-import` requires to work. Neither is optional and neither is a
regression — the tests encode the defects.

---

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `include/hermes/VM/JSWebAssemblyGlobal.h`, `lib/VM/JSWebAssemblyGlobal.cpp` | One traced `GCHermesValue` replaces the two scalar fields; `ValType` gains `ExternRef`/`FuncRef`. | 1 |
| `include/hermes/FrontEndDefs/Builtins.def`, `include/hermes/VM/PredefinedStrings.def` | Declare the Exported-Function predicate and its name. | 2 |
| `lib/VM/JSLib/HermesBuiltin.cpp` | The predicate; `wasmMakeGlobal`'s discriminator; `wasmGlobalSet`'s dispatch; the storage funnel; `wasmLinkGlobal`'s protocol; the disabled-build registration range. | 2,3,4,5 |
| `lib/VM/JSLib/WebAssembly/WebAssembly.cpp` | Constructor descriptors; `.value` getter and setter dispatch. | 3,4 |
| `lib/WasmIRGen/WasmIRGen.cpp` | `globalValTypeCode`; the link path's raw-value arms; the nullable IR annotation; `ref.is_null`; `ref.func`; conversion points; `onCatch`; reference-result transport; element segments; the v128 diagnostic. | 1,5,6,7,8,9,10,11,12 |
| `lib/WasmFrontend/BinaryReaderHermesIRGen.cpp` | `ref.func`/`ref.is_null` dispatch; element-expression entries; the `flags == 3` misclassification. | 6,7,11 |
| `include/hermes/WasmFrontend/WasmTypes.h` | `WasmElemSegment`'s item model. | 11 |
| `test/wasm/*.wat`, `test/wasm/*-driver.js_` | Acceptance tests per task. | all |

---

### Task 1: Storage — one traced value slot, and two new ValType enumerators

**Spec:** section 1, "Storage" and "ValType".

**Files:**
- Modify: `include/hermes/VM/JSWebAssemblyGlobal.h`
- Modify: `lib/VM/JSWebAssemblyGlobal.cpp`
- Modify: `lib/VM/JSLib/WebAssembly/WebAssembly.cpp` — the constructor, the
  public setter and the snapshot getter, all of which touch the fields
- Modify: `lib/VM/JSLib/HermesBuiltin.cpp` — `wasmMakeGlobal`, the internal
  setter, `wasmGlobalGet` and `wasmLinkGlobal`, likewise
- Modify: `lib/WasmIRGen/WasmIRGen.cpp` (`globalValTypeCode` only)

**Interfaces:**
- Consumes: nothing.
- Produces: `ValType::ExternRef` (4) and `ValType::FuncRef` (5); a
  `GCHermesValue value_` replacing `double value_` and `int64_t i64Value_`;
  `globalValTypeCode` returning 4 and 5 instead of `0xFF`.

**This task is behaviour-PRESERVING, not inert, and it owns every consumer of
the fields it changes.** No reference value exists yet, but the i64
representation changes from a scalar to a `BigIntPrimitive*`, and the current
accessors cannot survive that:

```cpp
void setI64Value(int64_t val) { ... }   // no Runtime &, no failure path
```

Its four callers perform unchecked scalar writes — the constructor and the
public setter in `WebAssembly.cpp`, the snapshot builtin and the internal
setter in `HermesBuiltin.cpp` — and **two of them call it for non-i64 globals
too**, before overwriting the separate numeric field. An allocating store
needs a `Runtime &` and can fail, so all four migrate **in this commit**.
Deferring them to Tasks 3 and 4 does not compile.

Port them mechanically: same observable behaviour, new representation. The
reference *semantics* arrive in Tasks 3-5.

- [ ] **Step 1: Replace the two fields with one**

Delete `double value_` and `int64_t i64Value_`; add
`GCHermesValue value_;`. Initialize it in the constructor. Register it:
`mb.addField("value", &self->value_);` beside the two closure fields.

Doc-comment it with the canonical-representation table from the spec: which
`valType_` implies which slot content. That table is the invariant the rest
of the plan relies on.

- [ ] **Step 2: Convert the accessors**

`getValue`/`setValue`/`getI64Value`/`setI64Value` are replaced by accessors
over the single slot. **i64 becomes an allocating store**: the value is a
`BigIntPrimitive*` wrapped to 64 bits, so a setter takes a `Runtime &`, roots
its destination across the allocation, and can fail.

**Migrate all four call sites here**, plus all THREE snapshot readers — the
public `.value` getter, `wasmGlobalGet` and `wasmLinkGlobal` — each of which
today special-cases i64 and encodes everything else as a Number. Two callers
currently invoke `setI64Value` unconditionally for non-i64 globals; that
pattern goes away rather than being carried forward.

Wrapping stays at store time so the slot is canonical:
`2n**100n + 5n` stores as `5n`, as it does today via
`truncateToSingleDigit`.

- [ ] **Step 3: Extend ValType**

Append `ExternRef = 4` and `FuncRef = 5`. Keep 0-3 and the existing
`static_assert`s. **No `V128` enumerator** — Task 12 diagnoses v128 before it
reaches the runtime.

- [ ] **Step 4: Update `globalValTypeCode` and find the other consumers**

`WasmIRGen.cpp`'s `globalValTypeCode` gains two arms and stops returning
`0xFF` for reference types.

`-Wswitch` will name `setWasmGlobalNumber`, which has no `default:`. It is
**not** the only consumer: `wasmMakeGlobal` range-checks with
`rawCode > ValType::F64`, an ordering comparison the compiler cannot flag.
Grep for every use of `ValType` and list them in your report — the compiler's
silence is not coverage.

- [ ] **Step 5: Build and run the suite**

```bash
cmake --build cmake-build-asan --target hermes hermesc
cmake --build cmake-build-asan --target check-hermes
```

Expected: 4540 passes, 0 unexpected. The i64 tests are the ones that matter —
they exercise the representation change.

Add the **store-path** test the spec requires, which existing coverage does
not give: an i64 global written repeatedly through both setters so the
allocating store runs many times. Arithmetic wrapping
(`2n**100n + 5n -> 5n`) is a separate assertion and does not exercise
allocation failure or rooting.

**`-gc-sanitize-handles=1` does nothing in `cmake-build-asan`**, which has
`HERMESVM_SANITIZE_HANDLES=OFF` — the flag is ignored at runtime, so the test
reports green without exercising anything. Build the sanitizing configuration
and run it there:

```bash
cmake -B cmake-build-sanhandles -G Ninja -DCMAKE_BUILD_TYPE=Debug \
  -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ \
  -DHERMES_ENABLE_WASM=ON -DHERMESVM_SANITIZE_HANDLES=ON
cmake --build cmake-build-sanhandles --target hermes hermesc
LIT_FILTER="wasm" cmake --build cmake-build-sanhandles --target check-hermes
```

`-DHERMES_ENABLE_WASM=ON` is required: it defaults OFF, and `test/lit.cfg`
only discovers `.wat` when it is on, so without it the command runs zero Wasm
tests and passes.

- [ ] **Step 6: Commit**

---

### Task 2: The Exported-Function predicate builtin

**Spec:** section 1, subsection "The link path" — "The raw-value rule,
and a new predicate builtin".

**Files:**
- Modify: `include/hermes/FrontEndDefs/Builtins.def`
- Modify: `include/hermes/VM/PredefinedStrings.def`
- Modify: `lib/VM/JSLib/HermesBuiltin.cpp`

**Interfaces:**
- Consumes: nothing.
- Produces: a builtin taking one value and returning a boolean — whether it
  is a WebAssembly Exported Function. Tasks 5 and 8 emit calls to it.

Generated IR currently has no way to brand-check a function:
`isWasmExportedFunction` is a C++ helper, and `wasmSetFuncInfo` **stamps** the
brand without querying it.

- [ ] **Step 1: Declare it** in `Builtins.def`, immediately after
`WASM_BUILTIN(wasmMakeGlobal)` and before the `JS_BUILTIN` block, with a
comment saying why it exists (generated IR cannot call the C++ predicate).

- [ ] **Step 2: Implement it** next to `isWasmExportedFunction`, delegating
to it so the two notions of the brand cannot drift. **It allocates** — see
the global constraint — so its own locals follow the `Locals`/`PinnedValue`
rules.

- [ ] **Step 3: Register it — three edits, all required**

1. The enabled-build `defineInternMethod`, arity 1, beside its neighbours.
2. `STR(<name>, "<name>")` in the WASM block of `PredefinedStrings.def`, or
   `P::<name>` does not exist and the build fails without naming the cause.
3. **Move the Wasm-disabled registration range's hardcoded endpoint**, which
   currently reads `i <= B::HermesBuiltin_wasmMakeGlobal`. A builtin appended
   after it falls outside the loop and its id resolves to nothing with Wasm
   compiled out. Note the implementations sit inside `HERMES_ENABLE_WASM` —
   they are not compiled unconditionally.

- [ ] **Step 4: Verify both build configurations — and note that building is
not enough**

```bash
cmake --build cmake-build-asan --target hermes hermesc
cmake --build cmake-build-asan --target check-hermes
cmake --build cmake-build-wasm-off --target hermes
```

An unregistered appended builtin still **compiles and links**. The
completeness assertion runs during runtime initialization, so the Wasm-off
build must actually be RUN: execute a trivial script with the Wasm-off Debug
`hermes` binary and confirm it starts. Also exercise the predicate's own
behaviour directly in the enabled build — a builtin nothing calls yet is
otherwise untested.

- [ ] **Step 5: Record the builtin on dz `01a0460c-4361`** with a comment
naming this work, so the deferred `BYTECODE_VERSION` bump accounts for it.
**Do not bump the version.**

- [ ] **Step 6: Commit**

---

### Task 3: JS construction — the discriminator and the descriptors

**Spec:** section 1, "Constructor" and "`wasmMakeGlobal`'s mode
discriminator".

**Files:**
- Modify: `lib/VM/JSLib/HermesBuiltin.cpp` (`wasmMakeGlobal`)
- Modify: `lib/VM/JSLib/WebAssembly/WebAssembly.cpp` (`wasmGlobalConstructor`)

**Interfaces:**
- Consumes: Task 1's `ValType` and slot; Task 2's predicate.
- Produces: a `wasmMakeGlobal` that keys on `isMutable`; a constructor that
  accepts `"externref"` and `"anyfunc"`.

- [ ] **Step 1: Change the discriminator to `isMutable`**

Today `wasmMakeGlobal` decides "live" by testing whether argument 2 is
callable, then refuses that when the global is immutable. An immutable
`ref.func` global's **value is callable**, so it is misread and refused.
Adding types does not fix this — it is why the feature cannot work without
this change.

- `isMutable` — argument 2 is the getter closure, argument 3 the setter.
- `!isMutable` — argument 2 is the value; argument 3 unused.

**Preserve live-implies-mutable**; do **not** impose its converse. The public
constructor creates **mutable snapshots** and must keep working — the spec is
explicit that "a snapshot is always immutable" is a property of this
builtin's callers only.

- [ ] **Step 2: Widen the range check and the snapshot validation**

`rawCode > ValType::F64` becomes the new highest enumerator. The Number-only
snapshot validation is relaxed so a reference value is accepted for a
reference type — but an i64 still requires a BigInt, and a numeric type still
requires a Number.

- [ ] **Step 3: The constructor's descriptors**

| descriptor | behaviour |
|---|---|
| `"externref"` | accept; default value `undefined` |
| `"anyfunc"` | accept; default value `null` |
| `"funcref"` | **TypeError** |
| `"v128"` | TypeError, message naming SIMD |

`"funcref"` being refused while `"anyfunc"` is accepted is not a typo — node
does exactly this, and an importing module brand-checks against whatever the
constructor produced. Do not be lenient.

An explicit initial value for a funcref must be `null` or an Exported
Function, via Task 2's predicate. **That check allocates** — root across it.

- [ ] **Step 4: Test the descriptor table**

A driver asserting all four rows plus both defaults, matching the node
measurements in the spec.

- [ ] **Step 5: Build, run the suite, commit**

---

### Task 4: Value access — both setters, the getter, the funnel

**Spec:** section 1, subsections "`.value` setter", "The internal
setter" and "One writer".

**Files:**
- Modify: `lib/VM/JSLib/WebAssembly/WebAssembly.cpp` (getter, setter)
- Modify: `lib/VM/JSLib/HermesBuiltin.cpp` (`wasmGlobalSet`, the funnel)
- Modify: `lib/VM/JSLib/JSLibInternal.h` if the funnel's declaration changes

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces: all four value paths handling reference types.

- [ ] **Step 1: The public setter's per-type dispatch**

Its shape today is "i64 requires a BigInt; **everything else** goes through
`toNumber_RJS`" — which would coerce an externref object to `NaN`. Dispatch
explicitly, **before** any coercion:

- immutable → TypeError, first, unchanged
- I32 / F32 / F64 → `ToNumber`, then narrow
- I64 → require BigInt, wrap to 64 bits
- ExternRef → store as-is, **no coercion**
- FuncRef → `null` or an Exported Function, else TypeError

- [ ] **Step 2: Get the rooting right — this is the task's main hazard**

`toNumber_RJS` is a safepoint and the code already re-derives `glob` after
it. **The funcref brand check is also a safepoint** and currently has no such
handling anywhere. Root or re-derive after it, here and at every other site
that calls the predicate. A live reference global's setter additionally
invokes a closure through `Callable::executeCall1`. Only the externref
store-as-is path is genuinely safepoint-free.

- [ ] **Step 3: The internal setter**

`wasmGlobalSet` rejects every non-i64, non-Number value **before** reaching
either its live-setter call or the storage funnel, so widening the funnel does
not reach it. Give it the same per-type dispatch, with validation **before**
any closure invocation.

The two setters have **different contracts**: the public one **coerces**
(`ToNumber`); the internal one **validates strictly** and rejects. Do not
unify them.

- [ ] **Step 4: The funnel and the getter**

`setWasmGlobalNumber` widens into a funnel taking a `HermesValue` and coercing
per type, keeping the no-`default:` switch that makes `-Wswitch` speak. It
stays in `HermesBuiltin.cpp`.

The readers were migrated by **Task 1** and are not reimplemented here.
Verify them instead: the getter returns the slot, already canonical, and its
i64 arm no longer allocates per read. If either is untrue, Task 1 is
incomplete — say so rather than fixing it in this task's diff.

- [ ] **Step 5: Tests**

- `{}` assigned to an externref global reads back as the same object —
  under the old dispatch it becomes `NaN`, so this fails loudly on regression.
- A plain JS function assigned to a funcref global raises TypeError.
- `null` and `undefined` assigned to an externref global round-trip.
- i64 unchanged, including `2n**100n + 5n → 5n`.
- **The identity round-trip the spec requires**: JS writes an object through
  `.value`, Wasm reads it with `global.get` and returns it, and the result is
  `===` the original. A coerced or copied value fails this; nothing else in
  the plan asserts it.
- **The internal setter, in its own two-module test.** An imported mutable
  reference global written by the importing module via `global.set` goes
  through `wasmGlobalSet`, not the public setter — a module-owned write uses
  frame storage instead, so no public-setter test reaches that dispatch.
  Include a write from the importer's **start function**, which runs during
  instantiation.

- [ ] **Step 6: Build, run the suite, commit**

---

### Task 5: The link path

**Spec:** section 1, subsection "The link path".

**Files:**
- Modify: `lib/VM/JSLib/HermesBuiltin.cpp` (`wasmLinkGlobal`)
- Modify: `lib/WasmIRGen/WasmIRGen.cpp` (the global import path, ~1002-1125)

**Interfaces:**
- Consumes: Tasks 1-4; Task 2's predicate.
- Produces: globals working end to end. **This is the natural mid-plan
  review gate** — after it, a reference-typed global can be exported,
  imported, constructed and accessed.

- [ ] **Step 1: Generalise the matched-object return**

`wasmLinkGlobal` returns `null` for "not a Global", `undefined` for
"mismatch", anything else for the value — and for reference types both
sentinels are legal values. The live-globals work already returns the matched
**object** for a live global; generalise that to **every** match. A
`WebAssembly.Global` is never `null` or `undefined`.

The caller then fetches with `wasmGlobalGet`, **only for an immutable
import** — a mutable one keeps the object. The fetch cannot run a closure,
because a live global is always mutable.

- [ ] **Step 2: The raw-value rule, three compile-time arms**

The declared type is known at compile time, so this stays a compile-time
branch like today's `isI64`:

- numeric — `typeof` `"number"`, or `"bigint"` for i64, as now
- **externref — any JS value**, including `null` and `undefined`
- **funcref — `null` or an Exported Function**, via Task 2's predicate,
  routing a false result into the existing `LinkError` branch

Rewrite the messages. The current one — "must be a Number to satisfy an
externref global import" — describes a rule nobody wrote.

- [ ] **Step 3: Tests, including the sentinel collision**

- A genuine `Global` satisfying a reference import.
- A **raw object** satisfying an immutable externref import.
- A raw plain function **refused** for a funcref import.
- **The sentinel collision**: import a global whose value genuinely **is**
  `null`, and another whose value is `undefined`. These must be **immutable
  snapshot** Globals — a mutable import discards the link result's value and
  keeps the object, so a mutable case passes without exercising the collision
  at all.

- [ ] **Step 4: The golden that must change**

`test/wasm/irgen-memory-global-import-link.wat` pins the old link-result
value in the import phi. Update it to pin the new fetch to the
matched-immutable branch specifically. Review the diff; do not regenerate
blindly.

- [ ] **Step 5: Build, run the suite, commit**

---

### Task 6: Nullable IR type, then `ref.is_null`

**Spec:** section 3.

**Files:**
- Modify: `lib/WasmIRGen/WasmIRGen.cpp` (`wasmValTypeToIRType`, the opcode)
- Modify: `include/hermes/WasmIRGen/WasmIRGen.h` if a declaration changes
- Modify: `lib/WasmFrontend/BinaryReaderHermesIRGen.cpp` (dispatch)
- Delete: `test/wasm/ref-is-null-unsupported.wat` (and its driver if any)

**Interfaces:**
- Consumes: Tasks 1-5.
- Produces: `ref.is_null` returning 0 or 1.

**The two steps are ordered and the order is the point.**

- [ ] **Step 1: Fix the annotation FIRST**

`wasmValTypeToIRType(FuncRef)` returns `Type::createObject()`, which
**excludes null**, and that annotation reaches parameters, locals and
indirect-call results. `InstSimplify` folds strict equality between disjoint
types:

```cpp
// Operands of different types can't be strictly equal.
if (typeCtx_.intersectTy(leftTy, rightTy).isNoType())
  return builder_.getLiteralBool(false);
```

So a null funcref compared against `null` folds to `false` **unconditionally,
in optimized builds only**. Change it to `Type::createObjectOrNull()`, which
already exists.

Regenerate any golden IR this moves and review each diff.

- [ ] **Step 2: Implement the opcode**

A **strict** null comparison yielding 0 or 1. Strictness matters both ways:
`undefined` is a valid, **non-null** externref, so a loose comparison would
report it as null.

- [ ] **Step 3: Replace the old test, do not edit it**

`ref-is-null-unsupported.wat` asserts the *wrong* answer today. A test whose
name and expectations both encode the defect is not a regression test for the
fix — delete it and write a new one.

- [ ] **Step 4: Test the optimized paths, not just a literal**

`ref.is_null` on: a literal `ref.null`, a **parameter**, a **local**, and an
**indirect-call result**, each at the default optimization level, for both
funcref and externref. A test using only `ref.null` passes even with the
annotation bug still present — that is the trap this step exists to avoid.

- [ ] **Step 5: Build, run the suite, commit**

---

### Task 7: Function-body `ref.func`

**Spec:** section 2.

**Files:**
- Modify: `lib/WasmFrontend/BinaryReaderHermesIRGen.cpp` (dispatch)
- Modify: `lib/WasmIRGen/WasmIRGen.cpp` (`computeEscapableFuncs`, the opcode)
- Modify: `include/hermes/WasmIRGen/WasmIRGen.h` if a declaration changes
- Modify: `test/wasm/irgen-unsupported-warn.wat` (it requires the warning)

**Interfaces:**
- Consumes: Tasks 1-6.
- Produces: `ref.func` in a function body pushing the canonical Exported
  Function wrapper.

- [ ] **Step 1: Make a wrapper exist for every index `ref.func` can name**

`computeEscapableFuncs` currently collects element-segment indices and
`ref.func` global initializers. A function-body occurrence needs its index in
that set too, or there is no canonical wrapper and the only thing to hand out
is the internal closure.

The comment there already anticipates this change and names the safe
fallback — "put every function in the set". Choosing the fallback over a
body pre-pass is acceptable; **justify whichever you choose in the code**.

- [ ] **Step 2: Implement the opcode**, pushing the wrapper rather than
calling `warnUnsupported`.

- [ ] **Step 3: Update the test that requires the warning you removed**

`test/wasm/irgen-unsupported-warn.wat` asserts that `ref.func` IS reported as
unsupported — it will fail the moment Step 2 lands. That is correct, not a
regression: the test encodes the defect. Remove the `ref.func` expectation and
keep the file's coverage of whatever remains genuinely unsupported. Do not
delete the file wholesale.

- [ ] **Step 4: Test the round-trip that the spec moved here**

Wasm stores a `ref.func` wrapper into a mutable exported funcref global; JS
reads `.value`; it is `===` the Exported Function the module exports by name.
Plus `ref.func; drop` in an uncalled function, which works today and must
keep working.

- [ ] **Step 5: Build, run the suite, commit**

---

### Task 8: The JS-to-Wasm conversion points

**Spec:** section 5.

**Files:**
- Modify: `lib/WasmIRGen/WasmIRGen.cpp` (`createExportWrapper` parameter
  marshalling; the imported-function result path, including multi-value)
- Modify: `test/wasm/e2e-mv-ref-import.wat` and its `-driver.js_` (they
  require an ordinary JS function to work as a funcref)

**Interfaces:**
- Consumes: Task 2's predicate.
- Produces: funcref validation on both routes.

Both sites currently read:

```cpp
default:
  // FuncRef, ExternRef, etc: pass through for now.
```

- [ ] **Step 1: Validate export-wrapper funcref parameters.** An exported
`(param funcref)` function receives whatever JS passes; `local.get 0;
global.set $g` then puts an ordinary JS function into a funcref global, with
no setter involved.

- [ ] **Step 2: Validate imported-function funcref results**, including the
multi-value path.

- [ ] **Step 3: Leave the externref arms alone.** Any JS value is a valid
externref. A check there is a bug — and a test that refuses an externref
would be asserting the bug.

- [ ] **Step 4: Rewrite the existing test that asserts the defect**

`test/wasm/e2e-mv-ref-import-driver.js_` defines a plain
`function seven() { return 7; }`, returns it from an import declared to return
funcref, and `e2e-mv-ref-import.wat` asserts
`importedFunc: function same=true calls -> 7`. Step 2 makes that a rejection,
so the test fails — correctly. **Keep its transport coverage** by switching it
to a genuine Exported Function, and add the rejection as a new case rather
than losing the identity assertion.

- [ ] **Step 5: Tests with oracles that can actually fail**

Rejection alone proves nothing: existing table-slot tests already reject
invalid funcrefs downstream, so a test can pass with the wrapper validation
still absent. Require:

- The function body has **no downstream validator**, and the test asserts the
  body **was not entered** — a counter the body increments, still zero.
- **Both** the single-result and multi-result import paths.
- Positive cases that stop unconditional rejection passing: `null` accepted,
  a canonical wrapper accepted, and a wrapper for an **imported** function
  accepted.
- externref param **accepted** and externref result **accepted** — a test
  that refuses either is asserting a bug.

- [ ] **Step 6: Build, run the suite, commit**

---

### Task 9: Exception payloads

**Spec:** section 6.

**Files:**
- Modify: `lib/WasmIRGen/WasmIRGen.cpp` (`onCatch`)

**Interfaces:**
- Consumes: Task 2's predicate.
- Produces: funcref payload validation at the catch boundary.

`wasmMatchException` accepts a JS array whenever element zero is the expected
tag, without validating the payload; `onCatch` then loads payload values
straight onto the Wasm stack. A module exporting a `(param funcref)` tag and
importing a void JS function that throws `[exports.tag, ordinaryFunction]`
reaches a `global.set` with an unvalidated function. No forgery is needed —
the real exported tag works.

- [ ] **Step 1: Validate in `onCatch`, not in the matcher.** Validate **the
same loaded value that is pushed**. Checking an item in the matcher and
reading it again lets an accessor-backed payload return one value to the
check and another to the consumer.

- [ ] **Step 2: An invalid funcref item raises a TypeError**, not a tag
mismatch. The two differ observably: a mismatch falls through to an outer
handler or rethrows, hiding the error.

- [ ] **Step 3: Tests with a precise oracle**

- A mixed `(i64 funcref)` payload, so the two-index i64 layout is exercised
  alongside the check.
- An accessor-backed payload item that counts reads: assert **exactly one**
  read, and that the value pushed is identical to the value that read
  returned. "Did not let an invalid funcref through" is too weak — it passes
  if the item is read twice and the second read happens to be valid.
- A **valid-first, invalid-second** accessor must succeed, delivering the
  first value.
- An **invalid-first** case must raise **TypeError** specifically, not merely
  throw and not fall through to an outer handler.
- Externref payloads of any value accepted.

- [ ] **Step 4: Build, run the suite, commit**

---

### Task 10: Multi-value reference results — per-activation transport

**Spec:** section 7 **in full**. This is the largest and subtlest task; read
the whole section, including "The precedent to NOT follow".

**Files:**
- Modify: `lib/WasmIRGen/WasmIRGen.cpp` (result stores and loads, `onCall`,
  `call_indirect`, `createExportWrapper`, import trampolines,
  `createFunctions`, `beginFunction`)
- Modify: `include/hermes/WasmIRGen/WasmIRGen.h` (declarations, parameter
  counts)
- Add: builtins for allocating and accessing the container — with the full
  three-edit registration recipe from Task 2

**Interfaces:**
- Consumes: Tasks 1-9.
- Produces: reference results that cross module boundaries and cannot be
  intercepted.

**Do not copy the numeric return buffer.** It is built from replaceable
`ArrayBuffer`/`Uint32Array`/`Float64Array` globals and accessed with ordinary
property operations, and it carries two filed defects — `01a0820d-5190`
(nested-call buffer mismatch) and `01a0821a-1093` (interceptable storage).
Read both issues before threading buffers.

- [ ] **Step 1: The container.** `ArrayStorage` — traced full `HermesValue`
elements, direct indexed access, barriered writes, existing tracing metadata.
Allocate **size as well as capacity**: `create(runtime, n, n)`, because
indexed access checks `size()`. Allocate and access through native builtins so
it is never script-reachable.

- [ ] **Step 2: Per-activation allocation.** One container per multi-value
call **that has a reference result** — not per module. That is what makes a
reentrant call unable to reach an outstanding activation's values, and it is
the whole point of the step.

- [ ] **Step 3: The caller must retain the container as a local IR value**
through the final reference load. Today's code looks the buffer up in the
module frame at each use; allocating fresh storage while keeping that lookup
allocates per activation and then reads a shared object anyway — the contract
in name only. **Replacing the lookup is the change.**

- [ ] **Step 4: Write the ABI table BEFORE writing code.** The contract below
is settled; these are the decisions inside it, and they are shared by code
paths written independently, so they must be recorded in the task's report
before implementation:

- **Which signatures carry the reference argument**, and its **position**
  relative to the two numeric buffers and split i64 parameters. Definitions
  currently prepend two buffers, and trampolines and function bodies
  independently hardcode the first Wasm parameter as index 3 — that constant
  moves, in every place that knows it.
- **The slot-index formula and the allocation size.** Reference slots today
  use *byte offset divided by four*, not a reference ordinal. Keeping that
  sparse layout is a legitimate choice; if you keep it, allocating merely the
  number of references is **wrong**. Decide, write it down, and size the
  allocation to match.
- **Separate representations** for the buffer this function received and the
  buffer it passes to each nested call. Conflating them is defect (a).
- **The signatures** of the allocation, load and store builtins, and of the
  revised result-loading helper.
- Name the affected declarations: `createFunctions`, `beginFunction`, the
  parameter counts, and `WasmIRGen.h`.

Then: return stores target the incoming buffer; a nested call's result loads
target **exactly the buffer passed to that nested call** — buffer *identity*
is the rule, module locality is not the defect. Capacity sized for the
callee's signature. Import trampolines keep their two-pass scheme, which
captures every result before writing any buffer. The hidden buffer never
enters a JS import's argument list and never becomes the public result array.

- [ ] **Step 4b: Register this task's own builtins.** Task 2's three-edit
recipe applies again, in full, to every builtin added here: the declaration,
the predefined string, the enabled registration, and **moving the
Wasm-disabled range's hardcoded endpoint**. Record each on dz
`01a0460c-4361`. Task 12 audits *all* added builtins, not only Task 2's.

- [ ] **Step 5: The public result array** is allocated intrinsically — fresh,
own data elements, no script invocation — closing the separate interception
route in `createExportWrapper`, which today calls `globalThis.Array` after the
internal call and interleaves buffer reads with indexed writes.

- [ ] **Step 6: Four tests, because they fail independently**

- **Direct JS call** to a multi-value export returning a reference. No second
  module needed.
- **Cross-module indirect**: B calls A's exported `(result i32 funcref)`
  through a table and receives the real reference.
- **Cross-module direct import**: `B trampoline → A wrapper → A function`.
- **Reentrancy, two shapes**: `globalThis.Array` replaced by storage whose
  setter re-enters an export, with both a funcref and an **externref** result
  surviving unchanged; and a **mixed** `(result externref i32 externref)` with
  `Uint32Array`/`Float64Array` replaced by storage supplying reentrant getters
  and setters.

Each of those needs conditions that stop it passing vacuously:

- **Replace the typed-array constructors BEFORE instantiation.** The numeric
  buffers are built at instantiation and retained in frame slots, so a
  replacement installed afterwards never runs. Assert the reentry counters
  actually fired.
- **Give the outer and nested activations distinguishable reference
  identities.** Returning the same object or wrapper from both lets shared
  storage pass the test.
- For the undersized-buffer case, put an inner **reference** at an index
  beyond the outer buffer's size. "More results" alone is not enough — extra
  *numeric* results can leave every reference index in bounds.
- For the GC case, collect **after the producing frame has returned**, with
  the reference held only by the outstanding transport container. Collecting
  while the producer still roots everything does not test transport tracing.
- Test **inherited indexed setters** on the public result array separately:
  switching to intrinsic allocation while keeping ordinary property stores
  still violates the own-data-elements requirement.

- [ ] **Step 7: Assert the reference half only in the mixed test**, with a
comment saying why. The `i32` may still be corrupted — that is
`01a0821a-1093` and out of scope. Without the comment, a future reader takes
the test's silence as evidence the `i32` is safe.

- [ ] **Step 8: Build, run the suite, commit**

---

### Task 11: Element expressions and the segment model

**Spec:** section 4, "Element expressions".

**Files:**
- Modify: `include/hermes/WasmFrontend/WasmTypes.h` (`WasmElemSegment`)
- Modify: `lib/WasmFrontend/BinaryReaderHermesIRGen.cpp`
- Modify: `lib/WasmIRGen/WasmIRGen.cpp` (the active-element loop)

**Interfaces:**
- Consumes: Tasks 1-10.
- Produces: element expressions containing `global.get` and `ref.null`.

- [ ] **Step 1: Grow the item model.** `WasmElemSegment` holds only
`std::vector<uint32_t> funcIndices` and cannot represent a null or a
global-derived entry, so `(global.get $g), (ref.func $f)` silently **loses its
first entry** and places `$f` at the wrong index. Represent three item kinds:
a function index, a null, and a global-derived value.

- [ ] **Step 2: `ref.func` inside an element expression already works and
must keep working.** The reader records its index through the same callback
that delivers ordinary function-index segments, so key any change on
**context**, not on the opcode alone, or ordinary segments break.

- [ ] **Step 3: The ordering precondition is already met.** H20
(`01a0460b-ab9a`) moved `initializeGlobals` before `createTables`, so a
`global.get` in an element expression reads an initialized slot. Do not
re-fix it; do confirm the H20 tests still pass.

- [ ] **Step 4: Fix the `flags == 3` misclassification**, which treats
expression-form **declarative** segments — whose flags are `7` — as passive.
Pre-existing, in the code this task rewrites; fix it here or file it, but do
not leave it unrecorded.

- [ ] **Step 4b: Migrate EVERY consumer of `funcIndices`, not just the active
loop.** Removing or changing the field breaks compilation otherwise, and
migrating only the active path leaves passive segments silently wrong. The
consumers are:

- the active-element application loop;
- **wrapper discovery** in `computeEscapableFuncs`, which iterates
  `seg.funcIndices` — unless Task 7 chose the all-functions fallback, in which
  case say so;
- **passive-segment storage** for `table.init`, which independently tests
  emptiness, sizes its array and materializes function indices.

Give them a shared item-to-value lowering rule rather than three ad-hoc
conversions.

- [ ] **Step 5: Tests that actually observe the entries**

- A segment mixing `ref.func`, `ref.null` and `global.get`, asserting each
  lands at the right index.
- **Passive segments must be USED through `table.init`.** Merely instantiating
  a module with a passive `ref.null` segment passes today while its contents
  are discarded — the test would prove nothing.
- For an active `ref.null` entry, **overwrite a previously populated slot**.
  Checking an initially-null slot passes even if the implementation skips the
  null store entirely.
- Declarative segments containing `ref.null`, which work today and must keep
  working.

- [ ] **Step 6: Build, run the suite, commit**

---

### Task 12: The v128 diagnostic, and the sweep

**Spec:** scope section; dz `01a07d4b-01bc`.

**Files:**
- Modify: `lib/WasmIRGen/WasmIRGen.cpp`
- Modify: `dz/issues/…` (close `01a074ce-ac97`, update `01a07d4b-01bc`)

- [ ] **Step 1: Diagnose v128 global exports** with a message naming SIMD,
before they reach the runtime. There is no `ValType::V128`, so this must be a
compile-time refusal.

Assert it **executably**, in the `hermesc`-refuses shape used by
`compile-invalid-call-indirect-externref.wat`: the exact diagnostic, not just
a non-zero exit, since a refusal for the wrong reason would pass a bare
exit-code check.

- [ ] **Step 2: The full sweep**

```bash
cmake --build cmake-build-asan --target hermes hermesc
cmake --build cmake-build-asan --target check-hermes
cmake --build cmake-build-wasm-off --target hermes
```

Plus the sanitize-handles build, which **requires** `-DHERMES_ENABLE_WASM=ON`
— it defaults OFF, and `test/lit.cfg` only discovers `.wat` when it is on, so
without it the command runs zero Wasm tests and passes:

```bash
cmake -B cmake-build-sanhandles -G Ninja -DCMAKE_BUILD_TYPE=Debug \
  -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ \
  -DHERMES_ENABLE_WASM=ON -DHERMESVM_SANITIZE_HANDLES=ON
cmake --build cmake-build-sanhandles --target hermes hermesc
LIT_FILTER="wasm" cmake --build cmake-build-sanhandles --target check-hermes
```

- [ ] **Step 3: The GC test must not pass vacuously.** Use an **immutable
snapshot** global and eliminate other roots: an exported *mutable* global
retains its reference through the getter/setter closures and the module
frame, so deleting `value_`'s metadata would not break it. Hold only the
`Global` across a collection under `-gc-sanitize-handles=1`, and prove the
check bites by removing the `addField` and watching it fail.

- [ ] **Step 4: Close `01a074ce-ac97`** with what landed and what did not,
and update `01a07d4b-01bc` with the v128 diagnostic's message so whoever
implements the shell knows what to remove.

- [ ] **Step 5: Audit EVERY builtin this plan added** — Task 2's predicate
and each builtin Task 10 introduced for the reference container. For each,
confirm four things: the `Builtins.def` declaration, the predefined string,
the enabled registration, and that the **Wasm-disabled range's hardcoded
endpoint** covers it. Then confirm `BYTECODE_VERSION` is still 100 and that
dz `01a0460c-4361` records them all.

Building the Wasm-off binary does not prove the registration: an unregistered
builtin compiles and links, and the completeness assertion runs at runtime
init. **Run it** — execute a trivial script with the Wasm-off `hermes` and
confirm it starts.

- [ ] **Step 6: Commit**

---

## Out of scope

Filed, deliberately not fixed here:

- `01a07d4b-01bc` — the v128 `Global` shell whose `.value` throws.
- `01a079ff-5d7b` — externref table export interposition.
- `01a079fe-a227`, `01a079ff-5dc0` — global object identity.
- `01a07f24-a1cc` — active element bounds checking.
- `01a07f25-046f` — the `__wasm_type__` forging hole.
- `01a0820d-5190`, `01a0821a-1093` — the two numeric return-buffer defects.
  Task 10 must not inherit them; fixing them is separate work.
