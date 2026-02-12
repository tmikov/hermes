# Wasm Spec Test Status

Last updated: 2026-02-11 (branch `wasm`)

## Summary

| Metric | Value |
|--------|-------|
| Test files passing | 24 / 83 (29%) |
| Test files failing | 59 / 83 |
| Crashes | 0 |
| Timeouts | 0 |
| Assertions passing | 23,336 |
| Assertions failing | 1,463 |

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

## Passing Tests (24)

| Test | Passed | Failed | Skipped |
|------|--------|--------|---------|
| type | 0 | 0 | 2 |
| int_literals | 30 | 0 | 20 |
| int_exprs | 89 | 0 | 0 |
| const | 300 | 0 | 76 |
| float_literals | 99 | 0 | 78 |
| unreachable | 63 | 0 | 0 |
| forward | 4 | 0 | 0 |
| left-to-right | 95 | 0 | 0 |
| endianness | 68 | 0 | 0 |
| bulk | 66 | 0 | 0 |
| table_copy | 1,649 | 0 | 0 |
| stack | 5 | 0 | 0 |
| traps | 32 | 0 | 0 |
| unwind | 49 | 0 | 0 |
| names | 482 | 0 | 0 |
| custom | 8 | 0 | 0 |
| binary | 107 | 0 | 0 |
| binary-leb128 | 58 | 0 | 0 |
| comments | 3 | 0 | 0 |
| token | 0 | 0 | 26 |
| utf8-custom-section-id | 176 | 0 | 0 |
| utf8-import-field | 176 | 0 | 0 |
| utf8-import-module | 176 | 0 | 0 |
| utf8-invalid-encoding | 0 | 0 | 176 |

## Failing Tests (59)

### Failure Categories

The failures fall into several distinct categories:

#### 1. Incomplete Validator (`assert_invalid: validate returned true`)

The Wasm validator (`WebAssembly.validate`) accepts modules that should be
rejected. This is the **most common** failure mode, affecting nearly every
failing test file. The validator does not catch type errors, arity mismatches,
or other structural invalidity in many cases.

**Affected tests:** nop (4), f32 (9), f64 (9), f32_bitwise (9), f32_cmp (6),
f64_bitwise (9), f64_cmp (6), float_exprs (8), float_misc (6),
block (most of 155), if (most of 95), func (most of 52),
i32 (83), i64 (some), conversions (some), local_get (16), local_set (33),
load (46), store (51), memory_copy (64), memory_fill (64), memory_init (65),
table_fill (9), table_get (9), table_grow (some), table_init (67),
table_set (15), table_size (17), address (some), exports (most of 35),
memory_size (2), switch (1)

#### 2. wast2json Parse Errors (GC/Reference Type Proposals)

Some test files use syntax from newer Wasm proposals (GC types, typed function
references) that the bundled `wast2json` (from WABT) cannot parse. These fail
immediately with 0 pass / 1 fail before any assertions run.

**Affected tests:** br_if, br_table, local_tee, global, memory, table, elem,
select, align, unreached-valid, tag, ref_is_null, ref_null, linking, data (partially)

Example error:
```
br_if.wast:670:26: error: unexpected token "null", expected a numeric index
    (func $f (param (ref null $t)) (result funcref) (local.get 0))
```

#### 3. i64 Remaining Failures

i64 values are now correctly represented as BigInt at the JS/Wasm boundary.
Internal i64 arithmetic uses split lo/hi 32-bit representation. The remaining
i64 failures (29 in the i64 test) are primarily due to incomplete validation
and some edge cases in type conversions.

**Affected tests:** i64 (29), conversions (33 — mostly `f32.convert_i64_s/u`
and `f64.convert_i64_s/u`)

#### 4. Missing Trap on Out-of-Bounds Memory Access

Memory loads/stores with large offsets that should trap (out of bounds) instead
succeed, returning incorrect values. The compiled code does not check whether
`base + offset` exceeds the memory size.

**Affected tests:** address (38), load (some), store (some)

Example: `i32.load offset=65536 (i32.const 0)` should trap but succeeds.

#### 5. Module Load Failures / Missing Features

Some modules fail to load (`invalid Wasm binary`) due to unsupported features
like multiple memories, certain import/export combinations, or advanced memory
operations.

**Affected tests:** memory_grow (49 — first module fails, cascading to all),
memory_redundancy (3), start (3)

#### 6. Unlinkable / Uninstantiable Modules Not Rejected

Modules that should fail at instantiation time (e.g., out-of-bounds data
segments, incompatible imports) are instantiated successfully.

**Affected tests:** data (34), imports (most of 107), memory_fill (some),
memory_init (some)

#### 7. Global/Memory Export Issues

Exported globals return `undefined` instead of their values. Memory and table
exports may not work correctly.

**Affected tests:** exports (some — `get e` returns `undefined`), imports (some)

#### 8. f32.nearest / Math.round Semantics

`f32.nearest` uses `Math.round`, but Wasm's `nearest` uses banker's rounding
(round half to even), while JS `Math.round` rounds half up. This causes
`f32.nearest(0.5)` to return `1` instead of `0`.

**Affected tests:** f32 (1), float_misc (some)

#### 9. f32 Copysign Bit-Level Issues

`f32.copysign` and `f64.copysign` produce incorrect results for certain
sign/magnitude combinations, particularly involving signed zeros and
subnormals.

**Affected tests:** f32_bitwise (10), f64_bitwise (10)

### Detailed Failure Table

| Test | Passed | Failed | Skipped | Primary Failure Mode |
|------|--------|--------|---------|---------------------|
| nop | 83 | 4 | 0 | Validator |
| start | 7 | 3 | 1 | Module load |
| func | 96 | 52 | 23 | Validator |
| func_ptrs | 18 | 14 | 0 | Validator / call_indirect |
| i32 | 374 | 83 | 2 | Validator |
| i64 | 384 | 29 | 2 | Validator + conversions |
| f32 | 2,499 | 12 | 2 | Validator + nearest |
| f64 | 2,499 | 12 | 2 | Validator |
| f32_bitwise | 344 | 19 | 0 | Copysign + Validator |
| f32_cmp | 2,400 | 6 | 0 | Validator |
| f64_bitwise | 344 | 19 | 0 | Copysign + Validator |
| f64_cmp | 2,400 | 6 | 0 | Validator |
| float_exprs | 811 | 8 | 0 | Validator |
| float_memory | 50 | 10 | 0 | Validator |
| float_misc | 464 | 6 | 0 | Validator + nearest |
| conversions | 585 | 33 | 0 | i64 conversions |
| block | 52 | 155 | 15 | Validator (multi-value) |
| loop | 77 | 27 | 15 | Validator |
| if | 121 | 95 | 24 | Validator |
| br | 76 | 20 | 0 | Validator |
| br_if | 0 | 1 | 0 | wast2json parse error |
| br_table | 0 | 1 | 0 | wast2json parse error |
| switch | 26 | 1 | 0 | Validator |
| return | 63 | 20 | 0 | Validator |
| unreached-valid | 0 | 1 | 0 | wast2json parse error |
| labels | 25 | 3 | 0 | Validator |
| call | 67 | 21 | 0 | Validator |
| call_indirect | 119 | 37 | 11 | Validator + type mismatch |
| local_get | 19 | 16 | 0 | Validator |
| local_set | 19 | 33 | 0 | Validator |
| local_tee | 0 | 1 | 0 | wast2json parse error |
| global | 0 | 1 | 0 | wast2json parse error |
| memory | 0 | 1 | 0 | wast2json parse error |
| load | 37 | 46 | 13 | Validator + OOB |
| store | 9 | 51 | 7 | Validator + OOB |
| memory_grow | 0 | 49 | 0 | Module load failure |
| memory_size | 36 | 2 | 0 | Validator |
| memory_redundancy | 1 | 3 | 0 | Module load |
| address | 218 | 38 | 0 | OOB traps + offsets |
| align | 0 | 1 | 0 | wast2json parse error |
| memory_copy | 4,338 | 64 | 0 | Validator |
| memory_fill | 20 | 64 | 0 | Uninstantiable not rejected |
| memory_init | 142 | 65 | 0 | Uninstantiable not rejected |
| data | 0 | 34 | 0 | Uninstantiable not rejected |
| table | 0 | 1 | 0 | wast2json parse error |
| elem | 0 | 1 | 0 | wast2json parse error |
| table_fill | 35 | 9 | 0 | Validator |
| table_get | 5 | 9 | 0 | Validator |
| table_grow | 18 | 30 | 0 | Validator + missing feature |
| table_init | 662 | 67 | 0 | Uninstantiable not rejected |
| table_set | 10 | 15 | 0 | Validator |
| table_size | 21 | 17 | 0 | Validator |
| select | 0 | 1 | 0 | wast2json parse error |
| imports | 21 | 107 | 16 | Unlinkable not rejected |
| exports | 6 | 35 | 0 | Validator + global export |
| tag | 0 | 1 | 0 | wast2json parse error |
| ref_is_null | 0 | 1 | 0 | wast2json parse error |
| ref_null | 0 | 1 | 0 | wast2json parse error |
| linking | 0 | 1 | 0 | wast2json parse error |

### Priority for Fixing

1. **Validator completeness** — would fix the most tests with one effort
2. **Bounds checking** for memory access — needed for correctness/security
3. **Instantiation-time validation** (data segments, imports) — many tests blocked
4. **f32.nearest** semantics (banker's rounding) — easy, 1 assertion
5. **f32/f64.copysign** bit-level precision — affects bitwise tests
6. **wast2json upgrade** — would unblock tests using newer proposal syntax
