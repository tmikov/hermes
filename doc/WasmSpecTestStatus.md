# Wasm Spec Test Status

Last updated: 2026-02-18 (branch `wasm`)

## Summary

| Metric | Value |
|--------|-------|
| Test files passing | 62 / 83 (75%) |
| Test files failing | 21 / 83 |
| Crashes | 0 |
| Timeouts | 0 |
| Assertions passing | 24,682 |
| Assertions failing | 120 |

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

## Passing Tests (62)

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
| if | 216 | 0 | 24 |
| br | 96 | 0 | 0 |
| switch | 27 | 0 | 0 |
| return | 83 | 0 | 0 |
| unreachable | 63 | 0 | 0 |
| labels | 28 | 0 | 0 |
| forward | 4 | 0 | 0 |
| left-to-right | 95 | 0 | 0 |
| call | 88 | 0 | 0 |
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
| table_get | 14 | 0 | 0 |
| table_grow | 48 | 0 | 0 |
| table_init | 729 | 0 | 0 |
| table_set | 25 | 0 | 0 |
| table_size | 38 | 0 | 0 |
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
| address | 256 | 0 | 0 |
| imports | 128 | 0 | 16 |
| memory_redundancy | 4 | 0 | 0 |

## Failing Tests (21)

### Failure Categories

#### 1. NaN Bit Pattern Corruption (54 failures)

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

**NaN bit patterns through linear memory (6 failures):** Storing a
non-canonical NaN to a Wasm register and then reading it back (or loading it
from memory after storing via the register) corrupts the bit pattern. This
affects `float_memory` tests where specific NaN bit patterns are stored to
linear memory and loaded back with `i32.load`/`i64.load`, returning
Hermes's canonical NaN bits instead of the original. (4 of the original 10
failures were actually alignment issues, now fixed by the unaligned path.)

**Non-arithmetic NaN bit patterns (8 failures):** `float_exprs` tests
`f32.nonarithmetic_nan_bitpattern` and `f64.nonarithmetic_nan_bitpattern` check
that specific non-canonical NaN bit patterns survive through non-arithmetic
operations (reinterpret, load/store). They don't, because all NaN values
collapse to the canonical NaN on the register stack.

This cannot be fixed without a separate Wasm value representation that bypasses
NaN-boxing.

**Affected tests:** f32_bitwise (16), f64_bitwise (16), conversions (8),
float_memory (6), float_exprs (8)

#### ~~2. Missing Trap on Out-of-Bounds Memory Access (was 38 failures — FIXED)~~

Fixed by adding explicit memory bounds checking to `onLoad()` and `onStore()`
in `lib/WasmIRGen/WasmIRGen.cpp`, gated behind the `--test262` flag (now passed
automatically by `run-spec-test.py`). New helpers `emitEffectiveAddr()` (treats
the base address as unsigned via `>>> 0` to prevent signed wrap-around) and
`emitMemoryBoundsCheck()` (emits `if (addr + numBytes > HEAPU8.length) trap`)
catch OOB accesses before they reach the typed array views.

Results: address (38 → 0). The original 9 alignment-related failures were fixed
by forcing all multi-byte loads/stores through the byte-assembly (unaligned)
path when `--test262` is active (`if (test262_) alignLog2 = 0` in `onLoad()`
and `onStore()`).

**Previously affected tests:** address (38 → 0)

#### ~~3. Missing Trap on Out-of-Bounds Table Access (was 26 failures — FIXED)~~

Fixed by adding bounds checking to `onTableGet()` and `onTableSet()` in
`lib/WasmIRGen/WasmIRGen.cpp`. A new helper `emitTableBoundsCheck()` emits an
unsigned comparison of the index against the table array's length, branching to
a trap block on OOB. This follows the same pattern used for data segment OOB
checks.

The remaining 4 table_grow failures were due to imported tables not being wired
into the compiled module. Fixed by storing `__wasm_funcs__` and `__wasm_types__`
arrays on exported `WebAssembly.Table` objects and extracting them during import
validation. The import min check now uses `__wasm_funcs__.length` (actual
current size after `table.grow`) instead of `__wasm_min__` (original declared
size). `createTables()` skips imported tables since their arrays are already
wired during import processing.

Results: table_get (4 → 0), table_set (8 → 0), table_grow (14 → 0).

**Previously affected tests:** table_get (4 → 0), table_set (8 → 0),
table_grow (14 → 0)

#### ~~4. Unlinkable / Uninstantiable Modules Not Rejected (was 2 failures — FIXED)~~

Modules that should be rejected at instantiation time are accepted by Hermes.
The spec requires validation between parsing and execution; Hermes skips some
of these checks, so errors surface later (or not at all) as wrong results.

**imports (0 failures):** Import type validation is now implemented using
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

All sub-issues have been resolved:

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
- **~~Table/memory exports not implemented (26):~~** Fixed. Memory exports are
  now implemented as `WebAssembly.Memory` objects (13 fixed) and table exports
  as `WebAssembly.Table` objects (12 fixed). All 25 table/memory import
  validation failures are resolved.
- **~~Alignment hint trusted for memory access (2):~~** Fixed. When
  `--test262` is active, `onLoad()` and `onStore()` now force `alignLog2 = 0`,
  routing all multi-byte operations through the byte-assembly (unaligned) path.
  This ensures correct results regardless of actual alignment, as the spec
  requires.
- **~~`memory.grow` on imported memory (4):~~** Fixed. When a module imports
  a memory, `createMemoryViews()` now uses the imported memory's actual
  `__wasm_min__` (initial page count) instead of the import declaration's
  minimum. Similarly, `onMemoryGrow()` uses the imported memory's actual
  `__wasm_max__` limit. This ensures the locally-created ArrayBuffer has
  the correct initial size and growth limit.

**Affected tests:** imports (2)

**Fix approach — dependency graph:**

The remaining failures have the following dependency structure:

```
Export globals as WebAssembly.Global ──→ Global type validation works (12 fixed) ✓ DONE

Export tables as WebAssembly.Table ────→ Table imports resolve (12 fixed) ✓ DONE
                                    └──→ Wire imported table into compiled code ✓ DONE
                                          (table_grow 4 → 0)

Export memories as WebAssembly.Memory ─→ Memory imports resolve (13 fixed) ✓ DONE
                                    └──→ Wire imported memory into compiled code
                                     └──→ memory.grow on imported memory works (4 fixed) ✓ DONE

Tags (independent) ───────────────────→ Tag import/export support (3+1 fixed) ✓ DONE

Alignment hints (independent) ────────→ Force unaligned path under --test262 (2 fixed) ✓ DONE
```

All import/export type validation and alignment issues are now resolved.
imports.wast passes with 0 failures.

**Previously affected tests:** imports (2 → 0)

#### 5. Module Load Failures / Missing Features (50 failures)

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

**~~memory_redundancy (3):~~** Previously showed 3 failures attributed to
module load issues. Now passes with 4/0/0 — the failures were actually
alignment-related, fixed by the unaligned byte-assembly path.

**Affected tests:** memory_grow (50)

#### ~~6. Multi-Value Return from Calls Not Implemented (was 6 failures — FIXED)~~

Fixed by replacing the thread-local `wasmI64HiStash_`/`wasmI64HiResult`
mechanism with a per-module return buffer (ArrayBuffer with Uint32Array +
Float64Array views). Functions returning i64 or multiple values now receive
buffer views as parameters and store results there. All 18 i64-returning
builtins take `retBufI` as their first argument. The `wasmI64HiStash` and
`wasmI64HiResult` builtins are deleted. Bytecode version bumped 116 → 117.

Multi-value returns are now fully supported: callees store all results into
the shared buffer, callers read them back, and export wrappers marshal
multi-value results to/from JS Arrays.

**Previously affected tests:** call (3 → 0), if (3 → 0)

#### 7. Unsupported Wasm Proposals (16 failures)

Test files that use syntax from newer Wasm proposals (GC types, typed function
references, extended constant expressions). The bundled `wast2json` (WABT
1.0.39, the latest release) cannot parse this syntax, so the module binary is
never produced. Even if wast2json were updated, Hermes's own Wasm binary
parser also does not support these proposals, so the tests would fail at
module load time instead. These are not actionable without implementing the
underlying proposals in both the toolchain and the engine.

Most (14) fail with 0 pass / 1 fail because the entire test file is rejected.
The `data` test (2 failures) partially works but wast2json rejects two modules
that use `global.get` on non-imported globals in data segment offset
expressions — valid in the current spec but not in the older spec version
wast2json implements.

**Affected tests:** br_if, br_table, local_tee, global, memory, table, elem,
select, align, unreached-valid, tag, ref_is_null, ref_null, linking, data (2)

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
| float_memory | 54 | 6 | 0 | NaN through memory (cat 1) |
| conversions | 610 | 8 | 0 | NaN reinterpret (cat 1) |
| br_if | 0 | 1 | 0 | Unsupported proposal (cat 7) |
| br_table | 0 | 1 | 0 | Unsupported proposal (cat 7) |
| unreached-valid | 0 | 1 | 0 | Unsupported proposal (cat 7) |
| local_tee | 0 | 1 | 0 | Unsupported proposal (cat 7) |
| global | 0 | 1 | 0 | Unsupported proposal (cat 7) |
| memory | 0 | 1 | 0 | Unsupported proposal (cat 7) |
| memory_grow | 0 | 50 | 0 | Module load failure (cat 5) |
| align | 0 | 1 | 0 | Unsupported proposal (cat 7) |
| data | 34 | 2 | 0 | Unsupported proposal (cat 7) |
| table | 0 | 1 | 0 | Unsupported proposal (cat 7) |
| elem | 0 | 1 | 0 | Unsupported proposal (cat 7) |
| select | 0 | 1 | 0 | Unsupported proposal (cat 7) |
| tag | 0 | 1 | 0 | Unsupported proposal (cat 7) |
| ref_is_null | 0 | 1 | 0 | Unsupported proposal (cat 7) |
| ref_null | 0 | 1 | 0 | Unsupported proposal (cat 7) |
| linking | 0 | 1 | 0 | Unsupported proposal (cat 7) |

### Priority for Fixing

1. ~~**Memory bounds checking** (cat 2) — FIXED. All 38 failures resolved:
   29 by bounds checks in `emitMemoryBoundsCheck()`, remaining 9 by forcing
   the unaligned byte-assembly path when `--test262` is active.~~
2. ~~**Table bounds checking** (cat 3) — FIXED. All 26 failures resolved:
   22 by bounds checks in `emitTableBoundsCheck()`, remaining 4 by wiring
   imported table arrays into the compiled module.~~
3. ~~**Instantiation-time validation** (cat 4) — FIXED. All import/export
   validation and alignment issues resolved. 0 failures.~~
4. **Module load failures** (cat 5) — multi-memory proposal not supported.
   50 failures, all cascading from one module.
5. **Unsupported proposals** (cat 7) — GC types, typed function references,
   extended constant expressions. Neither wast2json (WABT 1.0.39) nor Hermes
   supports these. 16 failures.
6. **NaN-boxing limitations** (cat 1) — requires non-NaN-boxed Wasm value
   representation. 54 failures.

## Known Architectural Limitations

These limitations are not yet surfaced as spec test failures because the
`linking.wast` test (which exercises cross-module scenarios) is blocked by a
Unsupported proposal (cat 7). They will become visible once cross-module tests
can run.

### Non-Function Imports Not Fully Wired

**Function imports**, **global imports**, and **table imports** are resolved
from the imports object and connected to the compiled module. Function imports
are validated for type compatibility using `__wasm_type__` strings. Global
imports read their value from the import object (either a
`WebAssembly.Global`'s `.value` property or a raw JS number). Table imports
from Wasm-exported tables share the exporter's internal arrays
(`__wasm_funcs__` and `__wasm_types__`) so that `table.grow`, `table.get`,
`table.set`, and `call_indirect` operate on the same storage. Tables imported
from JS-API `WebAssembly.Table` objects get fresh arrays (no sharing).

Other import kinds are stubbed:

- **Memory imports partially wired:** The compiled code creates a fresh
  `ArrayBuffer` sized to the imported memory's actual `__wasm_min__` (not
  the import declaration's lower bound). `memory.grow` also respects the
  imported memory's `__wasm_max__` limit. However, the actual buffer from
  the imported `WebAssembly.Memory` object is not used — two modules
  cannot share linear memory contents.

- **Table imports wired for Wasm-exported tables:** When a module imports a
  table that was exported by another Wasm module, the importing module uses the
  exporter's `__wasm_funcs__` and `__wasm_types__` arrays directly.
  `table.grow` in either module affects both (since `JSArray::setLengthProperty`
  grows arrays in-place). Tables imported from JS-API `WebAssembly.Table`
  objects (without `__wasm_funcs__`) get fresh empty arrays — the Table's
  internal storage is not shared.

### Export Objects — Partial Storage Sharing

The exports object includes function exports (with `__wasm_type__` metadata),
global exports (wrapped in `WebAssembly.Global`), tag exports (plain objects
with `__wasm_type__`), memory exports (wrapped in `WebAssembly.Memory`), and
table exports (wrapped in `WebAssembly.Table`). All export kinds are handled.

Exported `WebAssembly.Table` objects now carry `__wasm_funcs__` and
`__wasm_types__` properties pointing to the module's internal table arrays,
enabling cross-module table sharing via imports.

However, exported `WebAssembly.Memory` objects still have their own separate
storage — they do NOT share the module's internal linear memory. Import *type
validation* works (initial/maximum limit checks pass), but cross-module
memory sharing does not. Wiring imported memory buffers into the compiled
module is a separate change.

### Alignment Hints Trusted (without `--test262`)

Wasm load/store instructions include an alignment hint (e.g., `align=4` for
`i32.load`). The spec says this hint is advisory: implementations must
produce correct results regardless of whether the actual effective address
satisfies the declared alignment. The hint exists so that engines targeting
native code can emit faster aligned-load instructions when the hint
guarantees alignment.

When `--test262` is active (as in spec tests), `onLoad()` and `onStore()`
force `alignLog2 = 0`, routing all multi-byte operations through the
byte-assembly (unaligned) path. This ensures spec-correct results at
a significant performance cost: every multi-byte load/store reads/writes
individual bytes from `HEAPU8` and assembles them with shifts and ORs,
instead of a single typed array element access.

Without `--test262`, the compiled code trusts alignment hints. When
`alignLog2 == naturalAlign` (the common case, including all default-aligned
loads/stores), it uses typed array element access: `HEAP32[addr >>> 2]`,
`HEAPF64[addr >>> 3]`, etc. These typed array accesses implicitly round the
byte address down to the element boundary, silently reading/writing the
wrong bytes when the actual address is not aligned.

In practice, well-formed Wasm compilers (LLVM, Binaryen) emit correct
alignment hints, so production code is unaffected by this limitation.

### Cross-Module `call_indirect` Type Indices

The canonical type index map (commit d1907f2a3) is built per-module. If two
modules define the same function signature, they may assign different canonical
indices. When a function from module A is placed in module B's table (via shared
`WebAssembly.Table`), `call_indirect` in module B will compare module B's
canonical index against the index stored when module A populated the table — a
mismatch. Fixing this requires either a runtime-level cross-module type
registry or structural signature comparison at call time.
