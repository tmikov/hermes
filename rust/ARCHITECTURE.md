# Architecture

This document describes the structure, design decisions, and validation
methodology of the Hermes Rust front-end port. For project history, component
status, and what comes next, read
[`doc/superpowers/RustPortRoadmap.md`](../doc/superpowers/RustPortRoadmap.md)
(in the repo root) — this file distills the architecture; the roadmap is the
authoritative log.

---

## Crate map

The Rust workspace lives under `rust/` with seven published library crates and
two internal-only crates:

```
rust/
  Cargo.toml            workspace root
  crates/
    support/            SourceErrorManager, diagnostics, JSON emitter, WTF-8 codec
    atom_table/         string interner (copied from juno)
    unicode/            Unicode character-property tables (generated from UnicodeData.inc)
    ast/                GC-arena AST: 271 ESTree nodes + JSON dumper
    parser/             lexer + JSON parser + JS parser
    command_line/       LLVM-cl-style CLI flag parser
    sema/               semantic validation and scope resolution
    tools/              CLI drivers (ast-dump, json-parse-dump, gen-json,
                        preparse-dump, sema-dump) — publish = false
    comparison/         benchmark harness — excluded from workspace, publish = false
```

**Package names.** The directory names above are the short in-tree names used
throughout this document; the Cargo package names (and therefore the `use`
paths) are the `hermes-*` family:

| Directory | Package | Import path | Stability |
|---|---|---|---|
| `support/` | `hermes-support` | `hermes_support` | support crate |
| `atom_table/` | `hermes-atom-table` | `hermes_atom_table` | support crate |
| `unicode/` | `hermes-unicode` | `hermes_unicode` | support crate |
| `ast/` | `hermes-ast` | `hermes_ast` (also `hermes_parser::ast`) | stable public surface |
| `parser/` | `hermes-parser` | `hermes_parser` | stable public surface |
| `sema/` | `hermes-sema` | `hermes_sema` | stable core, advanced modules may change |
| `command_line/` | `hermes-command-line` | `hermes_command_line` | support crate |

Cargo commands therefore take `-p hermes-parser`, not `-p parser` (likewise
`-p hermes-sema`). The two internal crates (`tools`, `comparison`) keep their
bare package names and `publish = false`.

`hermes-sema`'s stable core is the `resolve` façade, the two entry points in
its `resolve` module, and the result model (`sem_context`, `ids`). Its other
seven public modules (`resolver`, `decl_collector`, `ast_eval`, `dump`,
`dump_context`, `libhermes`, `keywords`) are public because `tools`' `sema-dump`
bin and the crate's integration tests drive them directly; the port is also
still missing the `$SHBuiltin` module protocol and the lazy/`eval` entry
points, so those modules are labelled advanced / port-internal in their own
module docs and may change or be made private in a 0.x bump.

### `support`

Contains the `SourceErrorManager` façade (buffer, locations, line index,
diagnostics), the `JSONEmitter` (ESTree JSON output), and a WTF-8 ↔ UTF-16
codec (`hermes_support::utf8`). Zero `unsafe` — forbids `unsafe` via
`[lints.rust] unsafe_code = "forbid"` in its Cargo.toml.

### `atom_table`

A string interner, copied verbatim from the juno project and extended with a
WTF-8 byte-intern path (`AtomBytes`/`atom_bytes`). Encapsulated `unsafe` lives
here; it does not leak across crate boundaries.

### `unicode`

Unicode character-property predicates and range tables, generated from
`UnicodeData.inc` (Unicode 17.0.0) by the committed `gen_tables.py` script.
Zero `unsafe` — forbids `unsafe` via `[lints.rust] unsafe_code = "forbid"` in
its Cargo.toml.

### `ast`

The ESTree-compatible AST:
- **GC arena** — copied and adapted from juno (`Context`/`GCLock`/`NodeRc` +
  mark-sweep). `unsafe` confined to `context.rs`.
- **271-node set** — generated from `include/hermes/AST/ESTree.def` by the
  committed `gen_nodes.py` script into `// @generated src/node.rs`.
- **Immutable children + `Cell` attributes** — structural child fields are
  `&'gc Node`/`Option<&'gc Node>`/`NodeList`, rebuilt on change via a
  functional walk. Non-structural attributes are `Cell<…>`, mutated in place.
- **Transforming visitor** — `VisitorMut` / `TransformResult` / `Path` /
  `NodeField` + `visit_children_mut` (functional rebuild); generated `builder`
  module for clone-with-one-field-changed.
- **`ESTreeJSONDumper`** — generates ESTree JSON byte-for-byte matching
  `hermesc -dump-ast -dump-source-location=both`.

### `parser`

The lexer, JSON parser, and JS parser, consuming `support` + `atom_table` +
`unicode` + `ast`:
- Lexer (`src/lexer/`) — faithful port of `JSLexer.h`/`.cpp` (~3,700 LOC C++).
- JSON parser (`src/json/`) — first `JSLexer` consumer; mirrors `JSONParser`.
- JS parser (`src/js/`) — port of `JSParserImpl-*.cpp` (~16,900 LOC C++),
  sliced into `mod`, `expressions`, `statements`, `functions`, `classes`,
  `modules`, and `flow/{mod, declarations, types, function_types, object_types,
  params, match_}`.

### `sema`

Semantic analysis, consuming only `ast` + `support` + `atom_table` — the same
layering C++ `lib/Sema` has:
- `decl_collector.rs` — port of `DeclCollector.{h,cpp}`: per-scope declaration
  grouping.
- `resolver/` — port of `SemanticResolver.{h,cpp}` (scope tree, bindings,
  identifier resolution, validation diagnostics, the named AST rewrites),
  sliced into `mod`, `declarations`, `identifiers`, `statements`,
  `expressions`, `functions`, `classes`, `calls`, `modules`, `promoter`
  (`ScopedFunctionPromoter`) and `unresolver`.
- `sem_context.rs` — port of `SemContext.h`'s `Decl` / `LexicalScope` /
  `FunctionInfo` records, id-indexed instead of pointer-linked.
- `resolve.rs` — the two entry points, `resolve_ast` (`compile = true`) and
  `resolve_ast_for_parser` (`compile = false`).
- `dump.rs` / `dump_context.rs` — the `semDump` / `SemContextDumper` ports the
  differential compares byte-for-byte.

---

## The GC-arena AST (juno lineage)

The AST model is copied and adapted from juno's
`unsupported/juno/crates/juno_ast/`. The key decisions:

**References, not index handles.** In a never-freed bump arena both are equally
safe; references are more logically robust and read close to C++ `node->field`.

**Immutable children, mutable attributes.** Child fields (structural edges in
`ESTree.def`) are `&'gc`/`Option`/`NodeList` and are rebuilt by a functional
walk when a transform changes them. Attribute fields (value fields, decorations)
are `Cell<…>` and mutated in place. The split falls directly out of the `.def`
type tags: no cross-edges between cells in different nodes, so the GC marker
traces decoration lists explicitly without the invariance problem juno hit.

**`#[repr(C)]` structs.** Metadata-first layout; the `Node<'gc>` enum arms
match the struct types so a deep `match` compiles to a single dispatch.

**Node generation.** `gen_nodes.py` parses `ESTree.def` with all dialect flags
on (Flow/JSX/TS/Cover) and emits the `// @generated src/node.rs`. Per node: the
struct, the enum arm, `NodeKind` entry (mirrors the C++ enum, `#[repr(u32)]`,
`.def` order, interleaved `_First`/`_Last` sentinels), range predicates, leaf
accessors, `new` constructor, and `visit_children`/`mark_lists` arms.
An idempotency test (`tests/generated_idempotent.rs`, forced by `REQUIRE_GEN=1`)
guards against drift between the generator and the committed output.

---

## Faithful-port conventions

These are the binding rules for how C++ constructs map to Rust. They are not
style preferences — deviating from them breaks the structural correspondence
with the C++ source and is unauthorized unless explicitly approved.

### C++ templates → Rust generics

Every C++ template specialization becomes a Rust generic. Specific examples:

- `template<bool RequireNoNewLine>` on `JSLexer::lookahead1/2` →
  `fn lookahead1<const REQUIRE_NO_NEWLINE: bool>`.
- `template<IdentifierMode>` on `scanIdentifierFastPath/Parts` → the `IdMode`
  marker trait with `JsMode`/`JsxMode`/`FlowMode` zero-sized-type implementors.
- `template<bool JSX>` on `scanString` → `fn scan_string<const JSX: bool>`.

A runtime `bool` or enum parameter is behaviorally equivalent and the
differential test cannot detect the difference, but it is a structural deviation
and is not acceptable. Any `template <` in the C++ source that was ported must
remain a generic in Rust.

### C++ RAII guards → explicit guard types

Rust's borrow checker prevents the C++ pattern of a `&mut`-holding RAII guard
coexisting with method calls through the same `&mut self`. The mapping:

- `SaveAndRestore<bool>` on `paramYield_`/`paramAwait_` → `Rc<Cell<bool>>` +
  a `ParamFlagGuard` that implements `Drop` (restores on every `?` path).
- `SaveFunctionState` strict-mode restore → explicit save/restore wrappers.
- `SaveAndSuppressMessages`/`CollectMessagesRAII` → explicit begin/end APIs on
  `SourceErrorManager`.
- `SavePoint` (lexer) → a plain value struct + an explicit `restore` call.

The full feature is always implemented, just without the syntactic sugar.

### The `*const u8` cursor decision

The lexer scan cursor (`rust/crates/parser/src/cursor.rs`) uses a raw
`*const u8`, confined to that module and converted to an offset at every
boundary. This is the **only** `unsafe` in the `parser` crate. It matches
decision "B" documented in the roadmap: the lexer's speed depends on in-register
pointer arithmetic, the `NullTerminatedBuf` trailing NUL makes one-byte
lookahead unconditionally in-bounds, and encapsulation keeps the unsafety
reviewable.

### C++ default arguments are spec

Hermes uses default arguments heavily. Every ported call site must look up the
default in the C++ header — assuming any default is a real bug source. Examples
that have caused bugs in this port:

- `JSLexer::lookahead1(None)` defaults to `RequireNoNewLine = true`
  (JSLexer.h:658).
- `checkAndEat` defaults to `AllowRegExp` grammar context.
- `parseTypeArgsFlow` defaults to the `Type` grammar context (which splits `>>`).

### Diagnostics byte-compatible with `hermesc`

Column = byte distance from line start; caret columns are code points with
tab expansion (TabStop 8); the caret line is shown only for all-ASCII source
lines. Rendering goes through a pluggable `DiagHandler` trait, validated against
captured `hermesc` output in `tests/golden.rs`.

---

## Differential testing methodology

Differential testing is the primary conformance gate for every component. No
component is declared complete until its differential test passes byte-for-byte.

### The oracle

For the JS parser, the oracle is `hermesc -dump-ast -dump-source-location=both`.
This flag produces the raw parse AST before semantic analysis
(`CompilerDriver.cpp:867`), so no separate C++ tool is needed. The Rust
`ast-dump` binary produces output in the identical format.

For the lexer, the oracle is a small C++ tool (`tools/js-lexer-dump/`) that
links the real `JSLexer` and emits tokens in a deterministic text format.

For the JSON parser, the oracle is a `tools/json-parse-dump/` C++ tool.

### The corpora

Parser corpora live under `rust/crates/parser/tests/`:

| Corpus directory | Flag passed to both binaries |
|---|---|
| `parser_corpus/` | (none — plain JS) |
| `parser_corpus_flow/` | `-parse-flow` |
| `parser_corpus_flow_component/` | `-parse-flow -Xparse-component-syntax` |
| `parser_corpus_flow_records/` | `-parse-flow -Xparse-flow-records` |
| `parser_corpus_flow_match/` | `-parse-flow -Xparse-flow-match` |
| `parser_corpus_ts/` | `-parse-ts` |
| `parser_corpus_jsx/` | `-parse-jsx` |
| `parser_corpus_jsx_flow/` | `-parse-jsx -parse-flow` |

The plain JS corpus has 77 files; the Flow corpus 42; component 8; records 5;
match 7; TS 20; JSX 6; JSX/Flow 1 — 166 files total. Every file is parsed by
both binaries and the outputs are compared byte-for-byte.

### The gate command

```bash
# Build the Rust ast-dump binary and the C++ hermesc oracle:
cargo build --manifest-path rust/Cargo.toml -p tools --bin ast-dump
cmake --build cmake-build-asan --target hermesc

# Run the differential gate (fails if the oracle binary is absent):
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml \
    -p hermes-parser --test parser_differential
```

This gate must pass before any parser change is merged.

### Idempotency guard

The AST node generator has its own gate:

```bash
REQUIRE_GEN=1 cargo test --manifest-path rust/Cargo.toml \
    -p hermes-ast --test generated_idempotent
```

This confirms the committed `src/node.rs` is byte-for-byte identical to what
`gen_nodes.py` would produce from the current `ESTree.def`.

---

## Dependency graph

```
hermesc (C++ oracle) ← differential tests only
         ↑
    parser ──→ ast ──→ support ──→ unicode → (std)
         │         └──→ atom_table
         └──→ unicode
         └──→ atom_table

    sema ──→ ast, support, atom_table

    tools ──→ parser, sema, ast, support, atom_table
         └──→ command_line (CLI flags, no logic)
```

`unicode` has no intra-workspace dependencies. The `support` crate depends on
`unicode`. The `ast` crate depends on `support` and `atom_table`. The `parser`
crate depends on all of the above — and on nothing else: the CLI drivers and
their `command_line` dependency live in the unpublished `tools` crate, so the
published library is a pure library. The `sema` crate sits beside `parser`
rather than above it, consuming only the AST — the same layering C++
`lib/Sema` has; it takes `parser` as a dev-dependency only, because its tests
build trees by parsing source. The `hermes-command-line` crate is a thin
CLI argument helper; it is published because the drivers are built on it, but
nothing in `hermes-parser`'s dependency closure needs it.

---

## Toolchain

Rust 1.96.0, pinned via `rust/rust-toolchain.toml`. Build with:

```bash
cargo build --manifest-path rust/Cargo.toml
cargo test  --manifest-path rust/Cargo.toml
```

Do not `cd` into `rust/` — pass `--manifest-path` instead.
