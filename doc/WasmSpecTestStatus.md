# Wasm Spec Test Status

Last updated: 2026-02-16 (branch `wasm`)

## Summary

| Metric | Value |
|--------|-------|
| Test files passing | 54 / 83 (65%) |
| Test files failing | 29 / 83 |
| Crashes | 0 |
| Timeouts | 0 |
| Assertions passing | 24,601 |
| Assertions failing | 228 |

## How to Run

```bash
python3 test/wasm/spec/run-all-spec-tests.py \
  --wast2json cmake-build-debug/external/wabt/wabt/wast2json \
  --hermes cmake-build-debug/bin/hermes \
  --testsuite external/wasm-testsuite/tests/
```

Individual test:
```bash
python3 test/wasm/spec/run-spec-test.py \
  --wast2json cmake-build-debug/external/wabt/wabt/wast2json \
  --hermes cmake-build-debug/bin/hermes \
  external/wasm-testsuite/tests/i32.wast
```

## Passing Tests (53)

| Test | Passed | Failed | Skipped |
|------|--------|--------|---------|
| nop | 87 | 0 | 0 |
| type | 0 | 0 | 2 |
| start | 10 | 0 | 1 |
| func | 148 | 0 | 23 |
| func_ptrs | 32 | 0 | 0 |
| i32 | 457 | 0 | 2 |
| i64 | 413 | 0 | 2 |
| f32 | 2,511 | 0 | 2 |
| f64 | 2,511 | 0 | 2 |
| f32_cmp | 2,406 | 0 | 0 |
| f64_cmp | 2,406 | 0 | 0 |
| float_literals | 99 | 0 | 78 |
| float_misc | 470 | 0 | 0 |
| int_exprs | 89 | 0 | 0 |
| int_literals | 30 | 0 | 20 |
| const | 300 | 0 | 76 |
| block | 207 | 0 | 15 |
| loop | 104 | 0 | 15 |
| br | 96 | 0 | 0 |
| switch | 27 | 0 | 0 |
| return | 83 | 0 | 0 |
| unreachable | 63 | 0 | 0 |
| labels | 28 | 0 | 0 |
| forward | 4 | 0 | 0 |
| left-to-right | 95 | 0 | 0 |
| call_indirect | 156 | 0 | 11 |
| local_get | 35 | 0 | 0 |
| local_set | 52 | 0 | 0 |
| load | 83 | 0 | 13 |
| store | 60 | 0 | 7 |
| memory_size | 38 | 0 | 0 |
| memory_copy | 4,402 | 0 | 0 |
| memory_fill | 84 | 0 | 0 |
| memory_init | 207 | 0 | 0 |
| bulk | 66 | 0 | 0 |
| table_copy | 1,649 | 0 | 0 |
| table_fill | 44 | 0 | 0 |
| table_init | 729 | 0 | 0 |
| stack | 5 | 0 | 0 |
| traps | 32 | 0 | 0 |
| unwind | 49 | 0 | 0 |
| names | 482 | 0 | 0 |
| exports | 41 | 0 | 0 |
| custom | 8 | 0 | 0 |
| binary | 107 | 0 | 0 |
| binary-leb128 | 58 | 0 | 0 |
| comments | 3 | 0 | 0 |
| token | 0 | 0 | 26 |
| utf8-custom-section-id | 176 | 0 | 0 |
| utf8-import-field | 176 | 0 | 0 |
| utf8-import-module | 176 | 0 | 0 |
| utf8-invalid-encoding | 0 | 0 | 176 |
| endianness | 68 | 0 | 0 |

## Failing Tests (29)

### Failure Categories

#### 1. NaN Bit Pattern Corruption (58 failures)

All Wasm f32/f64 values are stored as NaN-boxed `HermesValue`s on the register
stack. NaN-boxing reserves the sign bit and fraction bits of NaN patterns for
type tags (pointers, booleans, etc.), so only one canonical NaN bit pattern
survives — Hermes's internal NaN, which has a **negative** sign bit
(`0xFFF8...`). A positive-sign NaN (`0x7FF8...`) would be misinterpreted as a
tagged non-number value.

This causes four classes of failures:

**Copysign with NaN sign source (32 failures):** `copysign(x, nan)` produces
wrong-sign results because the spec's positive NaN becomes Hermes's negative
NaN. `std::copysign(x, negative_NaN)` copies the negative sign, producing `-x`
instead of `+x`.

**Reinterpret of NaN values (8 failures):** `i32.reinterpret_f32` and
`i64.reinterpret_f64` return wrong bit patterns for NaN inputs because
non-canonical NaN bit patterns cannot survive on the register stack.

**NaN bit patterns through linear memory (10 failures):** Storing a
non-canonical NaN to a Wasm register and then reading it back (or loading it
from memory after storing via the register) corrupts the bit pattern. This
affects `float_memory` tests where specific NaN bit patterns are stored to
linear memory and loaded back with `i32.load`/`i64.load`, returning
Hermes's canonical NaN bits instead of the original.

**Non-arithmetic NaN bit patterns (8 failures):** `float_exprs` tests
`f32.nonarithmetic_nan_bitpattern` and `f64.nonarithmetic_nan_bitpattern` check
that specific non-canonical NaN bit patterns survive through non-arithmetic
operations (reinterpret, load/store). They don't, because all NaN values
collapse to the canonical NaN on the register stack.

This cannot be fixed without a separate Wasm value representation that bypasses
NaN-boxing.

**Affected tests:** f32_bitwise (16), f64_bitwise (16), conversions (8),
float_memory (10), float_exprs (8)

#### 2. Missing Trap on Out-of-Bounds Memory Access (38 failures)

Memory loads/stores with large offsets that should trap (out of bounds) instead
succeed, returning incorrect values. The compiled code does not check whether
`base + offset` exceeds the memory size.

**Affected tests:** address (38)

Example: `i32.load offset=65536 (i32.const 0)` should trap but succeeds.

#### 3. Missing Trap on Out-of-Bounds Table Access (22 failures)

`table.get` and `table.set` with out-of-bounds indices succeed instead of
trapping. Same class of issue as the memory OOB problem (category 2) but for
table operations.

**Root cause:** In `lib/WasmIRGen/WasmIRGen.cpp`, `onTableGet()`
uses `createLoadPropertyInst(funcsArr, idx)` and `onTableSet()`
uses `createStorePropertyStrictInst(val, funcsArr, idx)`. These are JS
property operations — loading an out-of-bounds index from a JS array returns
`undefined` rather than trapping, and storing silently extends the array.
The Wasm spec requires a trap for out-of-bounds table access.

**Affected tests:** table_get (4), table_set (8), table_grow (10)

Example (table_get.wast):
```
get-externref: expected trap but succeeded
get-funcref: expected trap but succeeded
```

#### 4. Unlinkable / Uninstantiable Modules Not Rejected (33 failures)

Modules that should be rejected at instantiation time are accepted by Hermes.
The spec requires validation between parsing and execution; Hermes skips some
of these checks, so errors surface later (or not at all) as wrong results.

**imports (31 failures, 29 with patched test):** Import type validation is now implemented using
`__wasm_type__` string comparison at instantiation time. The compiled IR
checks each import value against the expected type string, throwing a
`WebAssembly.LinkError` on mismatch. This covers:

- Function signature mismatches (Wasm-to-Wasm and JS-to-Wasm)
- Global type and mutability mismatches (via `WebAssembly.Global` wrappers)
- Table/memory kind mismatches and limit validation
- Cross-kind mismatches (e.g., importing a memory as a function)
- Missing imports (undefined module or field)
- Non-callable values imported as functions
- Tag type mismatches (via `__wasm_type__` on tag export objects)

Remaining failures (31, or 29 with patched test) are due to:

- **~~Tag exports not implemented (3+1):~~** Fixed. Tag exports are now
  implemented as plain objects with `__wasm_type__` metadata (e.g.
  "tag:i:"), and tag import validation checks the type string. This fixed
  4 failures: the initial test module's tag exports are now present
  (unblocking the module at line 35 and its 2 cascading failures at lines
  97–98), and the cross-kind mismatch at line 256 is now correctly
  rejected.
- **~~Raw global exports lack type metadata (12):~~** Fixed. Global exports
  are now wrapped in `WebAssembly.Global` objects with `__wasm_type__`
  metadata, enabling cross-module global type and mutability validation.
- **Table/memory exports not implemented (26):** Tables and memories are not
  exported as `WebAssembly.Table`/`Memory` objects, so cross-module
  table/memory imports fail with "unknown import".
- **Alignment hint trusted for memory access (2):** The spec allows
  alignment hints on load/store instructions that are strictly advisory —
  implementations must produce correct results even when the actual address
  is less aligned than the hint declares. The current compiled code trusts
  alignment hints and uses typed array views (e.g., `HEAP32[addr >>> 2]`)
  which silently round the address down to the element boundary. Two test
  cases import a memory, write a byte via `data` segment at an unaligned
  offset, then `i32.load` at that offset with the default alignment hint
  (align=4). The address is a function parameter (not constant at compile
  time), so this cannot be detected statically. See "Alignment Hints
  Trusted" under Known Architectural Limitations. A patched copy of the
  test (`test/wasm/spec/imports_patched.wast_`) changes these two
  instructions to `align=1`, reducing the failure count by 2.
- **`memory.grow` on imported memory (4):** `memory.grow` operates on the
  locally-created ArrayBuffer, not the imported `WebAssembly.Memory`
  object, so grow has no effect on the shared memory.

**data (2 failures):** Active data segments have an offset expression (e.g.,
`(data (i32.const 65536) "hello")`). If the offset + data length exceeds the
memory size, the spec requires the module to trap during instantiation with an
"out of bounds memory access" error. Bounds checking is implemented for
`i32.const` offsets (both locally-defined and imported memories with non-zero
minimum), for `global.get` offsets on locally-defined memories (runtime
check), and for extended constant expressions (`i32.add`/`i32.sub`/`i32.mul`).
The remaining 2 failures (lines 89, 90) use `global.get` on non-imported
globals, which wast2json rejects.

**Fix approach — dependency graph:**

The remaining failures have the following dependency structure:

```
Export globals as WebAssembly.Global ──→ Global type validation works (12 fixed) ✓ DONE

Export tables as WebAssembly.Table ────→ Table imports resolve (12 fixed)
                                    └──→ Wire imported table into compiled code
                                          (needed for linking.wast, not imports.wast)

Export memories as WebAssembly.Memory ─→ Memory imports resolve (13 fixed)
                                    └──→ Wire imported memory into compiled code
                                     └──→ memory.grow on imported memory works (4 fixed)

Tags (independent) ───────────────────→ Tag import/export support (3+1 fixed) ✓ DONE
```

The memory path is the deepest: export → import resolution → wire buffer →
memory.grow. Tags are fully independent of all three but least impactful
(4 failures). Recommended order: memory exports (13 fixes from just the
export side), then table exports (12 fixes from just the export side).

**Affected tests:** imports (31), data (2)

#### 5. Module Load Failures / Missing Features (53 failures)

Hermes's Wasm binary parser rejects modules that use features it doesn't
support. The module validates fine through WABT, but Hermes's own validation
fails when loading the resulting `.wasm` binary.

**memory_grow (50):** The very first module in `memory_grow.wast` declares two
memories (`(memory (export "mem1") 2 5) (memory (export "mem2") 0)`). Hermes
doesn't support the multi-memory proposal and rejects any module with more than
one memory declaration. Since this first module fails to load, every subsequent
assertion (all 50) cascades to failure — the module instance is null and all
exported function calls fail with "Cannot read property ... of null". The
`memory.grow` instruction itself works; the failures are entirely due to
multi-memory rejection.

**memory_redundancy (3):** Similar module load failures due to unsupported
features.

**Affected tests:** memory_grow (50), memory_redundancy (3)

#### 6. Multi-Value Return from Calls Not Implemented (6 failures)

When a Wasm function returns multiple values (e.g., `(result i64 i32)`), only
the first return value survives. All additional return values are replaced with
`undefined`. This is an explicit limitation in WasmIRGen — the code comments
say `"Push undefined placeholders for additional results (multi-value)"`.

**Root cause:** Hermes IR functions can only return a single value via
`ReturnInst`. The i64 case already works around this with a side-channel stash
for the hi32 half, but there is no mechanism for passing additional return
values from multi-value functions.

**6 locations in `lib/WasmIRGen/WasmIRGen.cpp` are affected:**

1. **`onReturn()` (~line 1037):** Pops all results beyond the first and
   discards them. Only the first result is passed to `ReturnInst`.

2. **`endFunction()` (~line 887):** Same as `onReturn()` for the fallthrough
   return path at the end of a function body.

3. **`onCall()` (~line 2013):** After a call, pushes `undefined` for all
   results beyond the first instead of actual values.

4. **`onCallIndirect()` (~line 2081):** Same as `onCall()`.

5. **`createExportWrapper()` (~line 560):** Only marshals `results[0]` to JS.

6. **`createImportTrampoline()` (~line 652):** Only unmarshals the first
   result from a JS import call.

**Note:** Multi-value *blocks* (block/loop/if with params and results within a
single function) work correctly — the phi infrastructure handles them. Only
cross-function-boundary multi-value is broken.

**How the `if` test fails:** `add64_u_with_carry` returns `(i64, i32)` where
the i32 is a carry flag. The caller `add64_u_saturated` uses this carry as the
condition for `if (param i64) (result i64)`. But `onCall()` pushes `undefined`
for the carry, so the `if` condition is always falsy and the saturation branch
never executes.

**Affected tests:** call (3), if (3)

```
;; add64_u_with_carry returns (sum: i64, carry: i32) — carry is lost
(call $add64_u_with_carry (local.get 0) (local.get 1) (i32.const 0))
(if (param i64) (result i64)    ;; carry (i32) is condition, sum (i64) is param
  (then (drop) (i64.const -1))  ;; never reached because carry = undefined
)
```

```
call.wast line 306: as-binary-all-operands: expected 7 got 0
call.wast line 308: as-call-all-operands: expected [3, 4] got undefined
if.wast line 722: add64_u_saturated(-1, 1): expected UINT64_MAX got 0
if.wast line 725: add64_u_saturated(-1, -1): expected UINT64_MAX got -2
if.wast line 728: add64_u_saturated(MIN, MIN): expected UINT64_MAX got 0
```

**Fix approach:** Extend the existing i64 hi-stash pattern. For functions with
N>1 results, use N-1 additional stash slots (global-like variables) to pass
extra return values out-of-band. The callee stores extra results into stash
slots before `ReturnInst`, and the caller reads them after `CallInst`.

#### 7. wast2json Parse Errors (14 failures)

Test files using syntax from newer Wasm proposals (GC types, typed function
references) that the bundled `wast2json` (from WABT) cannot parse. These fail
immediately with 0 pass / 1 fail before any assertions run.

**Affected tests:** br_if, br_table, local_tee, global, memory, table, elem,
select, align, unreached-valid, tag, ref_is_null, ref_null, linking

Example error:
```
br_if.wast:670:26: error: unexpected token "null", expected a numeric index
    (func $f (param (ref null $t)) (result funcref) (local.get 0))
```

### Detailed Failure Table

| Test | Passed | Failed | Skipped | Primary Failure Mode |
|------|--------|--------|---------|---------------------|
| f32_bitwise | 347 | 16 | 0 | NaN copysign (cat 1) |
| f64_bitwise | 347 | 16 | 0 | NaN copysign (cat 1) |
| float_exprs | 811 | 8 | 0 | NaN bit patterns (cat 1) |
| float_memory | 50 | 10 | 0 | NaN through memory (cat 1) |
| conversions | 610 | 8 | 0 | NaN reinterpret (cat 1) |
| if | 213 | 3 | 24 | Multi-value return (cat 6) |
| br_if | 0 | 1 | 0 | wast2json parse error (cat 7) |
| br_table | 0 | 1 | 0 | wast2json parse error (cat 7) |
| unreached-valid | 0 | 1 | 0 | wast2json parse error (cat 7) |
| call | 85 | 3 | 0 | Multi-value return (cat 6) |
| local_tee | 0 | 1 | 0 | wast2json parse error (cat 7) |
| global | 0 | 1 | 0 | wast2json parse error (cat 7) |
| memory | 0 | 1 | 0 | wast2json parse error (cat 7) |
| memory_grow | 0 | 50 | 0 | Module load failure (cat 5) |
| memory_redundancy | 1 | 3 | 0 | Module load (cat 5) |
| address | 218 | 38 | 0 | Memory OOB (cat 2) |
| align | 0 | 1 | 0 | wast2json parse error (cat 7) |
| data | 38 | 2 | 0 | Uninstantiable (cat 4) |
| table | 0 | 1 | 0 | wast2json parse error (cat 7) |
| elem | 0 | 1 | 0 | wast2json parse error (cat 7) |
| table_get | 10 | 4 | 0 | Table OOB (cat 3) |
| table_grow | 38 | 10 | 0 | Table OOB (cat 3) |
| table_set | 17 | 8 | 0 | Table OOB (cat 3) |
| table_size | 38 | 0 | 0 | ✓ all pass |
| select | 0 | 1 | 0 | wast2json parse error (cat 7) |
| imports | 122 | 31 | 16 | Unlinkable (cat 4) |
| tag | 0 | 1 | 0 | wast2json parse error (cat 7) |
| ref_is_null | 0 | 1 | 0 | wast2json parse error (cat 7) |
| ref_null | 0 | 1 | 0 | wast2json parse error (cat 7) |
| linking | 0 | 1 | 0 | wast2json parse error (cat 7) |

### Priority for Fixing

1. **Memory bounds checking** (cat 2) — runtime correctness; OOB memory access
   succeeds instead of trapping. 38 failures.
2. **Table bounds checking** (cat 3) — runtime correctness; OOB table access
   succeeds instead of trapping. 22 failures.
3. **Multi-value call returns** (cat 6) — semantic correctness; multi-value
   returns from calls produce wrong results. 6 failures.
4. **Instantiation-time validation** (cat 4) — data segments, imports. 33
   failures.
5. **Module load failures** (cat 5) — multiple memories, etc. 53 failures.
6. **wast2json upgrade** (cat 7) — would unblock 14 test files using newer
   proposal syntax.
7. **NaN-boxing limitations** (cat 1) — requires non-NaN-boxed Wasm value
   representation. 58 failures.

## Known Architectural Limitations

These limitations are not yet surfaced as spec test failures because the
`linking.wast` test (which exercises cross-module scenarios) is blocked by a
wast2json parse error (cat 7). They will become visible once cross-module tests
can run.

### Non-Function Imports Not Wired In

**Function imports** and **global imports** are resolved from the imports
object and connected to the compiled module. Function imports are validated
for type compatibility using `__wasm_type__` strings. Global imports read
their value from the import object (either a `WebAssembly.Global`'s `.value`
property or a raw JS number).

Other import kinds are stubbed:

- **Memory imports ignored:** The compiled code always creates a fresh
  `ArrayBuffer` regardless of whether the module imports a memory
  (`WasmIRGen.cpp`, `createMemoryViews()`). The `WebAssembly.Memory` object
  from the imports object is never read. Two modules cannot share linear memory.

- **Table imports ignored:** The compiled code always creates fresh JS Arrays
  for table storage (`WasmIRGen.cpp`, `createTables()`). The
  `WebAssembly.Table` object from the imports object is never used. Functions
  from one module cannot appear in another module's table through imports.

### Memory and Table Exports Missing

The exports object includes function exports (with `__wasm_type__` metadata),
global exports (wrapped in `WebAssembly.Global` objects with `__wasm_type__`),
and tag exports (plain objects with `__wasm_type__`). Memory and table exports
are silently skipped
(`WasmIRGen.cpp`, `finalizeModule()`), so `instance.exports.memory` and
`instance.exports.table` are `undefined`. This prevents JS code from accessing
the module's memory or table, and prevents passing them as imports to other
modules.

### Alignment Hints Trusted

Wasm load/store instructions include an alignment hint (e.g., `align=4` for
`i32.load`). The spec says this hint is advisory: implementations must
produce correct results regardless of whether the actual effective address
satisfies the declared alignment. The hint exists so that engines targeting
native code can emit faster aligned-load instructions when the hint
guarantees alignment.

The current compiled code trusts alignment hints. When `alignLog2 ==
naturalAlign` (the common case, including all default-aligned loads/stores),
it uses typed array element access: `HEAP32[addr >>> 2]`,
`HEAPF64[addr >>> 3]`, etc. These typed array accesses implicitly round the
byte address down to the element boundary, silently reading/writing the
wrong bytes when the actual address is not aligned.

An unaligned byte-assembly path exists (`emitUnalignedLoad` /
`emitUnalignedStore` in `WasmIRGen.cpp`) and is used when `alignLog2 <
naturalAlign` — i.e., when the Wasm author explicitly declares sub-natural
alignment. But when the hint says "naturally aligned" and the runtime
address is not, the fast path is taken and produces incorrect results.

Always using the byte-assembly path would fix correctness but impose a
significant performance cost on every memory access. A runtime alignment
check (`if (addr & (align - 1))`) that branches to the slow path is
possible but adds IR complexity and branch overhead to every load/store.

In practice, well-formed Wasm compilers (LLVM, Binaryen) emit correct
alignment hints. The spec test deliberately passes incorrect hints to verify
engine robustness. This causes 2 failures in `imports.wast` (lines 502,
514).

### Cross-Module `call_indirect` Type Indices

The canonical type index map (commit d1907f2a3) is built per-module. If two
modules define the same function signature, they may assign different canonical
indices. When a function from module A is placed in module B's table (via shared
`WebAssembly.Table`), `call_indirect` in module B will compare module B's
canonical index against the index stored when module A populated the table — a
mismatch. Fixing this requires either a runtime-level cross-module type
registry or structural signature comparison at call time.
