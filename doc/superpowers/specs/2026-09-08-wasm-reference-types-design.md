# Wasm reference types: externref and funcref

**Issue:** dz `01a074ce-ac97` (v128 split to `01a07d4b-01bc`)

**Status:** design, revision 7, reviewed and approved for
implementation. Revisions 1-3 scoped this as "reference-typed
globals" with the rest deferred behind gates; three review rounds showed that
seam does not exist. Revision 4 widened it to one piece, and a fourth review
produced a complete route inventory with one area unaccounted for. Revision 5
closes that area and two implementation prerequisites.

## Why this is one piece and not three

Revisions 1-3 tried to ship globals first and gate the operations that were
not yet supported. Each round closed the previous findings and leaked
somewhere new:

- **R1** — the design was wrong about storage, GC and the link path.
- **R2** — deferral exposes broken paths: a module exporting a mutable funcref
  global fails instantiation *today*, so everything downstream is unreachable.
  Making it instantiate turns "rejected" into "accepted but wrong".
- **R3** — the gates added to fix that **regress modules that work today**. A
  `ref.func` in an uncalled function, or `ref.func; drop` where the
  placeholder is discarded, work now and would be refused. A passive or
  declarative element segment containing `ref.null` never initializes anything
  and works now. And the element gate is not implementable as specified:
  `global.get` and `ref.null` leave no marker in `funcIndices`, so detecting
  them needs reader changes — most of the work of just supporting them.

The conclusion: everything downstream is harmless **only because**
reference-typed globals cannot exist. Removing that keystone requires the rest
to work. Gating the two unsupported opcodes costs about what implementing them
costs, and gating regresses working modules while implementing does not.

The whole unsupported reference-operation surface is **two opcodes**:
`ref.is_null` and function-body `ref.func`. That is what makes the wider scope
tractable.

## Scope

1. **Globals** — storage, the JS surface, the link path.
2. **Function-body `ref.func`** — currently warns and pushes `undefined`.
3. **`ref.is_null`** — currently warns and pushes `undefined`.
4. **Element expressions** — `global.get` and `ref.null` entries, and the
   data model that cannot represent them.
5. **JS-to-Wasm conversion points** — export-wrapper parameters and
   imported-function results.
6. **Exception payloads** — funcref items arriving from JS through a thrown
   tag array.
7. **Multi-value reference results** — the buffer they travel in does not
   cross module boundaries and is script-interceptable.

Out of scope: **v128** (`01a07d4b-01bc`); **reference-typed tables**
(`01a079ff-5d7b`); **global object identity** (`01a079fe-a227`,
`01a079ff-5dc0`); **active element bounds checking** (`01a07f24-a1cc`); the
**`__wasm_type__` forging hole** (`01a07f25-046f`).

Confirmed by review to need no work, recorded so nobody re-checks:
`table.set` and `table.fill` go through the validating slot writer,
`table.grow` checks its fill value, and a start function has no parameters or
results. `call_indirect` reaches a correct target — an internal function or an
import trampoline — but its reference RESULT transport is broken; that is
section 7, not a clean row.

## Constraints

- C++17, no exceptions, no RTTI. 80 columns, 2-space indent. Doc comment on
  every declaration.
- Runtime code uses `Locals` + `PinnedValue`; no raw GC pointer across a
  safepoint; every `CallResult` checked.
- **Do not bump `BYTECODE_VERSION`.** This work adds a builtin; record it on
  dz `01a0460c-4361`.
- Descriptor spellings and defaults match node v24.13.1, not spec prose.

## 1. Globals

### Storage

```cpp
  GCHermesValue value_;          // replaces double value_ + int64_t i64Value_
  ValType valType_;
  bool mutable_;
  GCPointer<Callable> getter_;   // unchanged, from the live-globals work
  GCPointer<Callable> setter_;
```

registered with `mb.addField("value", &self->value_)`.

`GCHermesValue` is `GCHermesValueImpl<HermesValue>` — full 64-bit in every
heap mode, unlike `GCSmallHermesValue`, which becomes `HermesValue32` under
`HERMESVM_BOXED_DOUBLES`. Its store performs a write barrier and introduces no
double boxing. The precedent is `FinalizationRecord::heldValue`, a plain
`GCCell` — **not** `JSFinalizationRegistry`, a `JSObject`.

The cell does **not** shrink: allocation is
`max(sizeof(Derived), cellSizeJSObject())`, so removing 8 bytes of fields
frees an overlap slot while leaving the allocated size unchanged. Claim the
field reduction, not a cell-size saving.

| `valType_` | slot holds |
|---|---|
| I32 | number, `ToInt32`-narrowed |
| F32 | number, `fround`-narrowed |
| F64 | number |
| I64 | `BigIntPrimitive*`, wrapped to 64 bits |
| ExternRef | any `HermesValue` |
| FuncRef | `null`, or an Exported Function |

**i64.** Truncation already happens on writes; what moves is **BigInt
materialization**. Four sites become allocating stores where they were scalar
writes — the constructor, `wasmMakeGlobal`, the public setter and the internal
setter — so each must root its destination across the allocation and check for
failure. The read-side saving applies only to snapshots; a live i64 getter
still rebuilds from the frame's lo/hi words.

**ValType.** `ExternRef` and `FuncRef` append as 4 and 5; 0-3 keep their
values and the `static_assert`s stand. `globalValTypeCode` gains two arms. No
`V128`. `-Wswitch` names exactly one site (`setWasmGlobalNumber`, which has no
`default:`) and is **not** sufficient: `wasmMakeGlobal` range-checks with
`rawCode > ValType::F64`, an ordering comparison the compiler will not flag.
Enumerate consumers by hand.

### Constructor

| descriptor | node v24.13.1 | Hermes |
|---|---|---|
| `"externref"` | accepted, defaults to `undefined` | accept |
| `"anyfunc"` | accepted, defaults to `null` | accept |
| `"funcref"` | TypeError | reject |
| `"v128"` | TypeError | reject, message naming SIMD |

`"funcref"` refused while `"anyfunc"` is accepted is not a typo: the JS API's
`ToValueType` lists `anyfunc`, and an importing module brand-checks against
whatever the constructor produced.

### `wasmMakeGlobal`'s mode discriminator

It decides "live" by testing whether argument 2 is callable, then refuses that
when the global is immutable. The export loop passes the **value** in that
position for a snapshot — so an immutable `ref.func` global, whose value *is*
callable, and an immutable externref holding a function are both misread and
refused.

**Discriminate on `isMutable`**, which this builtin already guarantees:

- `isMutable` — argument 2 is the getter closure, argument 3 the setter.
- `!isMutable` — argument 2 is the value; argument 3 unused.

Callable, `null` and `undefined` snapshot values are then unambiguous.

"A snapshot is always immutable" is a property of **this builtin's callers**,
not of `Global` in general — the public constructor creates a snapshot and
takes its mutability from the descriptor, so **mutable snapshots exist and
must keep working**. Preserve **live implies mutable**; do not impose its
converse.

Also widen the `rawCode > ValType::F64` bound and relax the Number-only
snapshot validation.

### `.value` setter

Today: "i64 requires a BigInt; **everything else** goes through
`toNumber_RJS`" — which would coerce an externref object to `NaN`. Dispatch
becomes explicit per type before any coercion:

- immutable -> TypeError, first, unchanged
- I32 / F32 / F64 -> `ToNumber`, then narrow
- I64 -> require BigInt, wrap to 64 bits
- ExternRef -> store as-is, no coercion
- FuncRef -> `null` or an Exported Function, else TypeError

**The funcref brand check is a safepoint.** `isWasmExportedFunction` reaches
`HiddenClass::findPropertyNoMap`, which initializes a missing property map and
**allocates**; it roots its own arguments, not the caller's. Root or re-derive
across it at **every** call site — the public setter, the constructor,
snapshot creation in `wasmMakeGlobal`, the internal setter, raw-import
validation, and the conversion points in section 5. A live reference global's
setter additionally invokes a closure through `Callable::executeCall1`. Only
the externref store-as-is path is genuinely safepoint-free.

### The internal setter

An imported mutable global is written through `wasmGlobalSet`, which rejects
every non-i64, non-Number value **before** reaching either its live-setter
call or the storage funnel. Widening the funnel does not reach it; it needs
the same per-type dispatch, with validation before any closure invocation.

The two setters have **different contracts**: the public one **coerces**
(`ToNumber`), the internal one **validates strictly** and rejects. Reference
types are validated in both.

### The link path

`wasmLinkGlobal` returns `null` for "not a Global", `undefined` for
"mismatch", anything else for the value — and for reference types both
sentinels are legal values.

**Generalise the matched-object return** the live-globals work already uses
for live globals: `null` / `undefined` / **an object**. A `WebAssembly.Global`
is never `null` or `undefined`. The caller fetches with `wasmGlobalGet`, only
for an **immutable** import. Review confirmed the TOCTOU argument holds when
the fetch happens only after a successful immutable match.

Raw-value rule, three compile-time arms: numeric as now; **externref accepting
any JS value** including `null` and `undefined`; **funcref accepting `null` or
an Exported Function**.

**A new predicate builtin.** The funcref arm has nothing to call:
`isWasmExportedFunction` is a C++ helper, the raw-import branch is generated
IR emitting `typeof` comparisons, and `wasmSetFuncInfo` **stamps** the brand
without querying it. Add a predicate builtin sharing `readWasmFuncInfo`'s
brand so the two cannot drift; its allocation does not disqualify it, its
calls are simply safepoints. Registration needs three edits, as
`wasmMakeGlobal` did: the enabled registration, a predefined string, and
moving the **hardcoded endpoint of the Wasm-disabled id range**, currently
`wasmMakeGlobal`. Note the implementations sit inside `HERMES_ENABLE_WASM`;
they are not compiled unconditionally.

Rewrite the messages — "must be a Number to satisfy an externref global
import" describes a rule nobody wrote.

## 2. Function-body `ref.func`

Currently `warnUnsupported`, pushing `undefined`; a following `global.set`
stores that unvalidated and a live getter returns it. Implement it: push the
canonical Exported Function wrapper for the index.

`computeEscapableFuncs` must cover function bodies so a wrapper exists. Its
comment already anticipates this change and names the safe fallback — "put
every function in the set". Choosing the fallback over a body pre-pass is
acceptable and should be justified in the code either way.

## 3. `ref.is_null`

Currently `warnUnsupported`, producing `undefined` instead of an i32.
Implement it as a **strict** null comparison yielding 0 or 1.

**A prerequisite, or the opcode miscompiles silently.**
`wasmValTypeToIRType(FuncRef)` returns `Type::createObject()`, which EXCLUDES
null, and that annotation reaches parameters, locals and indirect-call
results. `InstSimplify` folds strict equality between disjoint types:

```cpp
// Operands of different types can't be strictly equal.
if (typeCtx_.intersectTy(leftTy, rightTy).isNoType())
  return builder_.getLiteralBool(false);
```

So a null funcref compared against `null` folds to `false` unconditionally,
in optimized builds only. Change the annotation to `Type::createObjectOrNull()`,
which already exists, BEFORE implementing the opcode.

Strictness matters in the other direction too: `undefined` is a valid,
non-null externref, so a loose comparison would report it as null.

`test/wasm/ref-is-null-unsupported.wat` **asserts the wrong answer today** and
must be replaced rather than updated — a test whose name and expectations both
encode the defect is not a regression test for the fix.

## 4. Element expressions

`global.get` inside an element expression is silently **ignored** and
`ref.null` records no element, so `(global.get $g), (ref.func $f)` loses its
first entry and places `$f` at the wrong index. `ref.func` inside an element
expression **already works** and must keep working — the reader records its
index through the same callback that delivers ordinary function-index
segments, so any change must key on context, not on the opcode alone.

`WasmElemSegment` holds only `std::vector<uint32_t> funcIndices` and must grow
to represent three item kinds: a function index, a null, and a global-derived
value. The ordering precondition — globals initialized before tables — is
**already done**, fixed as H20 (`01a0460b-ab9a`, closed).

**A pre-existing bug in the same area, found by review:** the reader's
`flags == 3` test misclassifies expression-form **declarative** segments,
whose flags are `7`, as passive. Fix it here or file it; do not leave it
unrecorded while rewriting the surrounding code.

## 5. JS-to-Wasm conversion points

Two routes bypass both runtime setters entirely:

```cpp
default:
  // FuncRef, ExternRef, etc: pass through for now.
  callArgs.push_back(paramVal);
```

- **Export-wrapper parameters.** An exported `(param funcref)` function
  receives whatever JS passes; `local.get 0; global.set $g` then puts an
  ordinary JS function into a funcref global.
- **Imported-function results**, including the multi-value path.

Validate the **funcref** arms of both using the shared brand. Do **not** touch
the externref arms: any JS value is a valid externref, and a check there would
be a bug. Review confirmed both checks preserve legitimate nulls and canonical
wrappers, including wrappers for imported functions.

## 6. Exception payloads

`wasmMatchException` accepts a JS array whenever element zero is the expected
tag object, without validating the payload; `onCatch` then loads payload
values straight onto the Wasm stack, and a `global.set` stores them
unvalidated.

A module can export a `(param funcref)` tag and import a void JS function that
throws `[exports.tag, ordinaryJSFunction]`. No `__wasm_type__` forgery is
needed — the real exported tag works, as does mutating and rethrowing a
genuine Wasm exception array.

Validate funcref payload items at this boundary; preserve externref payloads
unvalidated.

**Where the check goes is part of the contract.** `wasmMatchException`
receives only the array and the tag identity; `onCatch` knows the payload
types and performs the ordinary property loads that push values onto the Wasm
stack, with i64 occupying two indices. Validate **the same loaded value that
is pushed**, in `onCatch` — checking an item in the matcher and reading it
again lets an accessor-backed payload return one value to the check and
another to the consumer.

Specify the failure mode explicitly: an invalid funcref item raises a
TypeError rather than being treated as a tag mismatch. The two differ
observably — a mismatch would fall through to an outer handler or rethrow,
which would hide the error.

## 7. Multi-value reference results

Reference results of a multi-value call travel in a side array. This is the
deepest area in the spec and the one four review rounds kept reopening, so it
is specified as a contract rather than a direction.

### Three defects, not one

**a. The transport array does not cross module boundaries.** `call_indirect`
passes only the caller's numeric return buffers. A reference result is stored
into the **callee's** module-local array and read back from the **caller's**
own — two different objects. Put module A's exported `(result i32 funcref)`
function into module B's table, call it indirectly, and B reads an unwritten
or stale slot. Externref and foreign import trampolines are affected the same
way.

**b. The transport array is script-interceptable.** It is constructed through
a replaceable `globalThis.Array`, and its writes and reads are ordinary
indexed property operations. A replacement constructor can return storage
whose setter discards the real reference and whose getter yields an ordinary
JS function, reaching the Wasm stack **after** section 5's validation with no
brand forgery. It can equally substitute an externref, so more funcref checks
do not help: externref values must arrive unchanged, and only private storage
gives that.

**c. The PUBLIC result array is a separate route.** `createExportWrapper`
calls `globalThis.Array` **after** the internal call and before reading its
results, then interleaves buffer reads with ordinary indexed writes into that
constructor's product. So a replacement constructor or indexed setter can
re-enter an export and clobber reusable return storage before the outer
wrapper has finished reading it, and the returned object can discard or
substitute results outright. This one is reachable by a **direct JS call** to
an export — no second module needed — and also on the route
`B import trampoline -> A export wrapper -> A function`, where a substituted
externref passes section 5 correctly and another genuine Exported Function
passes its funcref check.

### The precedent to NOT follow

Revision 5 said the numeric return buffer is "module-owned and unreachable
from script". **That is false.** Those buffers are built from replaceable
`ArrayBuffer`, `Uint32Array` and `Float64Array` globals, and numeric result
stores are ordinary property operations. Two consequences:

- Copying that model gives interceptable storage again.
- Even *private* reference storage is insufficient while numeric accesses
  stay interceptable, because a mixed result interleaves them: a numeric
  setter can re-enter Wasm between two reference stores, and a numeric getter
  between two reference loads.

The numeric path also carries a live defect of its own, filed as
`01a0820d-5190`: a function's result stores target the buffer its caller
supplied, but a nested call's result loads read the **incoming** buffer while
`onCall` hands the nested callee the **module-local** one. Those diverge after
a cross-module `call_indirect`. The reference contract below must not inherit
it, and applying the same contract to the numeric path would close it.

### The contract

- **A caller-supplied hidden reference buffer**, passed consistently by
  direct calls, indirect calls and export wrappers, and received by function
  definitions and import trampolines.
- **Return stores target the incoming buffer.** A nested call's result loads
  target **exactly the buffer passed to that nested call** — never a buffer
  selected independently from module scope. Buffer *identity* is the rule;
  module locality is not the defect. A caller may legitimately pass its own
  module-local buffer and then read that same buffer. What breaks today, and
  what `01a0820d-5190` records, is passing one buffer and reading a
  different one.
- **Capacity is sized for the callee's signature.** Forwarding a foreign
  incoming buffer can be too small when the nested signature has more
  results.
- **Import trampolines keep their existing two-pass scheme**, which captures
  every result before writing any buffer. That ordering is what makes them
  safe under callbacks and must not be flattened.
- **The hidden buffer never becomes visible.** It is not appended to a JS
  import's argument list, and it is never returned as the public export
  result array.
- **Publication invariant — the decision, not the menu.** Reference results
  use **per-activation storage**: each multi-value call with a reference
  result allocates its own container, so a reentrant call gets a different
  one and cannot reach an outstanding activation's values. Naming an
  invariant is not choosing one, and the alternative — reusable storage plus
  a no-script-runs window — is rejected here because it can only be honoured
  by making the **numeric** accesses non-interceptable too, which is a
  different defect with its own fix.

  **The invariant is therefore narrowed, honestly, to reference results.**
  Numeric results in a mixed signature such as `(result externref i32
  externref)` remain interceptable: the publication loops interleave in
  signature order, so a replaced `Uint32Array` setter can re-enter between
  the two reference stores. Per-activation reference storage means the two
  externrefs still come from one activation and are not substituted — but the
  i32 between them can be. That exposure is **pre-existing and filed as
  `01a0821a-1093`**; this spec does not claim to close it and must not be
  read as doing so. Do not write "every result" anywhere in the
  implementation's comments.
- **The public result array is allocated intrinsically** — a fresh array with
  own data elements, created without invoking script — closing (c).

### Storage

`ArrayStorage` is the container: it already holds traced full `HermesValue`
elements, supports direct indexed access and barriered writes, and has its
tracing metadata. Allocating and accessing it through native builtins keeps
it entirely unreachable from script.

Two details that cost time to rediscover:

- **Allocate size as well as capacity** — `create(runtime, n, n)` — because
  indexed access checks `size()`, not capacity.
- **The caller must retain the container as a local IR value** through the
  final reference load. Today's code looks the buffer up in the module frame
  at each use; allocating fresh storage while keeping that lookup would
  allocate per activation and then read a shared object anyway, which is the
  contract in name only. Replacing the lookup IS the change.

Ownership is a real choice, not a detail. A **caller-owned** buffer threaded
as a hidden argument is the contract above. A **callee-owned** container also
works provided its handle is explicitly returned to the caller; what does not
work is the caller looking the container up in the callee's module, which is
defect (a) restated.

## 8. Testing

- **Identity round-trip, both directions.** JS writes an object through
  `.value`; Wasm reads it with `global.get` and returns it; assert `===`. With
  section 2 implemented, the `ref.func` mirror belongs here too: Wasm stores a
  wrapper, JS reads `.value`, and it is the same Exported Function the module
  exports by name.
- **Setter dispatch, per type.** `{}` into an externref global reads back the
  same object; a plain JS function into a funcref global raises TypeError.
- **The conversion points**, which no setter covers: an exported
  `(param funcref)` function called with a plain JS function must be refused,
  and an imported `(result funcref)` returning one likewise. The externref
  equivalents must be **accepted** — a test that refuses them would be wrong.
- **Exception payloads**: a funcref payload item that is an ordinary JS
  function must be refused; an externref payload of any value accepted.
- **The internal setter**, separately: a two-module test where the importer
  writes an imported mutable reference global via `global.set`, including from
  its **start function**, which runs during instantiation.
- **Link path**, both shapes, plus a raw plain function refused for a funcref
  import — the case needing the new predicate builtin.
- **The sentinel collision.** Import a global whose value **is** `null`, and
  another whose value is `undefined`. These must be **immutable snapshot**
  Globals: a mutable import discards the link result's value and keeps the
  object, so a mutable case passes without exercising the collision.
- **`ref.is_null`** on null and non-null references of both types, replacing
  `ref-is-null-unsupported.wat`.
- **Element expressions**: a segment mixing `ref.func`, `ref.null` and
  `global.get` entries, asserting each lands at the right index — the case
  that currently loses entries and shifts the rest. Plus passive and
  declarative segments containing `ref.null`, which must keep working.
- **Constructor conformance**, as a table against the node measurements.
- **v128 diagnosed**, message naming SIMD.
- **i64 unchanged**, including `2n**100n + 5n -> 5n`, plus a store-path test
  now that snapshot stores allocate.
- **GC.** Use an **immutable snapshot** global and eliminate other roots: an
  exported *mutable* global retains its reference through the closures and the
  module frame, so deleting `value_`'s metadata would not break it and the
  test would pass vacuously. Hold only the `Global` across a collection under
  `-gc-sanitize-handles=1`.
- **Multi-value reference results**, four separate cases, because they fail
  independently:
  - **Direct JS call** to a multi-value export returning a reference. This
    needs no second module and covers defect (c); the earlier claim that
    every such check needs two modules was wrong.
  - **Cross-module indirect**: B calls A's exported `(result i32 funcref)`
    through a table and receives the real reference — the stale-slot case.
  - **Cross-module direct import**, `B trampoline -> A wrapper -> A function`,
    asserting a substituted externref and a substituted-but-genuine Exported
    Function are both refused or, better, unreachable.
  - **Reentrancy, two shapes.** First, `globalThis.Array` replaced by storage
    whose setter re-enters an export: both a funcref and an **externref**
    result must survive unchanged, since an externref that merely passes a
    funcref check is not enough. Second, a **mixed** signature such as
    `(result externref i32 externref)` with `Uint32Array` and `Float64Array`
    replaced by storage supplying reentrant **getters and setters** — the
    two references must come from one activation. The i32 may be corrupted;
    that is `01a0821a-1093` and the test should assert the reference half
    only, with a comment saying why, so nobody later reads its silence as
    coverage.
  Add a **nested cross-module call whose inner signature has more results
  than the outer**, which is what catches an undersized forwarded buffer, and
  a GC during an outstanding multi-value call.
- **`ref.is_null` on optimized paths**, not just a literal `ref.null`: a
  parameter, a local, and an indirect-call result, each compiled at the
  default optimization level. A test using only `ref.null` would pass while
  the `Type::createObject()` folding bug remains.
- **Exception payloads**: a mixed `(i64 funcref)` payload, so the two-index
  i64 layout is exercised alongside the check, and an accessor-backed payload
  item that returns different values to two reads — which must not let an
  invalid funcref through.
- **Golden IR.** `irgen-memory-global-import-link.wat` pins the old
  link-result value in the import phi and **must change**; pin the new fetch
  to the matched-immutable branch specifically.

## Suggested implementation order

Not separately shippable — the capability is incomplete until all of it
lands — but this order keeps each step reviewable:

1. Storage and `ValType` (inert; nothing produces a reference value yet).
2. The predicate builtin and its registration.
3. The JS surface: constructor, both setters, the discriminator.
4. The link path.
5. The nullable IR annotation (`createObjectOrNull`), then `ref.is_null`,
   then function-body `ref.func`. The annotation must precede the opcode or
   the opcode is silently folded away.
6. Conversion points and exception payloads.
7. Per-activation reference-result storage, and its cross-module transport.
   Allocation is per multi-value call with a reference result, not per
   module — that is what makes reentrancy safe, and it is the step's whole
   point.
8. Element expressions and the `WasmElemSegment` model.

## Risks

- **Consumers of `ValType`.** One `-Wswitch` site; `wasmMakeGlobal`'s
  `> F64` ordering comparison is invisible to the compiler.
- **Allocating stores.** Four i64 sites become allocating; each needs rooting
  and failure checks.
- **The mode discriminator** touches a builtin the live-globals work just
  landed. Preserve live-implies-mutable; do not impose its converse.
- **Over-validating externref.** Every validation added here is funcref-only.
  A check on an externref path is a bug, not extra safety.
- **The element data model** is the largest single change and the one most
  likely to disturb golden IR.
- **Nullable annotation before the opcode.** Implementing `ref.is_null`
  against `Type::createObject()` produces a test that passes on a literal
  `ref.null` and fails on a local — the worst kind of green.
- **Reference-result storage** has four independent failure modes — transport,
  interception of the transport array, interception of the public result
  array, and reentrancy. A single-module direct JS call catches only the
  third; do not treat one passing test as coverage of the area.
- **The numeric precedent is unsafe to copy** and carries two filed defects:
  `01a0820d-5190` (nested-call buffer mismatch) and `01a0821a-1093`
  (interceptable storage). Read both before threading buffers.
- **The invariant is narrower than it looks.** Reference results are
  protected; numeric results in a mixed signature are not. Anyone extending
  this work must not assume the publication window covers everything.
