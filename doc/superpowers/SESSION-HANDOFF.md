# Session Handoff — Hermes → Rust front-end port

Hand this to a new session to restore context. It **references** the authoritative files
(read them; don't trust this summary over them) and records the conventions, file map,
validation commands, and workflow.

> **Date of handoff:** 2026-06-02. **Branch:** `rust` (base is `static_h`, NOT `main`).
> **Status:** the **JS lexer is COMPLETE** and the **JSONParser is COMPLETE** (the first lexer
> consumer); the next component is the **JS Parser** (`lib/Parser/JSParserImpl*`).

---

## 1. Read these first (in order)

1. **`doc/superpowers/RustPortRoadmap.md`** — THE source of truth: goal, principles, component
   order/status, locked decisions, the "✅ JS lexer — COMPLETE" summary, and "Next component: the
   Parser". Read it fully.
2. **`CLAUDE.md`** (repo root) — build commands, code style, the "never `cd`" rule, GC-safe-coding
   skill rule (for C++ VM code, not relevant to the Rust port), environment split (`facebook/`
   dir → Meta; else GitHub — this is the GitHub env).
3. **The auto-memories** at `/home/tmikov/.claude/projects/-home-tmikov-work-hermes/memory/`
   (loaded each session via `MEMORY.md`):
   - `rust-port-roadmap-pointer.md` — resume pointer (lexer COMPLETE, next = Parser).
   - `rust_port_conventions.md` — `rust/` layout, copy juno, keep close to Hermes + comments,
     docs under `doc/` not `docs/`, branch is `rust` (stays there, no PRs/merges), base `static_h`.
   - `implement-components-completely.md` — implement a component's WHOLE public surface in one
     pass; the user pushed back twice on deferred features. (The capstone review at the end of the
     lexer caught a stubbed fallthrough + missing accessors — this rule is why we fixed them.)
   - `lexer-number-parsing-fast-float.md` — lexer uses `fast_float` (not `dtoa`); Rust std
     `str::parse::<f64>()` is bit-identical → pure Rust, no FFI.
4. **`doc/superpowers/specs/2026-06-01-js-lexer-design.md`** — the lexer design (decisions, seams,
   scope, crate layout). **`doc/superpowers/specs/2026-06-01-source-error-manager-design.md`** —
   the diagnostics foundation.

---

## 2. What's done (✅) and the code map

**Component status** (see the roadmap table): `SourceErrorManager` ✅ · **JS lexer ✅** ·
**JSONParser ✅** · JS Parser / Sema / IR / Optimizer / BCGen — future.

Rust workspace: **`rust/Cargo.toml`** (members: `support`, `parser`, `atom_table`, `unicode`),
toolchain pinned `rust/rust-toolchain.toml` (1.96.0).

| Crate | What | `unsafe`? |
|-------|------|-----------|
| `rust/crates/support/` | `SourceErrorManager` + buffer/locations/line-index/diagnostics. Byte-for-byte vs `hermesc` (`tests/golden.rs`). | zero (`forbid`) |
| `rust/crates/atom_table/` | juno `atom_table` verbatim + `AtomBytes`/`atom_bytes` WTF-8 path (the interner). | **encapsulated** (sanctioned) |
| `rust/crates/unicode/` | `CharacterProperties` predicates + tables generated from `UnicodeData.inc` (17.0.0) by `gen_tables.py`. | zero (`forbid`) |
| `rust/crates/parser/` | **the lexer** + token tables + number parsing + UTF codec. | only in `cursor.rs` (scoped) |

**Lexer modules** (`rust/crates/parser/src/`): `token_kinds.rs` (TokenKind from `TokenKinds.def`),
`number.rs` (scanNumber primitives), `html_entities.rs` (generated), `utf8.rs` (UTF-8↔16 codec),
`cursor.rs` (the `*const u8` cursor — decision B, the ONLY parser `unsafe`), `token.rs`
(Token/RegExpLiteral/StoredComment/StoredToken), and `lexer/` split into
`{mod,escape,identifier,number,string,template,dump,regexp,jsx,state,lookahead}.rs` (each an
`impl<'a> JSLexer<'a>` block; child modules see the struct's private fields).

**The differential oracle:** `tools/js-lexer-dump/js-lexer-dump.cpp` — a C++ tool linking the real
`JSLexer`, registered via `add_hermes_tool` in `tools/CMakeLists.txt`. The Rust lexer dumps tokens
in the identical format and `rust/crates/parser/tests/differential.rs` asserts byte-for-byte
equality across 5 contexts (`--context=div/regexp/type/jsx` + `--jsx-child`) plus a non-strict
corpus (`--non-strict`, exercising the future-reserved-word downgrade + legacy octal paths).

**C++ source of truth for the lexer:** `include/hermes/Parser/JSLexer.h` + `lib/Parser/JSLexer.cpp`,
plus `include/hermes/Support/UTF8.h`, `Support/Conversions.h`/`FastStrToDouble.cpp`,
`Platform/Unicode/CharacterProperties.{h,cpp}` + `UnicodeData.inc`, `Parser/TokenKinds.def` +
`HTMLEntities.def`.

**JSONParser (✅ COMPLETE)** — the first `JSLexer` consumer. Code: `rust/crates/parser/src/json/`
(`mod.rs` value model + `emit_into` + `JSONSharedValue`, `factory.rs` uniquing/hidden-classes over a
`bumpalo` arena, `parser.rs` recursive descent) + `rust/crates/support/src/json_emitter.rs`
(`JSONEmitter` + `number_to_string`). New dep `bumpalo`; sole hand-written `unsafe` is
`JSONSharedValue::get`. Differential oracle: C++ `tools/json-parse-dump/` vs the Rust
`json-parse-dump` bin (`rust/crates/parser/src/bin/json_parse_dump.rs`), byte-for-byte over a 16-file
corpus (`tests/json_differential.rs` + `tests/json_corpus/`); plus 5 ported `JSONParserTest`
(`tests/json_parser_ported.rs`) + 13 ported `JSONEmitterTest` (inline). Benchmarked within ~1.5% of C++
Release (`gen-json` bin + `--bench=N`). **C++ source of truth:** `include/hermes/Parser/JSONParser.h` +
`lib/Parser/JSONParser.cpp`, `Support/JSONEmitter.{h,cpp}`, `Support/Conversions.cpp:211` (`numberToString`),
`unittests/Parser/JSONParserTest.cpp`, `unittests/Support/JSONEmitterTest.cpp`. Spec/plan:
`specs/2026-06-02-json-parser-design.md`, `plans/2026-06-02-json-parser.md`.

**Per-phase plans** (build log) under `doc/superpowers/plans/`: `js-lexer-*` (the 5 prereqs) and
`js-lexer-proper-{1a,1b-i,1b-ii,2a,2b,2c,3a,3b,4a,4b,4c}.md` (the lexer phases).

---

## 3. Build & validate (commands)

```bash
# Rust workspace (do NOT cd; use --manifest-path). Build/test:
cargo test  --manifest-path rust/Cargo.toml            # whole workspace (≈136 tests)
cargo test  --manifest-path rust/Cargo.toml -p parser  # lexer crate
cargo build --manifest-path rust/Cargo.toml            # expect ZERO warnings
cargo clippy --manifest-path rust/Cargo.toml -p parser # only pre-existing faithful-C-idiom lints

# The C++ differential oracle (build once; ASan+Debug+-O1 tree per CLAUDE.md):
cmake --build cmake-build-asan --target js-lexer-dump
# The differential test resolves the binary via CARGO_MANIFEST_DIR; force it to run (not skip):
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test differential -- --nocapture
# Expect: differential[div] 58, [regexp] 5, [type] 6, [jsx] 4, [jsx-child] 10, [nonstrict] 7 — all pass.

# JSONParser oracle + differential (same pattern):
cmake --build cmake-build-asan --target json-parse-dump
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test json_differential -- --nocapture
# Expect: "json differential: 16 corpus files matched" — pass. Benchmark (not committed):
#   cargo run --manifest-path rust/Cargo.toml -p parser --release --bin gen-json -- 100000 > /tmp/big.json
#   ./rust/target/release/json-parse-dump --bench=50 /tmp/big.json   (vs cmake-build-release/bin/json-parse-dump)
```

If `cmake-build-asan/` is missing, configure it (it's git-ignored):
`cmake -B cmake-build-asan -G Ninja -DCMAKE_BUILD_TYPE=Debug -DHERMES_ENABLE_ADDRESS_SANITIZER=ON -DCMAKE_CXX_FLAGS="-O1" -DCMAKE_C_FLAGS="-O1"`.

---

## 4. Locked decisions & faithful-port conventions (don't relitigate)

- **Scan cursor:** raw `*const u8` (decision B), confined to `cursor.rs`, offset at every boundary;
  `Rc<SourceBuffer>` backing; the `NullTerminatedBuf` trailing NUL makes lookahead in-bounds.
- **Interner:** juno `atom_table` copied verbatim (keeps its encapsulated unsafe) + a WTF-8 byte path.
- **Numbers:** pure Rust — `str::parse::<f64>()` == `fast_float` (no FFI/`dtoa`); integer radix paths
  ported by hand; rounding validated vs a `u128`→f64 oracle.
- **Locations:** offset-based `SMLoc = (SourceId, u32)`; the C++ Token's raw pointers → offsets +
  `AtomBytes` handles. Pointer→offset adaptations are commented where a C++ method moved (e.g.
  `Token::checkFollowingCharacter`/`inputStr` became `JSLexer` methods; `StoredComment::getString`
  takes the buffer bytes).
- **C++ RAII guards → explicit methods** (SavePoint is a value struct + `restore`; suppress is
  save/restore; lookahead's `make_scope_exit` is explicit). **C++ `template`s stay generics**
  (monomorphized like the original — `template<bool>` → `const` generic; `template <IdentifierMode>`
  → the `IdMode` marker trait + `JsMode`/`JsxMode`/`FlowMode` ZSTs; `scanString<JSX>` → `const JSX`).
  Do NOT flatten a template to a runtime param. **Parser `Keywords&` → pass the needed atom.**
- **Diagnostics byte-compatible** with `hermesc` (inherited from `support`).
- We keep C-idiom comparisons (`>= '0' && <= '9'`) faithful to the C++ over clippy style lints, but
  fix genuine new lints (`int_plus_one`, `never_loop`, `approx_constant`, `needless_return`) with
  scoped `#[allow]` + comment or a clippy-clean rewrite. Gate on `cargo build` warnings (zero).
- **Only `getAllocator` has no Rust analog** (no bump allocator) — the one documented surface gap.

---

## 5. Workflow that's been used (keep using it)

**Subagent-driven development** (the `superpowers:subagent-driven-development` skill), one component
phase at a time:

1. Read the relevant C++ closely; write a **plan** under `doc/superpowers/plans/YYYY-MM-DD-<phase>.md`
   (the `superpowers:writing-plans` skill; TDD steps, exact C++ line ranges to port, no placeholders).
2. Dispatch a **general-purpose implementer subagent** with the full plan embedded + the conventions
   above. It does TDD and commits per task.
3. **Spec-compliance review** subagent (independent: build, run, read code vs the C++ — do NOT trust
   the implementer's report; the differential test is the byte-for-byte gate).
4. **Code-quality review** subagent (after spec passes).
5. Apply fixes (small ones directly; larger via a fix subagent), re-verify, commit.
6. Update the roadmap; move to the next phase.
7. **At component end: a capstone review of the WHOLE component** — it caught real bugs the
   per-phase reviews missed (a silently-skipping differential test; a `lookahead1` rollback bug; a
   stubbed `advance` fallthrough). Always do this.

Commit messages end with `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
Commit directly to `rust`; **never** open a PR or merge (project rule).

---

## 6. What's next

- **The JS Parser** (`lib/Parser/JSParserImpl.cpp` + `JSParserImpl-flow.cpp`/`-jsx.cpp`/`-ts.cpp`,
  `JSParserImpl.h`, `include/hermes/Parser/JSParser.h`) — consumes the completed lexer. It needs the
  **AST** (`lib/AST/`, `include/hermes/AST/`) and `Context`. This is a large component; scope it
  (juno has an AST + parser to crib from: `unsupported/juno/crates/juno_ast/`, `juno/src/hparser/`).
- The **lexer** has no open items (the optional `--non-strict` follow-up is DONE — `differential_nonstrict`).
- The **JSONParser** has no open items. Capstone passed; sole documented deviations are the fat-enum
  node layout and `getAllocator`/`getStringTable` → `arena()`/`atoms()`. The differential caught one real
  bug (an `emit_into` WTF-8 panic) which is fixed.

The lexer and JSONParser are done, reviewed, and self-validating — start the JS Parser fresh from §1's
reading (note: it is a far larger effort than the JSONParser, which was a deliberately small first
lexer-consumer to work out the integration pattern).
