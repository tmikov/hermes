# Hermes → Rust Port — Roadmap & Status

The single source of truth for *what* we are porting, *why*, *what is done*, and
*what is next*. Read this first when picking the effort back up. Component-level
specs/plans live under `doc/superpowers/specs/` and `doc/superpowers/plans/`.

## Goal & principles

Port the Hermes JavaScript compiler **front-end** to Rust, faithfully and incrementally.

- **Minimal `unsafe`.** Ideally none; where unavoidable, it must be *very well
  encapsulated* and never leak across module/crate boundaries. Each crate uses
  `unsafe_code = "forbid"` where possible (the `support` crate does).
- **Reuse juno by copying.** Useful code from `unsupported/juno/crates/` is **copied**
  into `rust/` and modified there, never referenced in place. (See `doc/JunoRustCrates.md`
  for the crate-by-crate analysis.)
- **Stay close to Hermes.** Keep the Rust structure close to the C++ where it makes
  sense and **copy the comments** (or keep them close), for traceability.
- **Faithful / byte-compatible where it matters.** Diagnostics are validated *byte-for-byte*
  against the real C++ `hermesc` binary.
- **Implement each component completely** (its whole public surface) in one pass — do not
  defer/stage features. The boundary is the *component*: don't pull in separate components.

## Repo layout & integration

- All Rust code lives under `rust/` — a Cargo workspace (`rust/Cargo.toml`) with member
  crates under `rust/crates/`. Toolchain pinned via `rust/rust-toolchain.toml` (1.96.0).
- Work lives on the **`rust`** branch and **stays there** — no merges, no PRs. The repo's
  main/base branch is **`static_h`** (not `main`).
- Build/test the workspace: `cargo test --manifest-path rust/Cargo.toml -p <crate>`.

## Component order & status

The front-end stratifies (see the dependency analysis below). We port bottom-up.

| Component | Crate / location | Status |
|-----------|------------------|--------|
| **SourceErrorManager** (+ buffer, locations, line index, diagnostics) | `rust/crates/support/` | ✅ **Complete** — entire public surface; **byte-for-byte validated vs `hermesc` 1.96.0** |
| **JS lexer** | (next) | ⏭ **Next** — see deps below |
| Parser | — | future |
| Sema (scope resolution + FlowChecker) | — | future |
| IR / IRGen | — | future |
| Optimizer | — | future |
| Inst / BCGen | — | future (BCGen couples to the VM — last) |

### Done: `support` crate (SourceErrorManager)

Modules under `rust/crates/support/src/`: `buffer` (copied `NullTerminatedBuf` + named
`SourceBuffer` with lazy line index), `location` (offset-based `SMLoc`/`SMRange`/`SourceId`/
`SourceCoords`), `line_index` (offset↔line/col), `diag` (`DiagKind`/`Subsystem`/`OutputOptions`/
`ResolvedDiagnostic`/`DiagHandler`/`CollectingHandler`/`Warning`), `render` (byte-compatible
`build_source_and_caret_line` + `render_diagnostic` + `StderrHandler`), `manager`
(`SourceErrorManager` façade). Tests: `tests/golden.rs` includes the live `hermesc`
differential. **Zero `unsafe`, zero warnings.** Spec: `specs/2026-06-01-source-error-manager-design.md`;
plan: `plans/2026-06-01-source-error-manager.md`.

### Next: JS lexer

Port `include/hermes/Parser/JSLexer.h` + `lib/Parser/JSLexer.cpp` (~3,700 LOC). What it
needs from `SourceErrorManager` is **done**. Its remaining *support-layer* prerequisites
(the first tasks of the lexer port — NOT part of SourceErrorManager):

| Dep | Hermes source | Status / note |
|-----|---------------|---------------|
| String interning (`StringTable`/`UniqueString`) | `Support/StringTable.h` | juno `atom_table` is the base, but needs a **byte/WTF-8 intern path** — JS string literals can hold lone surrogates encoded as ill-formed UTF-8 (`JSLexer.cpp` `appendUnicodeToStorage`), which a Rust `String` cannot hold. |
| Unicode char properties | `Platform/Unicode/CharacterProperties.h` | port generated tables or pin a crate to Hermes's Unicode version. |
| Number parsing | `Support/Conversions.h`, `FastStrToDouble`, `external/dtoa/` | bit-exact; FFI `dtoa` (small standalone C lib) or a vetted Rust crate. |
| Token tables | `Parser/TokenKinds.def`, `HTMLEntities.def` | mechanical `.def`→Rust. |
| Bump `Allocator` | `Support/Allocator.h` | **droppable** — Rust owns the decoded strings. |

## Key cross-cutting design decisions

- **Locations are offset-based with explicit buffer identity** (`SMLoc = (SourceId, u32)`),
  not raw pointers. A location knows its buffer, so the C++ pointer reverse-lookup vanishes.
  Chosen over a packed global 32-bit offset (clang-style) for simplicity; that's a later
  swap behind the same accessors if AST memory pressure demands it.
- **The lexer's scan cursor is the one place encapsulated `unsafe` is allowed** (decision
  "B"): a raw `*const u8` cursor *inside the lexer module only*, converted to an offset at
  every boundary so nothing unsafe escapes. The buffer is handed in as an `Rc<SourceBuffer>`
  (stable heap address + no borrow fight with the manager). The `support` crate itself is
  zero-`unsafe`.
- **Diagnostics are byte-compatible** with LLVH/`hermesc` (decision "A"): column = byte
  distance from line start; caret columns are *code points* with tab expansion (TabStop 8);
  the caret line is shown **only for all-ASCII source lines** (Hermes punts on non-ASCII
  widths); `adjustSourceLocation` backs the column off `\r`/UTF-8 continuation bytes.
  Rendering goes through a pluggable `DiagHandler` trait, validated against captured
  `hermesc` output.
- **C++ RAII guards → explicit methods.** `SaveAndSuppressMessages`/`SaveAndBufferMessages`/
  `CollectMessagesRAII` can't be literal guards in safe Rust (a `&mut`-holding guard can't
  coexist with emitting through the manager, and the crate forbids `unsafe`), so each is an
  explicit set-restore / enable-disable / begin-end API — the full feature, minus the sugar.
- **Translator vs. rendering.** A `CoordTranslator` affects *displayed* coordinates
  (`find_coords`, `dump_coords`) but the rendered diagnostic resolves its source line and
  caret column from the **untranslated** location, matching the C++ primary diagnostic.

## Dependency analysis (why the lexer is next)

The front-end is a clean DAG up to bytecode generation. `BCGen` is the chokepoint — it
reaches *into the VM* (`Runtime`, `VMLayouts`), so it comes last. Everything in Layers 0–3
(Support → AST/Parser/Sema → IR/IRGen → Optimizer) is VM-independent. The lexer sits at the
very bottom (depends only on the support layer), is self-contained, and is trivially
differential-testable (bytes in → tokens out), so it is the natural first real port after
the diagnostics foundation. Full analysis was done conversationally; `doc/JunoRustCrates.md`
covers what juno already provides.

## How to validate

- Tests: `cargo test --manifest-path rust/Cargo.toml -p support`.
- Diagnostic differential: build the reference binary once —
  `cmake -B cmake-build-asan -G Ninja -DCMAKE_BUILD_TYPE=Debug -DHERMES_ENABLE_ADDRESS_SANITIZER=ON -DCMAKE_CXX_FLAGS="-O1" -DCMAKE_C_FLAGS="-O1"`
  then `cmake --build cmake-build-asan --target hermesc` — and capture references with
  `(! cmake-build-asan/bin/hermesc -dump-ast FILE 2>&1)` (stderr is color-free when piped).
  `cmake-build-asan/` is git-ignored.
