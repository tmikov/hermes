# Wasm Spec Test Status

Last updated: 2026-02-15 (branch `wasm`)

## Summary

| Metric | Value |
|--------|-------|
| Test files passing | 49 / 83 (59%) |
| Test files failing | 33 / 83 |
| Crashes | 0 |
| Timeouts | 1 |
| Assertions passing | 24,333 |
| Assertions failing | 359 |

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

## Passing Tests (49)

| Test | Passed | Failed | Skipped |
|------|--------|--------|---------|
| nop | 87 | 0 | 0 |
| type | 0 | 0 | 2 |
| start | 10 | 0 | 1 |
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
| binary-leb128 | 58 | 0 | 0 |
| comments | 3 | 0 | 0 |
| token | 0 | 0 | 26 |
| utf8-custom-section-id | 176 | 0 | 0 |
| utf8-import-field | 176 | 0 | 0 |
| utf8-import-module | 176 | 0 | 0 |
| utf8-invalid-encoding | 0 | 0 | 176 |
| endianness | 68 | 0 | 0 |

## Failing Tests (33)

### Failure Categories

The failures fall into several distinct categories. Notably, **no execution
semantics bugs remain** — all correctly-loaded modules produce correct results
for all non-NaN-bit-pattern operations. The failures are about missing features,
rejection of bad modules, and NaN-boxing limitations.

#### 1. wast2json Parse Errors (GC/Reference Type Proposals)

Some test files use syntax from newer Wasm proposals (GC types, typed function
references) that the bundled `wast2json` (from WABT) cannot parse. These fail
immediately with 0 pass / 1 fail before any assertions run.

**Affected tests:** br_if, br_table, local_tee, global, memory, table, elem,
select, align, unreached-valid, tag, ref_is_null, ref_null, linking

Example error:
```
br_if.wast:670:26: error: unexpected token "null", expected a numeric index
    (func $f (param (ref null $t)) (result funcref) (local.get 0))
```

#### 2. Missing Trap on Out-of-Bounds Memory Access

Memory loads/stores with large offsets that should trap (out of bounds) instead
succeed, returning incorrect values. The compiled code does not check whether
`base + offset` exceeds the memory size. This is the only category that
affects runtime correctness (though not for well-formed programs).

**Affected tests:** address (38)

Example: `i32.load offset=65536 (i32.const 0)` should trap but succeeds.

#### 3. Unlinkable / Uninstantiable Modules Not Rejected

Modules that should fail at instantiation time (e.g., out-of-bounds data
segments, incompatible imports) are instantiated successfully.

**Affected tests:** data (14), imports (106)

#### 4. Module Load Failures / Missing Features

Some modules fail to load (`invalid Wasm binary`) due to unsupported features
like multiple memories, certain import/export combinations, or advanced memory
operations.

**Affected tests:** memory_grow (49 — first module fails, cascading to all),
memory_redundancy (3), func (1)

#### 5. NaN-Boxing Limitations (Phase 2)

All Wasm f32/f64 values are stored as NaN-boxed `HermesValue`s on the register
stack. NaN-boxing reserves the sign bit and fraction bits of NaN patterns for
type tags (pointers, booleans, etc.), so only one canonical NaN bit pattern
survives — Hermes's internal NaN, which has a **negative** sign bit
(`0xFFF8...`). A positive-sign NaN (`0x7FF8...`) would be misinterpreted as a
tagged non-number value.

This causes two classes of failures:

**Copysign with NaN sign source:** `copysign(x, nan)` produces wrong-sign
results because the spec's positive NaN becomes Hermes's negative NaN.
`std::copysign(x, negative_NaN)` copies the negative sign, producing `-x`
instead of `+x`. The C++ implementation is correct — the sign bit is already
wrong before copysign sees it. 16 failures per file in f32/f64_bitwise.

**Reinterpret of NaN values:** `i32.reinterpret_f32` and `i64.reinterpret_f64`
return wrong bit patterns for NaN inputs because non-canonical NaN bit patterns
cannot survive on the register stack. 8 failures in conversions.

This cannot be fixed without a separate Wasm value representation that bypasses
NaN-boxing (Phase 2).

**Affected tests:** f32_bitwise (16), f64_bitwise (16), conversions (8)

#### 6. Miscellaneous Runtime Issues

Various remaining issues including call_indirect type mismatches, table
operation failures, and float expression edge cases.

**Affected tests:** call (3), call_indirect (13), float_exprs (8),
float_memory (10), func_ptrs (7), if (3), table_get (4), table_grow (23),
table_set (8), table_size (15)

### Detailed Failure Table

| Test | Passed | Failed | Skipped | Primary Failure Mode |
|------|--------|--------|---------|---------------------|
| func | 147 | 1 | 23 | Module load |
| func_ptrs | 25 | 7 | 0 | call_indirect |
| f32_bitwise | 347 | 16 | 0 | NaN-boxing copysign (16) |
| f64_bitwise | 347 | 16 | 0 | NaN-boxing copysign (16) |
| float_exprs | 811 | 8 | 0 | Float edge cases |
| float_memory | 50 | 10 | 0 | Float edge cases |
| conversions | 610 | 8 | 0 | NaN-boxing reinterpret (8) |
| if | 213 | 3 | 24 | Runtime |
| br_if | 0 | 1 | 0 | wast2json parse error |
| br_table | 0 | 1 | 0 | wast2json parse error |
| unreached-valid | 0 | 1 | 0 | wast2json parse error |
| call | 85 | 3 | 0 | Runtime |
| call_indirect | 143 | 13 | 11 | Type mismatch |
| local_tee | 0 | 1 | 0 | wast2json parse error |
| global | 0 | 1 | 0 | wast2json parse error |
| memory | 0 | 1 | 0 | wast2json parse error |
| memory_grow | 0 | 49 | 0 | Module load failure |
| memory_redundancy | 1 | 3 | 0 | Module load |
| address | 218 | 38 | 0 | OOB traps + offsets |
| align | 0 | 1 | 0 | wast2json parse error |
| data | 20 | 14 | 0 | Uninstantiable not rejected |
| table | 0 | 1 | 0 | wast2json parse error |
| elem | 0 | 1 | 0 | wast2json parse error |
| table_get | 10 | 4 | 0 | Missing feature |
| table_grow | 25 | 23 | 0 | Missing feature |
| table_set | 17 | 8 | 0 | Missing feature |
| table_size | 23 | 15 | 0 | Missing feature |
| select | 0 | 1 | 0 | wast2json parse error |
| imports | 22 | 106 | 16 | Unlinkable not rejected |
| tag | 0 | 1 | 0 | wast2json parse error |
| ref_is_null | 0 | 1 | 0 | wast2json parse error |
| ref_null | 0 | 1 | 0 | wast2json parse error |
| linking | 0 | 1 | 0 | wast2json parse error |
| binary | 0 | 0 | 0 | Timeout |

### Priority for Fixing

1. **Bounds checking** for memory access — the only runtime correctness issue
2. **Instantiation-time validation** (data segments, imports) — many tests blocked
3. **wast2json upgrade** — would unblock tests using newer proposal syntax
4. **Missing features** (multiple memories, table operations) — small scope
5. **NaN-boxing limitations** — requires Phase 2 (non-NaN-boxed Wasm values)
