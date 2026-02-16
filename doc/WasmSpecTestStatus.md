# Wasm Spec Test Status

Last updated: 2026-02-15 (branch `wasm`)

## Summary

| Metric | Value |
|--------|-------|
| Test files passing | 53 / 83 (64%) |
| Test files failing | 30 / 83 |
| Crashes | 0 |
| Timeouts | 0 |
| Assertions passing | 24,461 |
| Assertions failing | 345 |

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

## Failing Tests (30)

### Failure Categories

#### 1. table.grow Not Implemented (38 failures)

`table.grow` always returns -1 (failure) instead of growing the table. This
causes all table_grow tests to fail directly, and table_size tests to fail as
a consequence (sizes remain unchanged after failed grows).

**Root cause:** In `lib/WasmIRGen/WasmIRGen.cpp` line 4667, `onTableGrow()`
pops both operands (delta and fill value) and unconditionally pushes -1:

```cpp
void WasmIRGen::onTableGrow(uint32_t tableIndex) {
  // Phase 1: not fully implemented — always returns -1 (failure).
  pop(); // delta
  pop(); // fill value
  push(builder_.getLiteralNumber(-1));
}
```

This is technically spec-compliant (table.grow is allowed to fail), but it
means no table can ever be grown at runtime.

**Affected tests:** table_grow (23), table_size (15)

Example (table_grow.wast):
```
grow: expected [{'type': 'i32', 'value': '0'}] got -1
size: expected [{'type': 'i32', 'value': '5'}] got 2
```

#### 2. NaN Bit Pattern Corruption (58 failures)

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

#### 3. Missing Trap on Out-of-Bounds Memory Access (38 failures)

Memory loads/stores with large offsets that should trap (out of bounds) instead
succeed, returning incorrect values. The compiled code does not check whether
`base + offset` exceeds the memory size.

**Affected tests:** address (38)

Example: `i32.load offset=65536 (i32.const 0)` should trap but succeeds.

#### 4. Missing Trap on Out-of-Bounds Table Access (12 failures)

`table.get` and `table.set` with out-of-bounds indices succeed instead of
trapping. Same class of issue as the memory OOB problem (category 3) but for
table operations.

**Root cause:** In `lib/WasmIRGen/WasmIRGen.cpp`, `onTableGet()` (line 4637)
uses `createLoadPropertyInst(funcsArr, idx)` and `onTableSet()` (line 4647)
uses `createStorePropertyStrictInst(val, funcsArr, idx)`. These are JS
property operations — loading an out-of-bounds index from a JS array returns
`undefined` rather than trapping, and storing silently extends the array.
The Wasm spec requires a trap for out-of-bounds table access.

**Affected tests:** table_get (4), table_set (8)

Example (table_get.wast):
```
get-externref: expected trap but succeeded
get-funcref: expected trap but succeeded
```

#### 5. Unlinkable / Uninstantiable Modules Not Rejected (126 failures)

Modules that should fail at instantiation time (e.g., out-of-bounds data
segments, incompatible imports) are instantiated successfully.

**Affected tests:** imports (106), data (20)

#### 6. Module Load Failures / Missing Features (53 failures)

Some modules fail to load (`invalid Wasm binary`) due to unsupported features
like multiple memories, certain import/export combinations, or advanced memory
operations.

**Affected tests:** memory_grow (50 — first module fails to load, cascading to
all subsequent assertions), memory_redundancy (3)

#### 7. Multi-Value Return from Calls Not Implemented (6 failures)

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

#### 8. wast2json Parse Errors (14 failures)

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
| f32_bitwise | 347 | 16 | 0 | NaN copysign (cat 2) |
| f64_bitwise | 347 | 16 | 0 | NaN copysign (cat 2) |
| float_exprs | 811 | 8 | 0 | NaN bit patterns (cat 2) |
| float_memory | 50 | 10 | 0 | NaN through memory (cat 2) |
| conversions | 610 | 8 | 0 | NaN reinterpret (cat 2) |
| if | 213 | 3 | 24 | Multi-value return (cat 7) |
| br_if | 0 | 1 | 0 | wast2json parse error (cat 8) |
| br_table | 0 | 1 | 0 | wast2json parse error (cat 8) |
| unreached-valid | 0 | 1 | 0 | wast2json parse error (cat 8) |
| call | 85 | 3 | 0 | Multi-value return (cat 7) |
| local_tee | 0 | 1 | 0 | wast2json parse error (cat 8) |
| global | 0 | 1 | 0 | wast2json parse error (cat 8) |
| memory | 0 | 1 | 0 | wast2json parse error (cat 8) |
| memory_grow | 0 | 50 | 0 | Module load failure (cat 6) |
| memory_redundancy | 1 | 3 | 0 | Module load (cat 6) |
| address | 218 | 38 | 0 | Memory OOB (cat 3) |
| align | 0 | 1 | 0 | wast2json parse error (cat 8) |
| data | 20 | 20 | 0 | Uninstantiable (cat 5) |
| table | 0 | 1 | 0 | wast2json parse error (cat 8) |
| elem | 0 | 1 | 0 | wast2json parse error (cat 8) |
| table_get | 10 | 4 | 0 | Table OOB (cat 4) |
| table_grow | 25 | 23 | 0 | table.grow unimplemented (cat 1) |
| table_set | 17 | 8 | 0 | Table OOB (cat 4) |
| table_size | 23 | 15 | 0 | table.grow unimplemented (cat 1) |
| select | 0 | 1 | 0 | wast2json parse error (cat 8) |
| imports | 22 | 106 | 16 | Unlinkable (cat 5) |
| tag | 0 | 1 | 0 | wast2json parse error (cat 8) |
| ref_is_null | 0 | 1 | 0 | wast2json parse error (cat 8) |
| ref_null | 0 | 1 | 0 | wast2json parse error (cat 8) |
| linking | 0 | 1 | 0 | wast2json parse error (cat 8) |

### Priority for Fixing

1. **table.grow implementation** (cat 1) — entirely missing feature. 38
   failures, blocks table_size tests too.
2. **Memory bounds checking** (cat 3) — runtime correctness; OOB memory access
   succeeds instead of trapping. 38 failures.
3. **Table bounds checking** (cat 4) — runtime correctness; OOB table access
   succeeds instead of trapping. 12 failures.
4. **Multi-value call returns** (cat 7) — semantic correctness; multi-value
   returns from calls produce wrong results. 6 failures.
5. **Instantiation-time validation** (cat 5) — data segments, imports. 126
   failures.
6. **Module load failures** (cat 6) — multiple memories, etc. 53 failures.
7. **wast2json upgrade** (cat 8) — would unblock 14 test files using newer
   proposal syntax.
8. **NaN-boxing limitations** (cat 2) — requires non-NaN-boxed Wasm value
   representation. 58 failures.
