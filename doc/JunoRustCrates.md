# Juno Rust Crates Analysis

Analysis of the Rust crates in `unsupported/juno/crates/`, evaluating their
potential for general-purpose reuse outside the Juno JavaScript compiler.

## Crate Summary

| Crate | General-Purpose? | Description |
|-------|-----------------|-------------|
| `command_line` | Yes | Zero-dependency CLI argument parser |
| `juno_support` | Partially | Utility modules, some general, some JS-specific |
| `juno_ir` | No | IR data structures (pool, use-def chains) |
| `lit` | No | Build helper for LLVM lit/FileCheck binaries |
| `juno` | No | JS parser frontend and semantic analysis |
| `juno_ast` | No | JS AST definitions |
| `juno_cli` | No | CLI binary for Juno |
| `juno_eval` | No | JS evaluator |
| `juno_pass` | No | JS optimization passes |
| `flow_native` | No | Flow Native compiler binary |
| `hermes` | No | FFI bindings to Hermes C++ parser |
| `libcplusplus` | No | C++ stdlib linking helper |

---

## `command_line` — Zero-Dependency CLI Argument Parser

**Interesting for general-purpose use: Yes.**

### Description

A zero-dependency command-line argument parsing library inspired by LLVM's `cl`
(command line) library. Options are registered imperatively with a `CommandLine`
instance rather than declaratively via derive macros.

**Supported features:**
- Long options (`--max-heap=10`) and short options (`-m 10`, `-m10`, combined
  `-abc`)
- Positional arguments (consumed in order)
- Boolean flags (`--strict`, `--pretty=on`)
- Enum options — both mutually-exclusive standalone flags and values on a single
  option
- Multi-valued list options
- Optional values (`Opt<Option<U>>`)
- `--` separator to end option processing
- Automatic `--help` / `--help-hidden` with category grouping
- Min/max occurrence count validation
- Custom parsers via `with_parser()`

**API pattern:**
1. Create `CommandLine` with a program description.
2. Define options using `Opt<T>::new(...)`, `new_flag()`, `new_bool()`,
   `new_enum()`, etc.
3. Call `CommandLine::parse()` or `parse_env_args()`.
4. Read values by dereferencing `Opt<T>` (implements `Deref<Target=T>`).

### Unsafe Usage

3 `unsafe` blocks, all in `opt.rs`, within the `OptValue<T>` struct:

- Two for mutating through `UnsafeCell<Vec<T>>` during the parse phase.
- One `get_unchecked_mut(0)` that avoids a bounds check (guarded by a prior
  `is_empty()` check).

The `UnsafeCell` implements a two-phase protocol: mutable during parsing,
immutable after `finish()`. The type is `!Send + !Sync` (uses `Rc`), so this is
sound. The `UnsafeCell` could be replaced with `RefCell` with negligible cost —
a commented-out `RefCell` line suggests this was a deliberate micro-optimization.

### Code Quality

**Strengths:**
- Zero external dependencies.
- Clean separation: `cl.rs` (registration/help), `opt.rs` (option types),
  `parser.rs` (parsing logic).
- Decent test coverage for core features (long/short options, positional args,
  enums, help output).

**Weaknesses:**
- `parse_env_args()` calls `exit(0)` on error instead of `exit(1)` — a bug.
- A `cond!` macro reimplements `if-else` for no clear benefit.
- `pub fn values(&self) -> &Vec<T>` should return `&[T]` per Rust API
  guidelines.
- No subcommand support, shell completions, or env var integration.
- Missing tests for: boolean parsing, optional values, list options, shared
  `opt_value`, min_count validation.
- Edition 2018 (outdated).

**Comparison to `clap`:** Occupies a niche as a minimal, zero-dep parser where
binary size or compile time matters. The LLVM-style imperative API is less
idiomatic than derive-based Rust approaches but will feel familiar to C++
developers.

---

## `juno_support` — Utility Library (Mixed General/Specific Modules)

Contains 13 modules of varying general-purpose usefulness. Below, modules are
grouped by reuse potential.

### General-Purpose Modules

#### `atom_table` — String Interning Table

A string interning/uniquing table. Stores each unique string once and returns a
lightweight `Atom` (u32 index) handle. Supports both UTF-8 (`Atom`) and UTF-16
(`AtomU16`) strings. Includes a thread-local debug context so `Debug` formatting
of `Atom` values can resolve to their string content.

**Unsafe:** ~8 blocks. Uses `UnsafeCell` for interior mutability and raw pointer
casts to `&'static str` for HashMap keys (sound because strings are append-only
and heap allocations are stable). Could be made safer with `RefCell`.

**Tests:** One basic test. No coverage for `AtomU16`, debug context, or edge
cases.

#### `scoped_hashmap` — HashMap with Lexical Scoping

A HashMap where pushing/popping scopes shadows/unshadows bindings. O(1) lookup
(always returns innermost binding), O(n) scope pop. Each key maps to a linked
list of nodes from different scopes. Classic compiler symbol table data
structure, but useful for any nested-scope scenario.

**Unsafe:** ~10 blocks. Built entirely on raw pointers (`Box::into_raw` /
`Box::from_raw`), essentially a C-style linked list. The `Drop` impl correctly
deallocates all nodes. Defensible for performance but could potentially be
rewritten with safer abstractions.

**Tests:** Two tests covering basic insert/lookup and nested scope push/pop with
shadowing.

#### `opaque_id` — Type-Safe ID Macro

A `declare_opaque_id!` macro generating a newtype around `NonZeroU32`. Provides
type-safe opaque identifiers with niche optimization (`Option<Id>` is same size
as `Id`). Index is offset by 1 so index 0 maps to `NonZeroU32(1)`.

**Unsafe:** 1 block (`NonZeroU32::new_unchecked`, guarded by `debug_assert`).

**Tests:** None.

#### `str_enum` — Enum-to-String Macro

A `define_str_enum!` macro that generates an enum with bidirectional `&str`
conversion (`FromStr`, `TryFrom<&str>`, `as_str()`). Clean design; error type
is caller-specified.

**Unsafe:** None.

**Tests:** None directly.

#### `timer` — Benchmarking Timer

Records named duration marks. Supports terse (debug map) and pretty-printed
(aligned table with auto-scaled s/ms/us/ns units) display. Uses the `{:#}` vs
`{}` alternate flag idiom.

**Unsafe:** None.

**Tests:** None.

#### `nullbuf` — Null-Terminated Buffer

A `Vec<u8>` wrapper guaranteeing null termination. Useful for C FFI interop.
Can be created from readers, files, slices, or strings. Handles
already-null-terminated inputs.

**Unsafe:** Two methods marked `unsafe` on the signature (`as_ptr`,
`as_c_char_ptr`) as a usage hint, but the implementations contain no unsafe
operations. Unconventional.

**Tests:** None directly (tested via `source_manager`).

#### `deque` — Stable-Address Append-Only Deque

Implemented as `Vec<Vec<T>>` with doubling chunk sizes. Elements never move
after insertion, so raw pointers to elements remain valid. Useful as a simple
arena.

**Unsafe:** None in implementation (1 in tests).

**Tests:** Two tests.

**Bug:** `is_empty()` always returns `false` because `new()` pre-allocates a
chunk.

#### `heap_size` — Heap Memory Estimation Trait

A `HeapSize` trait with implementations for `String`, `Box<T>`, `Vec<T>`, and
`HashMap<K, V>`. All estimates are shallow (do not recurse into element heap
allocations). Undocumented limitation.

**Unsafe:** None.

**Tests:** None.

#### `case` — Snake-to-Camel Case Conversion

A single function `ascii_snake_to_camel`. ASCII only (debug-asserts on
non-ASCII). Very small (29 lines).

**Unsafe:** None.

**Tests:** None.

#### `fetchurl` — URL Data Fetching

Fetches data from `file:` and `data:` (base64) URLs. Has `FetchFlags` to
restrict access. No HTTP support despite an `allow_network` flag.

**Unsafe:** None.

**Tests:** Good coverage for data URLs and error conditions.

### Somewhat General but with JS-Specific Ties

#### `source_manager` — Source File and Diagnostic Manager

Manages source files (name + buffer) and provides error/warning/note reporting
with file:line:col locations. The concept is general to any compiler/parser, but
the implementation outputs directly to stderr with no way to capture or redirect
diagnostics.

**Unsafe:** ~6 blocks (UnsafeCell for interior mutability of counters; could use
`Cell<usize>` instead).

**Bug:** `inner()` creates an unnecessary `&mut` reference from `&self`.

#### `json` — Streaming JSON Emitter

A streaming JSON writer with pretty-printing. Uses a state machine for call
order validation.

**Bug:** `emit_string` / `emit_key` do NOT escape quotes, backslashes, or
control characters — produces invalid JSON for inputs containing these.
`emit_string_literal` handles escaping but only for `&[u16]` (UTF-16) input.

Number formatting uses C++ FFI (ES5.1 `numberToString`), tying it to the Hermes
build.

### Juno-Specific Modules

#### `convert` — ES5.1 Number-to-String

Wraps a C FFI function for JavaScript number-to-string conversion. Depends on
`libcplusplus` and a C++ implementation linked via cmake. Not reusable outside a
JS engine context.

### Overall Code Quality

- **Naming:** Consistent, idiomatic Rust (`PascalCase` types, `snake_case`
  functions).
- **Documentation:** Moderate. Most public APIs have doc comments; some modules
  lack module-level docs.
- **Error handling:** Mixed. `fetchurl` uses proper `Result` + `thiserror`.
  Others panic on misuse or use deferred errors.
- **Idiomatic Rust:** Written by experienced programmers but with a C++ mindset
  in places — heavy `UnsafeCell` where `RefCell`/`Cell` would suffice, raw
  pointer linked lists where safer alternatives exist.
- **Test coverage:** Sparse. 7 of 13 modules have tests, mostly basic smoke
  tests. 6 modules have zero tests.
- **Edition:** 2018 (outdated; current is 2021).

---

## `juno_ir` — IR Data Structures

**Interesting for general-purpose use: No.**

Contains three modules (`pool`, `uref`, `value_list`), all `pub(crate)` (not
publicly exported). Implements an SSA-style use-def chain modeled on LLVM's
Value/Use/User pattern. While `Pool<T>` (chunk-based arena with free list) and
`URef<T>` (Copy-able non-owning pointer) are conceptually general, they are
low-level unsafe primitives (~17 unsafe blocks total) that exist specifically to
support the use-def chain. Better-maintained alternatives exist (`typed-arena`,
`bumpalo`, `slotmap`). The crate is clearly a prototype — all modules are
`#[allow(dead_code)]`, nothing is `pub`, and commented-out test code remains.
`Pool` has no `Drop` impl (leaks initialized values by design).

---

## `lit` — LLVM Lit/FileCheck Build Helper

**Interesting for general-purpose use: No.**

A 17-line crate that invokes CMake to build LLVM's `lit` and `FileCheck` from
the Hermes source tree, then exposes their paths. Hardcoded to the Hermes
directory layout via relative paths. No configurable API.

---

## Not Analyzed (Clearly Juno-Specific)

- **`juno`** — JS parser frontend, source map support, semantic analysis.
- **`juno_ast`** — JavaScript AST node definitions, visitor, validation.
- **`juno_cli`** — Juno CLI binary.
- **`juno_eval`** — JavaScript evaluator/interpreter.
- **`juno_pass`** — JS optimization pass manager and passes.
- **`flow_native`** — Flow Native compiler binary.
- **`hermes`** — FFI bindings to the Hermes C++ parser.
- **`libcplusplus`** — Helper crate to link against the C++ standard library.
