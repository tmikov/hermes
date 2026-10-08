# Wasm linking ABI on internal slots — design

**Status:** approved 2026-08-23. Supersedes the "record and defer" decision of
2026-08-03 (REVIEW.md §5.8) and the "dedicated later pass" note of 2026-08-15
(§4.4).

**Goal.** Move the WebAssembly linking ABI off script-visible JS properties and
onto native-only state, so that the objects crossing a module boundary cannot be
forged, mutated or enumerated by script. Absorbs H16, merge items 23/24, and
retires J4's interim float coercion.

## 1. What is wrong today

Everything that crosses a module boundary travels as an ordinary **writable,
enumerable, configurable** JS data property, and the import path validates by
string and number comparison in generated IR rather than by any brand check.

| property | carries | published at |
|---|---|---|
| `__wasm_funcs__`, `__wasm_types__` | table contents and type tags | `WebAssembly.cpp:1749,1756`; `WasmIRGen.cpp:2237,2242` |
| `__wasm_type__`, `__wasm_min__`, `__wasm_max__` | memory/table/global type and limits | 7 sites in `WebAssembly.cpp` |

Read back at 4 sites in the table case (`WasmIRGen.cpp:1097,1191,1207,6834`) and
10 across the limits properties.

Reproduced on the tip build (`e042066fa`); repros preserved in
`handoff-artifacts/`:

1. **VM abort from ordinary JS against a trusted `.hbc`.** `tbl.__wasm_funcs__[0]`
   — or equivalently `tbl.get(0)` — hands script the raw internal closure, whose
   ABI is the internal one (i64 as a lo/hi pair, return-buffer params). Calling it
   with the spec-legal `5n` reads a BigInt as a double:
   `Assertion 'isDouble()' failed` in Debug, undefined behaviour in Release.
2. **Type confusion.** `tbl.__wasm_types__[0] = <forged id>`, or `tbl.set()` /
   wasm `table.set` leaving a stale id, makes `call_indirect`'s check pass on a
   function of a different signature: a `(param i32 i32 i32)` function invoked
   with zero arguments, returning `undefined` where the value stack requires an
   i32.
3. **Missing spec validation.** `Table.prototype.set` accepts any callable; the
   JS-API spec requires null or an Exported Function.
4. **Identity deviation.** `tbl.get(0) !== exports.dbl`. One wrapper is created
   per *export entry* (`WasmIRGen.cpp:1963-1972`), so a function exported twice
   yields two objects, and a table function that is not exported has none.
5. **Conformance.** `Object.keys(new WebAssembly.Memory({initial:1}))` returns the
   metadata names and `JSON.stringify` leaks them. Real engines expose no own
   enumerable properties on Memory/Table/Global.

Three IRGen sites also fail to maintain the parallel types array at all —
`onTableSet` (`:6993`), `onTableFill` (`:7072`), and JS `Table.prototype.set`
(`WebAssembly.cpp:1861`) — which is the immediate cause of (2).

## 2. Principle

The linking ABI becomes native-only state, reachable from generated code solely
through builtins that brand-check their argument. Script sees Exported Functions
and the standard `WebAssembly.*` accessor methods, and nothing else.

Two facts make this cheap, both verified against the tree:

- **The hot path never touches the JS properties.** `call_indirect` reads
  `loadTableFuncs`/`loadTableTypes` (`WasmIRGen.cpp:6947,6954`), which load from
  **top-level scope Variables**, not from the table object. The cost of this
  change therefore lands at instantiate time, not per indirect call.
- **The internal state already exists.** `JSWebAssemblyTable` has `elements_` and
  `types_`; `JSWebAssemblyMemory` has `buffer_`/`maxPages_`;
  `JSWebAssemblyGlobal` has `value_`/`valType_`/mutability. The `__wasm_*`
  properties are a *parallel publication*, not the storage. This pass deletes the
  publication.

**No new CellKind.** Hermes already supports named internal properties for
exactly this purpose — `InternalProperties.def` documents them as "used to store
Hermes internal state on arbitrary objects" (precedents: `CapturedError`,
`NativeState`, `NapiData`, `IntlNativeType`), and `JSObject.cpp:614,2550,2618`
filter them from enumeration, so they are invisible to `Object.keys`,
`getOwnPropertyNames` *and* `getOwnPropertySymbols`. Script cannot create or
write one, because the symbol IDs are predefined below `NumInternalProperties`.
A new cell class would add only a cheaper brand test and typed accessors, which
does not justify the cost of a CellKind, its instructions and its metadata.

## 3. The Exported Function

A single canonical Exported Function per function index, created at instantiate
time for every function in `escapableFuncs_ ∪ exported functions ∪ imported
function indices`, and held in a top-level scope Variable array. Both the
exports object and table writes use that same object, which fixes (4).

It is an ordinary JS function carrying two new named internal properties:

- `WasmFuncClosure` — the internal closure it wraps. For an *imported* function
  index that closure is the import trampoline, so a JS function placed in a table
  through an element segment is reached the same way as a native one.
- `WasmFuncTypeId` — the *interned* type id (`typeIdVars_`), not a module-local
  index. Interned ids agree across modules; module-local ones do not.

Presence of `WasmFuncClosure` is the brand: it is precisely the spec's "is an
Exported Function" predicate, and it cannot be forged **by script**.
The qualification is exact rather than pedantic: `wasmSetFuncInfo` is a
`PRIVATE_BUILTIN`, and a `PRIVATE_BUILTIN` is reachable from any bytecode that
emits a `CallBuiltin` with its index. Bytecode is trusted, so this is out of
the threat model — but the builtin type-checks its arguments rather than
relying on the sentence being unqualified.

**A funcref value is the Exported Function, everywhere** — on the Wasm value
stack, in the reference slots of the return buffer added by `e042066fa`, in
funcref globals, and in JS. This is safe because nothing in Wasm calls a funcref
directly: `call_ref` is not supported, and `call_indirect` dispatches by table
index, not by value. The opcode handlers that consume a funcref are `table.set`,
`table.fill`, `table.grow`, `ref.is_null`, returns, and global set — none of
which call it.

*Consequence:* internal closures never appear on the value stack and are never
published, so no route hands one to script. This is the precondition for §7.

## 4. Table representation

Three parallel arrays per table, all of the same length, held as internal fields
on `JSWebAssemblyTable` and mirrored into top-level scope Variables for generated
code:

| array | holds | read by |
|---|---|---|
| `elements_` | internal closures | `call_indirect` (hot path) |
| `types_` | interned type ids | `call_indirect` (hot path) |
| `exported_` | Exported Functions, or null | every JS boundary crossing |

The types array is **retained**, not derived from the wrapper. Deriving it would
turn a hot-path array-element read into a hidden-class property lookup.

Note the division: the table *stores* internal closures (for the hot path) while
the *value stack* carries wrappers (§3). So `onTableGet` (`:6982`) changes from
reading `elements_[i]` to reading `exported_[i]` — this is the single change that
stops closures reaching the value stack, and from there script.

**Invariant.** For every slot `i`, either the slot is empty (`elements_[i]` is
null in all three), or `elements_[i]` is the internal closure, `types_[i]` its
interned type id, and `exported_[i]` the unique wrapper whose internal properties
hold that same closure and id. Never two of three.

**The invariant is enforced by funnelling, not by discipline.** Ten sites write
table slots today and two of them are already wrong. So the write is not spread:
two native helpers own it, and every site calls one of them.

- `wasmTableSetSlot(table, i, exportedFn)` — derives closure and type id from the
  wrapper's internal properties and writes all three arrays.
- `wasmTableCopySlots(dst, dstIdx, src, srcIdx, n)` — range copy across all three.

Callers, all of which must be converted:

| site | today |
|---|---|
| element-segment init (`WasmIRGen.cpp:6892`) | funcs + types |
| `onTableSet` (`:6993`) | **funcs only — defect** |
| `onTableFill` (`:7072`) | **funcs only — defect** |
| `onTableGrow` (`:7014`) | funcs + types |
| `onTableCopy` (`:7085`) | funcs + types |
| `onTableInit` (`:7104`) | funcs + types |
| JS `Table` constructor | allocates two, must allocate three |
| JS `Table.prototype.set` (`WebAssembly.cpp:1861`) | **elements only — defect** |
| JS `Table.prototype.grow` | two |
| import link path (`WasmIRGen.cpp:1241`) | adopts two |

**Cross-module sharing is preserved.** Sharing works because the importer stores
the *same* array objects into its own top-level Variables; a third shared array
behaves identically. `test/wasm/e2e-table-import-advanced` — the exporter
observing an element segment and a `table.grow` performed by the importer — must
keep passing, and is the regression test for this (§5.8 constraint 2).

**Wrappers are not put in the table.** `elements_` keeps internal closures, so
`emitCallIndirect` is unchanged and same-module indirect calls pay nothing
(§5.8 constraint 1).

## 5. Link path

Each read of a `__wasm_*` property on the import path is replaced by a native
builtin that brand-checks with `dyn_vmcast` and returns the internal state:

- `wasmLinkTable(importVal, expectedMin, expectedMax)` → the three arrays.
- `wasmLinkMemory(importVal, expectedMin, expectedMax)` → the buffer and views.
- `wasmLinkGlobal(importVal, expectedType, expectedMutability)` → the global.

A brand check subsumes merge items 23/24, which proposed `instanceof`: a
`dyn_vmcast` cannot be spoofed by a prototype or an object literal, whereas
`instanceof` can be. Those two rows become satisfied rather than reimplemented.

Failures raise `LinkError` naming the import and the reason, matching the message
style established by `2f7b135e6`. Reading limits from internal fields at use time
also dissolves H7 (limits were snapshots never updated by `grow`) and closes H2's
window (metadata was written with `putNamed_RJS`, which walks the prototype chain
and can run a user setter inside the native constructor).

Note on `WasmIRGen.cpp:1092-1115`: this currently branches on whether
`__wasm_funcs__` exists to distinguish a Wasm-exported table from a JS-supplied
one. Since the JS `Table` constructor publishes the property too, that test is in
truth "is this a genuine Table versus a forged literal", so the brand check is an
exact replacement — but it must be pinned by a test, because it reads as a
refactor and is not.

## 6. Boundary conversions

Outbound (wasm → JS), hand out `exported_[i]` / the wrapper, never a closure:
`Table.prototype.get`; funcref results from export wrappers, including the
reference return slots added by `e042066fa`; funcref globals if their export is
ever enabled (`finalizeModule` rejects it today, so this is latent).

Inbound (JS → wasm), require the brand and map back through
`WasmFuncClosure`: `Table.prototype.set` (TypeError otherwise, per
`ToWebAssemblyValue` for `funcref`); funcref parameters.

## 7. Retiring the J4 coercion

`WasmIRGen.cpp:762-768` types f32/f64 parameters of escapable functions as `:any`
and coerces at entry, because such a closure could be called from JS with any
argument. Once no route hands script an internal closure, every caller is Wasm
and the annotation is honest again, so the coercion and `escapableFuncs_`'s role
in parameter typing are removed and float params return to `:number`. The comment
at `:757-761` predicts exactly this.

**Proof obligation, not an assumption.** Removal requires enumerating every route
by which a closure could reach script and showing each now yields a wrapper:
table slots, `__wasm_funcs__` (deleted), export wrappers, funcref globals, funcref
results, import trampoline arguments. A test must call every such route and assert
the result is a wrapper. The J4 removal lands only after that test exists.

## 8. What this resolves

§4.4's list — F1, F2, F3, H2, H3, H6, H7, H9, J4, K1, K3, G2 — plus H16's five
symptoms, the three stale-types sites, merge items 23/24, and the
`Object.keys`/`JSON.stringify` conformance gap that no point fix addresses.

## 9. Testing

- **Conformance:** `Object.keys`, `getOwnPropertyNames`, `getOwnPropertySymbols`
  and `JSON.stringify` return nothing for Memory, Table and Global.
- **The abort repro** (`handoff-artifacts/h16d.js`) behaves per spec, and
  `__wasm_funcs__` is `undefined`.
- **Type confusion:** the `h16b` repro traps instead of calling the wrong
  signature; the forged-`__wasm_types__` variant is impossible because the
  property is gone.
- **Brand checks:** an object literal shaped like a Table/Memory/Global raises
  LinkError, not a successful link.
- **Identity:** `exports.f === tbl.get(i)`, and a function exported under two
  names yields one object.
- **Invariant:** after each mutating operation, all three arrays agree.
- **Cross-module:** `e2e-table-import-advanced` unchanged.
- **J4:** the escape-route enumeration test of §7.
- **Mutation proof is a gate.** Every assertion above must be shown to fail under
  a targeted mutation, per the standing convention on this branch. An assertion
  that cannot fail does not count as coverage.

## 10. Out of scope

`ref.func` in function bodies and `call_ref` (still unsupported); V128; the
`BYTECODE_VERSION` bump (explicitly last on this branch); the Worker gate (after
the `static_h` rebase).

## 11. Risks and open items

- **Golden IR churn will be large.** The link path changes shape, so many
  `%FileCheckOrRegen` goldens move. `update-lit` handles the mechanics, but the
  diff must be read, not regenerated blindly.
- **Bytecode format.** Generated code gains builtin calls; this feeds the queued
  `BYTECODE_VERSION` bump rather than being separate from it.
- **Assumption to state, not assume.** "Script cannot reach a closure Variable"
  holds for the normal path; the debugger API is a separate surface and the spec
  records this rather than leaving it implicit.
- **Spec text to confirm during implementation.** That `ToWebAssemblyValue` for
  `funcref` requires an Exported Function or null is stated here from memory and
  must be checked against the JS-API text before the TypeError is pinned by a
  test.
