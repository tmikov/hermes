# JSONParser → Rust — Design

Port of Hermes' `JSONParser` (`include/hermes/Parser/JSONParser.h` + `lib/Parser/JSONParser.cpp`,
plus the `Support/JSONEmitter.{h,cpp}` it relies on for `emitInto`) to Rust. This is the first
consumer of the completed Rust `JSLexer`, chosen as a small, low-dependency component to work out
the lexer-integration pattern before the full Parser, and to enable a Rust-vs-C++ benchmark.

> **Status:** design approved 2026-06-02. Base branch `static_h`, work on `rust`.
> **Reading order context:** `doc/superpowers/RustPortRoadmap.md` (roadmap), this spec, then the
> implementation plan(s) under `doc/superpowers/plans/`.

## Goal & scope

Faithfully port the entire public surface of the JSONParser component:

- The **value object model** — `JSONValue` and its variants (`Null`, `Boolean`, `String`,
  `Number`, `Object`, `Array`), with the C++ LLVM-style RTTI replaced by Rust enum dispatch.
- **`JSONFactory`** — owns everything; **uniques** strings & numbers and **shares hidden classes**
  (same-shape objects share one sorted-key descriptor).
- **`JSONParser`** — recursive-descent driver over `JSLexer`, reporting through `SourceErrorManager`.
- **`JSONValue::emitInto` + `JSONEmitter`** — serialization; `JSONEmitter` is ported in full (it is
  also the round-trip differential oracle).
- **`JSONSharedValue`** — allocator-backed value holder (used by the future SourceMap component).

Out of scope (genuinely separate components, per the "complete the component, don't pull in separate
components" rule): SourceMap, and any other `JSONEmitter`/`JSONParser` *callers*.

## Dependencies

Almost everything is already ported:

| Dependency | Source | Status |
|------------|--------|--------|
| `JSLexer` | `rust/crates/parser/src/lexer/` | ✅ done |
| `SourceErrorManager` (+ buffers, locations) | `rust/crates/support/` | ✅ done |
| String interner (`StringTable`/`UniqueString`) | `rust/crates/atom_table/` (`AtomTable`/`AtomBytes`) | ✅ done |
| Number parsing | `rust/crates/parser/src/number.rs` | ✅ done (via the lexer) |
| Bump allocator | `bumpalo` crate | **new** — first third-party dep |

`bumpalo` is the only new dependency. Its `unsafe` is encapsulated inside the vetted crate; we write
no hand `unsafe` on top of it except the single deref in §6 (JSONSharedValue).

## Crate layout

- **`support` crate** gains `json_emitter.rs` — faithful port of `Support/JSONEmitter.{h,cpp}`. Stays
  under the crate's `#![forbid(unsafe_code)]` (zero `unsafe`).
- **`parser` crate** gains a `json/` module mirroring `lib/Parser/JSONParser`:
  - `json/mod.rs` — the `JSONValue` model + accessors + `emit_into` + `JSONSharedValue`.
  - `json/factory.rs` — `JSONFactory`.
  - `json/parser.rs` — `JSONParser`.
  The `parser` crate already permits scoped `unsafe` (the lexer cursor); `bumpalo` is added here.
- A C++ **`json-parse-dump`** tool under `tools/json-parse-dump/`, registered via `add_hermes_tool`
  in `tools/CMakeLists.txt` (mirrors `tools/js-lexer-dump/`).
- A Rust **`json_parse_dump`** binary in the `parser` crate, mirroring the C++ tool.

## 1. Object model

`&'a JSONValue<'a>` **is** the C++ `JSONValue*`: nodes live in a `bumpalo::Bump` arena, the enum
variant replaces the kind tag + RTTI, and arena identity provides the pointer equality the C++ relies
on (and the unittests assert).

```rust
pub enum JSONValue<'a> {
    Null,
    Boolean(bool),
    Number(f64),
    String(AtomBytes),                                    // interned via the shared AtomTable
    Array(&'a [&'a JSONValue<'a>]),
    Object(&'a JSONHiddenClass, &'a [&'a JSONValue<'a>]), // class + values parallel to its keys
}

pub struct JSONHiddenClass {
    keys: Box<[AtomBytes]>, // sorted by string content; or &'a [AtomBytes] in the arena
}
```

Notes:
- The hidden class's `keys` are sorted by **string content** (matching C++ `sortProps`, which sorts by
  `->str()`); the object's `values` slice is parallel (value *i* belongs to key *i*).
- `JSONString`/`JSONNumber` are not separate node structs as in C++; their identity is the identity of
  the `&'a JSONValue` the factory uniques. `String` carries the interned `AtomBytes`; resolving back to
  bytes for `str()`/`c_str()`/emit goes through the shared `AtomTable`.

### Public surface (ported as methods / free fns)

- RTTI replacement: `kind()` plus `as_object()/as_array()/as_string()/as_number()/as_boolean()`
  returning `Option`, replacing `isa`/`dyn_cast`/`cast`. `JSONKindToString` → `kind_to_string`.
- Object: `size`, `get(name) -> Option<&JSONValue>`, `at(name)`, `count(name)`, `find(name)`,
  key/value iteration (yielding `(AtomBytes, &JSONValue)` — the "impression of key/value pairs" the
  C++ iterator gives), `get_hidden_class()`, index-by-position.
- Hidden class: `size`, `find(name) -> Option<usize>`, key iteration.
- Array: `size`, `at(pos)`, index, iteration.
- Scalars: `Boolean::get_value`, `Number::get_value`, `String::str()/c_str()` (resolved via the
  `AtomTable`).
- `emit_into(&self, emitter: &mut JSONEmitter)` — see §4.

### What lives where (memory ownership)

The arena holds only the parsed value tree; string bytes and uniquing bookkeeping live elsewhere —
mirroring the C++ split (`BumpPtrAllocator` ← nodes; `StringTable` ← bytes; `FoldingSet`/`std::map` ←
uniquing).

| Lives in… | Holds |
|-----------|-------|
| **`bumpalo` arena** (`&'a Bump`) | `JSONValue` nodes (32 B each: distinct numbers, distinct strings, each array, each object, the 3 singletons); the `&'a [&'a JSONValue]` child slices (N×8 B); `JSONHiddenClass` descriptors; the `&'a [AtomBytes]` class key arrays (N×4 B) |
| **`AtomTable`** (`&'a AtomTable`) | the actual string **bytes** (the arena's `String(AtomBytes)` holds only a `u32` handle) |
| **`JSONFactory` heap fields** | the uniquing `HashMap`s (`strings`/`numbers`/`classes`) — keys are handles, values are `&'a` refs into the arena; no payload |
| transient (heap/stack) | parser scratch `Vec` of pending elements/props (copied into an arena slice via `alloc_slice_copy`), lexer scratch, the input `Rc<SourceBuffer>` |

Child slices are built by collecting a temporary `Vec<&JSONValue>` during `parse_array`/`parse_object`,
then `arena.alloc_slice_copy(&vec)` (both `&'a JSONValue` and `AtomBytes` are `Copy`).

## 2. JSONFactory — append-only interner with interior mutability

```rust
pub struct JSONFactory<'a> {
    arena: &'a Bump,
    atoms: &'a AtomTable,
    strings: RefCell<HashMap<AtomBytes, &'a JSONValue<'a>>>,
    numbers: RefCell<HashMap<u64, &'a JSONValue<'a>>>,            // key = f64::to_bits()
    classes: RefCell<HashMap<Box<[AtomBytes]>, &'a JSONHiddenClass>>,
    null_v: &'a JSONValue<'a>,
    true_v: &'a JSONValue<'a>,
    false_v: &'a JSONValue<'a>,
}
```

- All accessors take `&self` and return `&'a JSONValue<'a>`. The returned reference lives in the
  **arena** (lifetime `'a`), independent of the transient `RefCell` borrow used to look up / insert the
  uniquing entry. This mirrors C++ "the factory is shared by reference; the allocator is append-only,"
  and is precisely what lets a caller interleave `factory.get_string(...)` with `parser.parse()`
  without `&mut` aliasing (the `SmokeTest2` unittest does exactly this).
- Uniquing fidelity:
  - strings keyed by `AtomBytes` (equivalent to C++ keying by `UniqueString*`),
  - numbers keyed by `f64::to_bits()` so `-0.0` and `0.0` are distinct (matches `NegativeNumbers`),
  - hidden classes keyed by their content-sorted `[AtomBytes]` vector (equivalent to C++ keying by the
    uniqued-`JSONString*` array; identical sharing semantics — `HiddenClassTest`).
- `null`/`true`/`false` are three singletons allocated once at construction; `get_null()/get_boolean(b)`
  return them (pointer equality holds via `ptr::eq`).
- Ports: `get_string(AtomBytes)`, `get_string(&str)` (interns then uniques), `get_number(f64)`,
  `get_boolean(bool)`, `get_null()`, `get_hidden_class(keys)`, `sort_props` (sort by content + detect
  duplicates → return the first duplicate), `new_object(props)`, `new_array(values)`. The
  `getAllocator`/`getStringTable` accessors map to the `&arena`/`&atoms` the factory holds.

## 3. JSONParser — lexer wiring

```rust
pub struct JSONParser<'a> {
    factory: &'a JSONFactory<'a>,
    lexer: JSLexer<'a>,            // owned, like C++
}
```

- Constructed from `(factory, buf_id, &'a mut SourceErrorManager, &'a AtomTable, convert_surrogates)`;
  the parser builds the `JSLexer` internally via `JSLexer::new_with_convert_surrogates(buf_id, sm,
  atoms, ctx, convert_surrogates)`. JSON is always `strict_mode = true` (already the Rust lexer's
  default, matching the C++ `strictMode=true` the JSONParser passes).
- Recursive descent ported branch-for-branch from `JSONParser.cpp`:
  - `parse()` → `advance`, `parse_value`, then check `error_count == 0`, return `Option`.
  - `parse_value()` → string / `minus`+numeric (with negation, and the "no numeric after minus" error) /
    numeric / `l_brace`→`parse_object` / `l_square`→`parse_array` / `rw_true`/`rw_false`/`rw_null` /
    default error.
  - `parse_array()`, `parse_object()` — including the comma/trailing handling, the
    `expected ']'`/`'}'`/`':'`/`a string` errors, and the duplicate-key check via `sort_props`.
- **Error routing / no `&mut` aliasing:** the lexer owns the single `&'a mut SourceErrorManager`. The
  parser reports through it (`self.lexer.source_mgr_mut().error(range, msg, Subsystem::Parser)`) and
  reads `get_error_count()` through the lexer (C++ `lexer_.getSourceMgr()`). There is exactly one
  `&mut sm` owner. (If the Rust lexer lacks a public `source_mgr_mut()`/error accessor, add a minimal
  one — it has the `sm` field already; C++ exposes `getSourceMgr()`.)
- `advance` uses the non-regexp grammar context (matches the C++ default; `/` never appears in valid
  JSON, so the context is immaterial for accepted input).

## 4. JSONEmitter (support crate)

Full faithful port of `Support/JSONEmitter.{h,cpp}` into `support::json_emitter`:

- The `State` stack (`Dict`/`Array`, `needsComma`/`needsKey`/`needsValue`/`isEmpty`),
  `open_dict/close_dict/open_array/close_array`, `emit_key`, `emit_value` overloads (bool, the integer
  widths, `f64`, `&str`, `&[u16]`), `emit_null_value`, `emit_key_value`, `emit_values`, `end_jsonl`,
  pretty mode (`prettyNewLine`/`indentMore`/`indentLess`), and `primitive_emit_string`.
- Output target: a `&mut impl std::fmt::Write` (or `&mut String`) in place of `llvh::raw_ostream`.
- **Byte-faithful** escaping (`primitiveEmitString`) and **`f64` formatting** — this is the round-trip
  oracle, so its bytes must match C++ exactly. The C++ double formatting in `JSONEmitter.cpp` is ported
  precisely (the plan pins the exact algorithm after reading that file).
- Rust enum overloading: instead of C++ overloads, use a small `EmitValue` trait or distinct methods
  (`emit_f64`, `emit_str`, `emit_bool`, …); `emit_into` calls the ones it needs.
- Unbalanced-structure asserts → `debug_assert!`.

`JSONValue::emit_into` mirrors `emitInto`: object → `open_dict` + per-key `emit_key` + recurse +
`close_dict`; array → `open_array` + recurse + `close_array`; scalars → the matching `emit_value`.

## 5. JSONSharedValue — the one hand-written `unsafe`

C++ pairs a `const JSONValue*` with a `shared_ptr<const Allocator>`. The faithful Rust equivalent owns
an `Rc<Bump>` plus the root reference, which is **self-referential**. **Decision (approved):** model it
with one small, encapsulated `unsafe` deref, scoped like the lexer's cursor:

- `JSONSharedValue { value: *const JSONValue<'static>, allocator: Rc<Bump> }` (lifetime-erased pointer),
  with `deref(&self) -> &JSONValue<'_>` doing the `unsafe` deref.
- **Invariant:** the `Rc<Bump>` keeps the arena alive for as long as the `JSONSharedValue` exists, so the
  stored pointer is always valid; the returned reference is tied to `&self`.
- This requires an owned-arena construction path on the factory (an `Rc<Bump>`-backed mode) in addition
  to the borrowed `&'a Bump` mode used everywhere else.
- Rejected alternative: the `ouroboros` crate (keeps it hand-`unsafe`-free at the cost of a proc-macro
  dependency and generated self-referential glue). Revisit only if the single `unsafe` proves awkward.

This is the **only** hand-written `unsafe` in the component; everything else is safe over `bumpalo`.

## 6. Validation

Two-pronged, mirroring the lexer:

1. **Differential round-trip oracle.** C++ `json-parse-dump` parses input and, on success, emits
   canonical JSON via `JSONEmitter` to stdout; on failure it prints the error count (and diagnostics).
   The Rust `json_parse_dump` emits identically. `rust/crates/parser/tests/json_differential.rs` asserts
   **byte-for-byte** equality over a JSON corpus that includes:
   - valid documents (objects, arrays, nesting, numbers incl. `-0`, strings incl. escapes/WTF-8,
     booleans, null, shared-shape objects exercising hidden-class sharing),
   - error cases (asserting equal error counts / failure).
   Resolution reuses the lexer's pattern: locate the C++ binary via `CARGO_MANIFEST_DIR`, assert every
   entry when present, and honor `REQUIRE_DIFFERENTIAL=1` to hard-fail if the binary is absent.
2. **Ported unittests.** Port the 5 `unittests/Parser/JSONParserTest.cpp` cases (`SmokeTest1`,
   `SmokeTest2`, `NegativeNumbers`, `HiddenClassTest`, `EmitTest`) and `unittests/Support/
   JSONEmitterTest.cpp` to Rust, faithfully (concrete values, error counts, emit bytes, uniquing and
   hidden-class identity assertions).

## 7. Benchmark

`--bench=N` mode on both `json-parse-dump` (C++) and `json_parse_dump` (Rust):

- Read the file once into memory, then parse it `N` times, using a **fresh factory/arena per
  iteration** (so each parse is independent, matching "read the file multiple times"), and suppress the
  emit step.
- Print elapsed wall-clock and derived MB/s.
- Run both tools on the same large file for an apples-to-apples comparison.
- A committed small **generator** (script or a tiny Rust/Python helper) produces a multi-MB `big.json`
  deterministically; the large blob itself is not committed.

## Faithfulness notes / deliberate deviations

- **RTTI → enum.** LLVM `isa`/`dyn_cast`/`cast` become enum matching / `as_*()` accessors. Pointer
  equality semantics are preserved via arena identity + uniquing.
- **Pointers → arena refs.** `JSONValue*` → `&'a JSONValue<'a>` in a `bumpalo` arena. Hidden-class
  flexible-array-member → an arena slice.
- **Uniform 32-byte enum nodes + separate value slices (deliberate).** A Rust enum is sized to its
  largest variant (the `Object` variant: thin ptr + slice fat ptr) + tag = **32 bytes per node**,
  regardless of variant. And object/array values live in a *separate* arena slice (`&'a [..]`) rather
  than inline as in C++'s `Pack`/flexible-array layout — i.e. two allocations + one extra indirection
  per object/array. We **deliberately accept** this over the more faithful inline-DST layout (which
  would need encapsulated `unsafe` in `new_object`/`new_array` via `alloc_layout` + manual construction)
  for simplicity. Impact is small: scalar nodes are uniqued/singletons so the fat-node overhead does not
  multiply, total bytes are ~equal to C++ (the N value pointers exist either way), and the extra cost is
  allocation count + one pointer-chase, which is minor for a bump-allocated parse benchmark. Revisit
  only if the benchmark shows this layer matters.
- **`FoldingSet` → `HashMap`.** Uniquing uses `HashMap` (strings/numbers/classes) instead of LLVM's
  `FoldingSet`; semantics (dedup by content) identical.
- **`shared_ptr<Allocator>` → `Rc<Bump>`** with one encapsulated `unsafe` (§6).
- **`getAllocator` has no separate analog** beyond the `&arena` the factory holds (consistent with the
  lexer's documented surface gap).

## Validation commands (to be finalized in the plan)

```bash
cargo test  --manifest-path rust/Cargo.toml -p parser           # model + parser + ported unittests
cargo test  --manifest-path rust/Cargo.toml -p support          # JSONEmitter port + its ported test
cmake --build cmake-build-asan --target json-parse-dump         # the C++ oracle
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test json_differential -- --nocapture
# benchmark:
cmake-build-asan/bin/json-parse-dump --bench=100 big.json
cargo run --manifest-path rust/Cargo.toml -p parser --release --bin json_parse_dump -- --bench=100 big.json
```
