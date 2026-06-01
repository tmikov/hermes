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
| **JS lexer** | `rust/crates/{atom_table,unicode,parser}/` (planned) | 🚧 **In progress** — design spec done; see deps below |
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
needs from `SourceErrorManager` is **done**. Full design: `specs/2026-06-01-js-lexer-design.md`.
Per-subsystem implementation plans land under `plans/` just-in-time as each is built.

**Locked decisions (this design pass):**
- **Scan cursor:** raw `*const u8` (option "B"), confined to the cursor module, offset
  at every boundary; `Rc<SourceBuffer>` backing, `NullTerminatedBuf` NUL makes lookahead
  in-bounds.
- **String interner:** copy juno `atom_table` **verbatim** (keep its encapsulated unsafe)
  and add a byte/WTF-8 intern path.
- **Number parsing:** **pure Rust, no FFI.** The lexer's decimal path uses `fast_float`
  (NOT `dtoa`), and Rust std's `str::parse::<f64>()` *is* that algorithm (correctly-rounded
  → bit-identical). Integer radix paths port `parseIntWithRadix*` directly.
- **Validation:** a small C++ token-dump harness (`tools/js-lexer-dump/`) linking the real
  `JSLexer` is the byte-for-byte oracle.

**Support-layer prerequisites** (separate ports, sequenced before the lexer proper — NOT
part of SourceErrorManager; in build order):

| # | Dep | Hermes source | Note |
|---|-----|---------------|------|
| 1 | Token tables | `Parser/TokenKinds.def`, `HTMLEntities.def` | ✅ **Done** — `rust/crates/parser/src/token_kinds.rs` (`TokenKind`, `token_kind_str`, `binop_precedence`, `is_res_word`/`is_punctuator`, `match_reserved_word`; 6 tests). `HTMLEntities.def` deferred to JSX. Plan: `plans/2026-06-01-js-lexer-token-tables.md`. |
| 2 | C++ token-dump harness | links `JSLexer` | ✅ **Done** — `tools/js-lexer-dump/` (`add_hermes_tool`, build `cmake --build cmake-build-asan --target js-lexer-dump`). Emits `<start> <end> <nl> <KIND>[ fields]`; KINDs are `.def` variant names; numbers as f64 bits; byte-exact `\xHH` quoting (WTF-8 round-trips); `--context=regexp\|div`. Plan: `plans/2026-06-01-js-lexer-dump-harness.md`. **Known oracle limits** (documented in-tool, revisit when porting those paths): `template_middle`/`template_tail` and IDENT_OP (`as_operator`) need parser-driven rescans so a plain `advance()` loop never emits them; JSX/Flow contexts not yet wired. |
| 3 | String interning (`StringTable`/`UniqueString`) | `Support/StringTable.h` | copy juno `atom_table` + WTF-8 byte path (lone surrogates from `appendUnicodeToStorage` can't live in a Rust `String`). Unblocks the first slice with token tables. |
| 4 | Unicode char properties | `Platform/Unicode/CharacterProperties.{h,cpp}`, `UnicodeData.inc` | port the generated range tables verbatim (binary search); pin to Hermes's Unicode version — do NOT use a Rust unicode crate. |
| 5 | Number parsing | `Support/Conversions.h`, `FastStrToDouble.cpp` (`fast_float`) | pure Rust per the locked decision above. |
|   | Bump `Allocator` | `Support/Allocator.h` | **droppable** — Rust owns the decoded strings. |

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
