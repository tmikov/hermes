# Session Handoff — Hermes → Rust front-end port

Hand this to a new session to restore context. It **references** the authoritative files
(read them; don't trust this summary over them) and records the conventions, file map,
validation commands, and workflow.

> **Date of handoff:** 2026-06-19. **Branch:** `rust` (base is `static_h`, NOT `main`).
> **Status:** the **JS lexer**, **JSONParser**, and the **AST are COMPLETE**, and the **JS Parser is IN PROGRESS** —
> **phases P0 (foundations + `parser_differential` gate), P1 (value expressions), P2 (statements & declarations), P3 (functions, classes,
> arrows, async/generators, methods, `super`, `yield`, decorators), P4 (modules), P5 (the FLOW TYPE GRAMMAR + declarative integration),
> P6 (the REST OF FLOW), P7 (ALL of TYPESCRIPT), and P8 (JSX) are DONE**, byte-for-byte vs `hermesc -dump-ast` (+ the matching
> hidden/dialect flag) over a **76-file plain + 42 Flow + 8 component + 5 records + 7 match + 20 TS + 6 JSX + 1 flow·JSX corpus**, each
> sub-task two-stage reviewed (spec w/ adversarial diffing + quality) + a whole-phase capstone, zero warnings, zero new clippy lints,
> `generated_idempotent` green (P6, P7 AND P8 added NO AST nodes). **The parser now handles the ENTIRE standard-ECMAScript grammar + ALL of
> Flow + ALL of TypeScript + JSX** — Flow's type grammar (P5) + ambiguous-expression grammar + `enum`/`component`/`hook`/`record`/`match`/
> `declare` (P6), TypeScript's full type grammar + interface/enum/namespace + `<Type>` casts/`as`/typed arrows/class modifiers/`import type`
> (P7), and JSX elements/fragments/children/attributes/spread/expression-containers/namespaced+member names/closing-tag matching (P8), behind
> seven `Context` flags (`parse_flow` + `parse_flow_ambiguous`/`_component_syntax`/`_records`/`_match`, `parse_ts` — mutually exclusive with
> `parse_flow` — and `parse_jsx`, an independent flag) that do NOT leak into plain JS. **Only the Pre/Lazy passes remain.**
> **Read `doc/superpowers/RustPortRoadmap.md` (the "🚧 JS Parser" section, the P5 + P6 + P7 + P8 DONE blocks) for the authoritative detail,
> the remaining deferral set (→ the **Pre/Lazy passes**: the `pass_`/PreParse/Lazy machinery + `SaveFunctionState` the eager Full pass
> no-ops; the `_param_yield`/`_param_await` args threaded into `parse_function_body` are the dormant hooks), and the review-caught bugs.
> **Port-wide lessons, re-confirmed by P6, P7 AND P8:** (1) **C++ DEFAULT ARGUMENTS are spec** — read the header.
> **`lexer.lookahead1(None)` defaults to `RequireNoNewLine = true` (JSLexer.h:658)** → always `::<true>`; and the grammar-context arg is
> per-call-site spec (TS uses `GrammarContext::Type` pervasively so `>>` splits for nested generics, with `AllowRegExp` exceptions; JSX uses
> `AllowJSXIdentifier` almost everywhere, with `AllowRegExp` at the `{`-child / spread / attribute-value advances, and a `jsx_depth`-driven
> `advance_in_jsx_child`-vs-`advance` lexer-mode switch at 4 sites — getting that branch wrong corrupts the post-JSX token stream).
> (2) **Two-stage review + adversarial differential diffing catches what the corpus misses.** (3) **A capstone that maps every dialect-gated
> C++ site to its Rust production** confirms completeness — for P7/P8 it verified all `getParseTS()`/`getParseJSX()` sites are wired AND that
> every silent-drop candidate is **rejected by hermesc itself** (so the omissions are correct). Specs/plans:
> `specs/2026-06-06-js-parser-design.md`, `plans/2026-06-06-js-parser-{p0,p1,p2}…`, `…p3…`, `…p4…`,
> `2026-06-09-js-parser-p5-flow-types.md`, `2026-06-13-js-parser-p6-flow-extensions.md`, `2026-06-19-js-parser-p7-typescript.md`,
> `2026-06-19-js-parser-p8-jsx.md`.
> **NEXT: the Pre/Lazy passes** (the three-pass Full/Pre/Lazy machinery — `SaveFunctionState`, lazy-function deferral, the `pass_` blocks the
> eager Full pass currently no-ops). After that the Parser component is COMPLETE and **Sema** (scope resolution + FlowChecker) is the next
> component. Write each phase plan just-in-time (lexer/P1–P8-style) and execute subagent-driven. juno has no parser to crib from (`hparser`
> is FFI-to-C++); port the C++ directly.
> The parser proper lives in `rust/crates/parser/src/js/{mod,expressions,statements,functions,classes,modules,jsx}.rs` +
> **`js/flow/{mod,declarations,types,function_types,object_types,params,match_}.rs`** + **`js/ts/{mod,types,function_types,object_types,
> declarations,params}.rs`**; the gate is `REQUIRE_DIFFERENTIAL=1 cargo test -p parser --test parser_differential` (build `ast-dump`
> first; the Flow/TS/JSX corpora need `cmake-build-asan/bin/hermesc`; gated dirs `parser_corpus_flow{,_component,_records,_match}`,
> `parser_corpus_ts`, `parser_corpus_jsx`, `parser_corpus_jsx_flow` each pass their hidden/dialect flag(s) to both binaries).

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
   - `rust-port-roadmap-pointer.md` — resume pointer (AST phases 1–3 done; next = AST phase 4 ESTreeJSONDumper).
   - `dont-pronounce-on-hermes-internals.md` — don't state guesses about Hermes internals as
     conclusions; verify against the C++ or defer (the user is a Hermes author; overconfidence annoys).
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
5. **`doc/superpowers/specs/2026-06-03-ast-design.md`** — the AST design (juno GC arena; immutable
   children + `Cell` attributes; **references not index handles**; the verified no-`Cell<&Node>`
   finding; the `ESTree.def` codegen approach) and the executed phase-1 plan
   **`doc/superpowers/plans/2026-06-04-ast-1-storage-and-spine.md`**.

---

## 2. What's done (✅) and the code map

**Component status** (see the roadmap table): `SourceErrorManager` ✅ · **JS lexer ✅** ·
**JSONParser ✅** · **AST ✅ (all 4 phases: GC spine + generated 271-node set + transforming visitor + `ESTreeJSONDumper`)** ·
JS Parser (next) / Sema / IR / Optimizer / BCGen — future.

Rust workspace: **`rust/Cargo.toml`** (members: `support`, `parser`, `atom_table`, `unicode`),
toolchain pinned `rust/rust-toolchain.toml` (1.96.0).

| Crate | What | `unsafe`? |
|-------|------|-----------|
| `rust/crates/support/` | `SourceErrorManager` + buffer/locations/line-index/diagnostics; `JSONEmitter`; **+ `Deque`/`HeapSize`** (shared utilities, moved from juno); **+ `utf8` WTF-8↔UTF-16 codec** (faithful copy of the subset of `parser::utf8` the JSON dumper needs; gained a `unicode` dep). Byte-for-byte vs `hermesc` (`tests/golden.rs`). | zero (`forbid`) |
| `rust/crates/atom_table/` | juno `atom_table` verbatim + `AtomBytes`/`atom_bytes` WTF-8 path (the interner). | **encapsulated** (sanctioned) |
| `rust/crates/unicode/` | `CharacterProperties` predicates + tables generated from `UnicodeData.inc` (17.0.0) by `gen_tables.py`. | zero (`forbid`) |
| `rust/crates/parser/` | **the lexer** + token tables + number parsing + UTF codec + the JSONParser. | only in `cursor.rs` (scoped) |
| `rust/crates/ast/` | **the AST** — juno **GC arena** (`Context`/`GCLock`/`NodeRc` + mark-sweep) copied+adapted; immutable children + `Cell` attributes; **full 271-node set generated from `ESTree.def`** by `gen_nodes.py` → `// @generated src/node.rs` (phase 2). `NodeKind`+ranges, `is_*`/`as_*`, `visit_children`/`mark_lists`, `new`. **Phase 3:** generated `builder` module + `VisitorMut`/`TransformResult`/`Path`/`NodeField` + `visit_children_mut` (functional rebuild); read `Visitor` unchanged. **Phase 4:** generated `node_type_str` + `dump_children`, hand-written `src/dump.rs` (`ESTreeJSONDumper`); 9 `tests/dump_golden.rs`. | only in `context.rs` (scoped) |

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
cargo test  --manifest-path rust/Cargo.toml            # whole workspace (≈236 tests)
cargo test  --manifest-path rust/Cargo.toml -p parser  # lexer + JSONParser crate
cargo test  --manifest-path rust/Cargo.toml -p ast     # AST: GC arena + generated 271-node model + spine/structural/transform/dump_golden tests
cargo build --manifest-path rust/Cargo.toml            # expect ZERO warnings
cargo clippy --manifest-path rust/Cargo.toml -p parser # only pre-existing faithful-C-idiom lints

# Regenerate the AST node set from ESTree.def (committed output; idempotent):
python3 rust/crates/ast/gen_nodes.py                   # writes src/node.rs (271 nodes); re-run = no diff
# Guard that committed src/node.rs matches the generator (force-run, don't skip):
REQUIRE_GEN=1 cargo test --manifest-path rust/Cargo.toml -p ast --test generated_idempotent

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
   the implementer's report; the differential test is the byte-for-byte gate). **Structural-fidelity
   check (the differential CANNOT catch this):** for every C++ `template`/specialization in the ported
   range, confirm it stayed a Rust generic (`const` generic / marker-trait type param) and was NOT
   flattened to a runtime `bool`/enum param — runtime dispatch is behaviorally identical (so the
   differential passes) but changes codegen and is an unauthorized deviation. Grep the C++ source range
   for `template <` and check each one. Likewise flag any other silent structure change (template↔runtime,
   layout, RAII→explicit beyond the agreed list).
4. **Code-quality review** subagent (after spec passes).
5. Apply fixes (small ones directly; larger via a fix subagent), re-verify, commit.
6. Update the roadmap; move to the next phase.
7. **At component end: a capstone review of the WHOLE component** — it caught real bugs the
   per-phase reviews missed (a silently-skipping differential test; a `lookahead1` rollback bug; a
   stubbed `advance` fallthrough). Always do this. **Re-run the structural-fidelity check from step 3
   over the whole component** — the lexer's `template`→runtime-param flattening (`lookahead1/2`,
   `scanIdentifierFastPath/Parts<IdentifierMode>`, `scanString<JSX>`) slipped past every per-phase
   review AND the capstone because the differential is byte-identical either way; it was only caught
   later by eye. Grep the component's C++ for `template <` and verify each survived as a generic.

Commit messages end with `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
Commit directly to `rust`; **never** open a PR or merge (project rule).

---

## 6. What's next

- **The JS Parser (IN PROGRESS — P0/P1/P2/P3/P4 DONE; standard-JS grammar COMPLETE; Flow/TS/JSX next)** (`lib/Parser/JSParserImpl.cpp` +
  `-flow.cpp`/`-jsx.cpp`/`-ts.cpp`, `JSParserImpl.h`, `include/hermes/Parser/JSParser.h`) — consumes the lexer + AST + `Context`. The parser
  proper lives in `rust/crates/parser/src/js/{mod,expressions,statements,functions,classes,modules}.rs`; the `ast-dump` bin + C++
  `hermesc -dump-ast` differential (`tests/parser_differential.rs`, `REQUIRE_DIFFERENTIAL=1`, **75-file corpus**) is the validation gate, and it
  exercises the AST's `ESTreeJSONDumper` byte-for-byte. **P0** = foundations + gate; **P1** = value expressions; **P2** = statements &
  declarations; **P3** = functions/classes/arrows/async/generators/methods/`super`/`yield`/decorators; **P4** = modules: `import`/`export`
  declarations + `import()`/`import.meta` (in `js/modules.rs`). **NEXT: Flow (P5/P6), TS (P7), JSX, and the Pre/Lazy passes** — all the
  remaining honest-omission `// P5/P6/P7` blocks (incl. the P4 `import type`/`export type`/Flow default-exports/type-kind branches). juno has an
  AST to crib from but NOT a parser (`hparser` is FFI-to-C++); port the C++ directly. Write each phase plan just-in-time (lexer/P1–P4-style) and
  execute subagent-driven. **Port-wide RAII pattern:** C++ `SaveAndRestore`/`SaveFunctionState` → `Rc<Cell<bool>>` Drop-guards (`ParamFlagGuard`,
  `RecursionGuard`) or explicit save/restore wrappers that survive `?` early-returns — strict-mode + param-yield/await state must not leak.
  **Port-wide contextual-keyword pattern (P4 lesson):** C++ `check(<ident>Ident_)` is escape-INsensitive (`check(UniqueString*)`) → use Rust
  `check_name`; only C++ `checkUnescaped(...)` maps to `check_unescaped_name`. Confusing them is a real bug (caught in P4 `import.meta`).
- **AST phase 4 is DONE** (`ESTreeJSONDumper`): the generator emits `Node::node_type_str` (JSON `"type"` == variant
  name) + `Node::dump_children` (walks ONLY `.def`-arg fields in declaration order — no decorations — baking the
  **retained camelCase `.def` names** as literal JSON keys + a per-field `ESTREE_IGNORE_IF_EMPTY` flag, validated against
  real nodes/fields). `src/dump.rs` is the driver: `ESTreeDumpMode{Compact,HideEmpty,DumpAll}`/`LocationDumpMode`/
  `ESTreeRawProp`, the `field_*`/`dump_*` helpers, `visit`, `print_source_location`, `dump_raw`, and public
  `dump_estree_json` (no-sm) + `dump_estree_json_with_sm`. Labels/strings emit WTF-8→UTF-16 via the new
  `support::utf8` codec + `emit_u16` (byte-matching C++ `primitiveEmitString`). 9 `tests/dump_golden.rs` cases +
  idempotency guard; whole-component capstone **APPROVED**. **Deliberate deviations (2, model-driven):** (a) `"raw"`
  needs the buffer (offset model has no location pointer) → omitted in the no-sm overload; (b) `StackOverflowGuard` →
  a plain 128-depth counter. Plan: `plans/2026-06-05-ast-4-json-dumper.md`.
  - **Tracked follow-up (not a blocker):** the C++ third overload (`dumpESTreeJSON(JSONEmitter&, …, includeSourceLocs,
    …)` — caller-owned emitter, no `endJSONL`, `NodeKindSet` filter) is not exposed, and `include_source_locs`/the
    depth limit have no public setter (both plumbed + tested internally). Add the thin wrapper when a consumer
    (LSP/debugger) needs it. Also: the dumper uses the **translated** `find_coords` for `loc` (matching the existing
    code), where C++ `printSourceLocation` uses `findBufferLineAndLoc` — confirm equivalence when the Parser
    differential wires real source locations.
- **No open items** on the lexer, the JSONParser, or the **AST** (all 4 phases): each two-stage reviewed per task +
  whole-component capstone (phase 2 — re-derived the 271-node count, `NodeKind` ordering, decoration composition, all
  `NodeList` fields traced in BOTH `visit_children`/`mark_lists`; phase 3 — `NodeChild` Removed/Expanded semantics,
  list-rebuild off-by-one, `Cell`-vs-ref `from_node` copying, decoration `Cell<NodeList>`s never threaded; phase 4 —
  `"type"`==`#NAME`, `.def`-order field walk with no decorations leaking, `isEmpty`/skip semantics per mode,
  `IGNORE_IF_EMPTY` baked 1:1, WTF-8 label emission byte-faithful, structural-fidelity grep found no
  template→runtime flattening, idempotency clean). Phase-4 deliberate scope: the two deviations above + the tracked
  follow-up. The lexer's `--non-strict` follow-up is DONE; the JSONParser's sole deviations are the fat-enum layout +
  `getAllocator`/`getStringTable` → `arena()`/`atoms()`.
