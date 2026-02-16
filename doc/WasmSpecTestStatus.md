# Wasm Spec Test Status

Last updated: 2026-02-16 (branch `wasm`)

## Summary

| Metric | Value |
|--------|-------|
| Test files passing | 54 / 83 (65%) |
| Test files failing | 29 / 83 |
| Crashes | 0 |
| Timeouts | 0 |
| Assertions passing | 24,499 |
| Assertions failing | 307 |

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

#### 4. Unlinkable / Uninstantiable Modules Not Rejected (116 failures)

Modules that should be rejected at instantiation time are accepted by Hermes.
The spec requires validation between parsing and execution; Hermes skips most
of these checks, so errors surface later (or not at all) as wrong results.

**imports (106 failures):** The spec requires that every import is matched
against the provided imports object and validated for type compatibility at
instantiation time. Mismatches must produce a `LinkError`. For example:

- Importing a function with the wrong signature should fail
- Importing a memory or table with limits that don't satisfy the declared
  minimums/maximums should fail
- A missing import should fail

Hermes currently does minimal or no validation of imports at link time. If a
module declares `(import "env" "foo" (func (param i32) (result i32)))` but the
host provides a function with a different signature (or a non-function),
instantiation succeeds. The mismatch only surfaces later at call time (if at
all), producing wrong results instead of an upfront `LinkError`.

**data (10 failures):** Active data segments have an offset expression (e.g.,
`(data (i32.const 65536) "hello")`). If the offset + data length exceeds the
memory size, the spec requires the module to trap during instantiation with an
"out of bounds memory access" error. Bounds checking for `i32.const` offsets
with locally-defined memories is now implemented. The remaining 10 failures
are: 6 module-load failures from unsupported offset expressions (`global.get`,
extended constant expressions like `i32.add`/`i32.sub`/`i32.mul`), 1
`global.get` offset bounds check (needs runtime check), and 3 imported-memory
bounds checks (the actual imported memory size is only known at runtime).

**Fix approach:** Add validation passes in the instantiation path — check
import compatibility (function signatures, memory/table limits) and data/element
segment bounds against actual memory/table sizes before any exported functions
are called.

**Affected tests:** imports (106), data (10)

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
| data | 30 | 10 | 0 | Uninstantiable (cat 4) |
| table | 0 | 1 | 0 | wast2json parse error (cat 7) |
| elem | 0 | 1 | 0 | wast2json parse error (cat 7) |
| table_get | 10 | 4 | 0 | Table OOB (cat 3) |
| table_grow | 38 | 10 | 0 | Table OOB (cat 3) |
| table_set | 17 | 8 | 0 | Table OOB (cat 3) |
| table_size | 38 | 0 | 0 | ✓ all pass |
| select | 0 | 1 | 0 | wast2json parse error (cat 7) |
| imports | 22 | 106 | 16 | Unlinkable (cat 4) |
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
4. **Instantiation-time validation** (cat 4) — data segments, imports. 116
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

Only **function imports** are resolved from the imports object and connected to
the compiled module. All other import kinds are stubbed:

- **Memory imports ignored:** The compiled code always creates a fresh
  `ArrayBuffer` regardless of whether the module imports a memory
  (`WasmIRGen.cpp`, `createMemoryViews()`). The `WebAssembly.Memory` object
  from the imports object is never read. Two modules cannot share linear memory.

- **Table imports ignored:** The compiled code always creates fresh JS Arrays
  for table storage (`WasmIRGen.cpp`, `createTables()`). The
  `WebAssembly.Table` object from the imports object is never used. Functions
  from one module cannot appear in another module's table through imports.

- **Global imports hard-coded to zero:** Imported globals are initialized to 0
  (`WasmIRGen.cpp`, `initializeGlobals()`). The actual value from the imports
  object is ignored, even though `WebAssembly.Global` JS API objects are fully
  implemented.

### Memory and Table Exports Missing

The exports object only includes function exports and (immutable) global
snapshots. Memory and table exports are silently skipped
(`WasmIRGen.cpp`, `finalizeModule()`), so `instance.exports.memory` and
`instance.exports.table` are `undefined`. This prevents JS code from accessing
the module's memory or table, and prevents passing them as imports to other
modules.

### Cross-Module `call_indirect` Type Indices

The canonical type index map (commit d1907f2a3) is built per-module. If two
modules define the same function signature, they may assign different canonical
indices. When a function from module A is placed in module B's table (via shared
`WebAssembly.Table`), `call_indirect` in module B will compare module B's
canonical index against the index stored when module A populated the table — a
mismatch. Fixing this requires either a runtime-level cross-module type
registry or structural signature comparison at call time.
