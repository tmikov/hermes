# Session Handoff — Hermes → Rust front-end port

Hand this to a new session to restore context. It **references** the authoritative files
(read them; don't trust this summary over them) and records the conventions, file map,
validation commands, and workflow.

> **Date of handoff:** 2026-07-15. **Branch:** `rust` (base is `static_h`, NOT `main`).
> **Status:** the **JS lexer**, **JSONParser**, the **AST**, and now the **JS Parser are ALL COMPLETE** —
> **phases P0 (foundations + `parser_differential` gate), P1 (value expressions), P2 (statements & declarations), P3 (functions, classes,
> arrows, async/generators, methods, `super`, `yield`, decorators), P4 (modules), P5 (the FLOW TYPE GRAMMAR + declarative integration),
> P6 (the REST OF FLOW), P7 (ALL of TYPESCRIPT), and P8 (JSX) are DONE**, byte-for-byte vs `hermesc -dump-ast` (+ the matching
> hidden/dialect flag) over a **77-file plain + 42 Flow + 8 component + 5 records + 7 match + 20 TS + 6 JSX + 1 flow·JSX corpus**, each
> sub-task two-stage reviewed (spec w/ adversarial diffing + quality) + a whole-phase capstone, zero warnings, zero new clippy lints,
> `generated_idempotent` green (P6, P7 AND P8 added NO AST nodes). **The parser now handles the ENTIRE standard-ECMAScript grammar + ALL of
> Flow + ALL of TypeScript + JSX** — Flow's type grammar (P5) + ambiguous-expression grammar + `enum`/`component`/`hook`/`record`/`match`/
> `declare` (P6), TypeScript's full type grammar + interface/enum/namespace + `<Type>` casts/`as`/typed arrows/class modifiers/`import type`
> (P7), and JSX elements/fragments/children/attributes/spread/expression-containers/namespaced+member names/closing-tag matching (P8), behind
> seven `Context` flags (`parse_flow` + `parse_flow_ambiguous`/`_component_syntax`/`_records`/`_match`, `parse_ts` — mutually exclusive with
> `parse_flow` — and `parse_jsx`, an independent flag) that do NOT leak into plain JS. **The Pre/Lazy passes are now DONE too — the three-pass
> Full/Pre/Lazy machinery + `parse_lazy_function`, gated by two oracles (Oracle B: a C++ `preparse-dump` byte-for-byte side-table differential vs
> hermesc; Oracle A: Rust-only reparse-equivalence of deferred bodies vs the eager AST). So the PARSER COMPONENT IS COMPLETE; the next component
> is Sema.** (See the roadmap's "✅ JS Parser — COMPLETE" Pre/Lazy block for detail, incl. the two capstone-caught bugs: strict-mode must be set by
> the CALLER of `parse_lazy_function` per `HBC.cpp:158`, and `SaveFunctionState` must be constructed AFTER formal params per cpp:510.)
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
> **NEXT: Sema** (scope resolution + FlowChecker) — the Parser is now COMPLETE. The Pre/Lazy open design question (how to validate
> lazy parsing when `-dump-ast` is blind to it) was resolved in `specs/2026-06-28-pre-lazy-passes-design.md`: TWO oracles — Oracle B (a C++
> `tools/preparse-dump/` tool + Rust bin + byte-for-byte `preparse_differential` of the `PreParsedBufferInfo` side-table vs hermesc, 13+77 files)
> and Oracle A (Rust-only `lazy_reparse` proving deferred bodies reparse to the eager hermesc-verified AST). Both shipped; the capstone caught a
> real flag-attribution bug a corpus-gated differential alone would miss (default-param arrows — `SaveFunctionState` was built before params
> instead of after; cpp:510). **PreParse scoped reclamation is also DONE (2026-07-15):** the `AllocationScope` discipline is now ported —
> `ast::AllocationScope` + `support::Deque::truncate`/`iter_from` at the two C++ scope sites (cpp:516-560 and cpp:7523); peak PreParse AST
> reduced 33× (251 vs 8,351 nodes mid-pass; 0 residual post-pass), gated by `tests/preparse_memory.rs`. Oracle B now covers Flow/TS too
> (151 files total); Oracle A now compares LOCATED dumps (78/78). Spec: `specs/2026-07-15-preparse-scoped-reclamation-design.md`. The
> `AllocationScope` deviation is retired; the sole remaining faithful-port deviation is the side-table-threaded-on-parser.
> **Sema has no `-dump-ast` analog either — decide its validation oracle during brainstorming.** Open the session with
> `superpowers:brainstorming`, THEN `writing-plans`. Write each phase plan just-in-time and execute subagent-driven. juno has no parser to crib
> from (`hparser` is FFI-to-C++); port the C++ directly.
> **Update (2026-07-26): Sema S0 (foundations) is DONE.** Commits `bd4090d17..7f097b899` on `rust`.
> **Update (2026-07-28): Sema S1 (declarations & scopes) is DONE.** Commits `53ddf2e92..77a41ed3e` on `rust`.
> S1 shipped the resolver as a direct `ast::VisitorMut` implementation (one phase early — the C++'s generic `Node **ppNode`
> replacement, used by constant folding, needed the mechanism from the start), hermesc error-epilogue parity, `ASTEval`
> constant folding, identifier resolution, the full declaration/redeclaration matrix, expression fold+validation wiring,
> function/parameter scopes, and a 69-file `test/Sema` corpus sweep with `MANIFEST.md`.
> **Update (2026-07-28): Sema S2 (rest of the walk) is DONE.** Commits `94b4695f1..dc2fb1661` on `rust`. Nine tasks:
> loops/labels/`break`/`continue`/`switch`; arrows + §3.4 rewrite #1 + `yield`/`await`/spread/meta + the `Cover*` errors;
> try/catch + rewrite #2 + `with`/`Unresolver` + the regexp visit (an explicit regex-engine deferral); classes +
> `ClassContext` + `super`; private names + static blocks; call specials (direct `eval` + rewrite #3 `$SHBuiltin` +
> `super()`); `CheckImplicitReturn`; a round-2 corpus sweep that also ran both binaries over 1416 upstream `test/` files;
> docs. **Three of the four §3.4 rewrites have now shipped** (#4, anonymous `export default function`, is S4 with the module
> visits). New resolver modules `resolver/{statements,unresolver,classes,calls}.rs` plus `src/check_implicit_return.rs`.
> Gate (live, green):
> `REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema --features dump-bin --test sema_differential -- --nocapture`
> → "sema differential (tests/sema_corpus): 160 corpus files matched" (88 succeed on hermesc).
> **Update (2026-07-29): Sema S3 (`ScopedFunctionPromoter`) is DONE.** Commits `36593518b..274fa63b8` on `rust`. T1:
> ports all 328 lines of `lib/Sema/ScopedFunctionPromoter.cpp` (+ header) as `resolver/promoter.rs` and replaces both
> S3 assert seams (`visit_program` cpp:224-227, `visit_function_body_after_params_visited` cpp:1904-1910) with the
> real promotion + `process_promoted_func_decls` (cpp:2129-2141), making S1 T5's dormant `promotedFuncDecls`
> redeclaration rows live; the third C++ call site (`runInScope`, cpp:158) is left to S5 with a note at the site. T2:
> a seven-file promotion corpus battery + the three unblocked `test/Sema` rows. T3: an upstream re-probe over the
> same 1416-file corpus confirming zero S3-attributable panics (**1209**/190/**17**, was 1203/190/23). New resolver
> module `resolver/promoter.rs`.
> Gate (live, green):
> `REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema --features dump-bin --test sema_differential -- --nocapture`
> → "sema differential (tests/sema_corpus): 173 corpus files matched" (97 succeed on hermesc; +1/+1 from the S3
> final-review follow-up's `promotion-for-family-let-blocker.js`, added after 172/96 was reached).
> **Update (2026-08-03): Sema S4a (standalone-front-end sema) is DONE.** Commits `041959a07..57221f7de` on
> `rust`. Six tasks: T1 the `// FLAGS: <hermesc args>` per-file harness (`sema_differential.rs`) + `sema-dump`
> growing `-enable-eval`/`-fstd-globals`/`-fno-std-globals` (plus an unplanned `command_line` fix so hermesc's
> single-dash long-option spelling parses) + the `TypeAlias` do-nothing visit; T2 the SECOND differential oracle
> pair — C++ `tools/sema-parser-dump/` vs `sema-dump --parser-entry`, both driving the new
> `resolve_ast_for_parser`/`resolveASTForParser` (`SemResolve.cpp:295-306`, `compile = false` — the actual
> `hermes-parser-wasm.cpp` entry) — plus a fix round making the C++ oracle's stderr colorless and giving the
> resolver a `run_always` method so "dump despite errors" genuinely works; T3 `resolver/modules.rs` — the four
> module visits (asymmetric guards preserved: import's error unconditional, export's `compile_`-gated) with §3.4
> **rewrite #4** (anonymous `export default function` → `FunctionExpression`) ported INLINE, `FunctionInfo::imports`
> backref, and 11 corpus files (2 authored module-error pins + 9 upstream sweep imports); T4 the untyped `-parse-flow` battery (deriving the real
> `CoverTypedIdentifierNode`-reaching shape, `(x?: number);`) plus a fix round porting the
> `TypeCastExpression`/`AsExpression` visits a first pass missed; T5 an upstream re-probe confirming zero
> S4a-attributable panics (**1218**/190/**8**, was 1209/190/17) and a sweep-tooling landmine (`--release` masks
> the `computed-fn-name.js` repro — sweep only meaningful with debug builds both sides). New resolver module
> `resolver/modules.rs`; new C++ tool `tools/sema-parser-dump/`; new corpus `sema_corpus_parser/`.
> Gates (live, green):
> `REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema --features dump-bin --test sema_differential -- --nocapture`
> → "sema differential (tests/sema_corpus): **196 corpus files matched (103 succeeded on hermesc)**" and
> "sema differential (tests/sema_corpus_parser): **11 corpus files matched (3 succeeded on the oracle)**" (the
> second gate is new in S4a; 7/2 at the end of T6, 11/3 after the final review's four added pins). Deferred
> `test/Sema` rows: 4.
> **Read the roadmap's Sema row for the authoritative what-shipped detail and the S4b/S5 carry-item list** —
> S4b VM modules (`$SHBuiltin` protocol + CJS wrapping + rewrite #4's `-commonjs` corpus pinning — the rewrite's
> CODE already shipped in S4a, per the 2026-08-03 spec §4 ruling; a genuinely separate much-later phase near
> IRGen; shared "S4" number is renumbering-avoidance only); S5 lazy/`eval` + the third promotion call site
> `runInScope` (cpp:158); the regex engine as its own future component; the documented landmines (same-location
> diagnostic ties, THREE hermesc self-aborts — `class C { x = class {}; }`, `$SHBuiltin.#x()`, and
> `using x = 1; { function f(){} }` — plus a FOURTH found in S4a: the dumper itself aborts on anonymous
> `export default function(){}` dumped under `compile = false`, `SemContext.cpp:493-494` vs `dump_context.rs:304`,
> permanently excluded from `sema_corpus_parser`); and the THREE tracked parser-phase follow-ups — the 180-file
> `errorExpected` same-line-range gap (open, measured by the sema sweeps); the recursion-depth gap **CLOSED
> 2026-08-04**: site parity was verified 1:1 rather than fixed, and the real defects were a `>` vs `>=`
> off-by-one plus the limit constants (the ASan oracle takes C++'s `HERMES_LIMIT_STACK_DEPTH` branch, 128/512,
> against the port's hardcoded 1024/1024 — now `cfg!(debug_assertions)`-selected), which also closed both
> stack-overflow crashes (sweep 1218/190/8 → 1220/188/8); plus (c) **CLOSED 2026-08-04**: Rust `parse()`, which had
> omitted `JSParserImpl::parse`'s trailing error-count gate (every caller compensated for it), now ports the gate
> verbatim (`JSParserImpl.cpp:168-172`) — the compensating checks in `sema-dump` and `ast-dump` stay as redundant
> defense in depth.
> **Update (2026-08-04): whole-Sema capstone (pre-S5) is DONE.** Commits `e31c1a1d8` (fixes) +
> `502ac85c3` (re-review closeout) on `rust`. A whole-component review across S0–S4a, run
> deliberately BEFORE S5, for the publication handoff. Verdict **APPROVED WITH FIXES** (first
> pass: 0 Critical/2 Important/3 Minor; the re-review closeout: 4 further Minors + 1 guard pin,
> same day). Verified: a completeness mapping of `SemanticResolver.h`'s **~62** `visit()`
> overloads plus every other public surface, zero silent drops; structural fidelity clean —
> templates stayed generic, RAII ported as explicit save/restore, every unported seam loud and
> correctly phase-tagged. Caught: (F1) ten reachable untyped `-parse-flow` shapes panicking
> where hermesc exits 0 — fixed, driver gate 196/103 → **200/107**; (F2) `calls.rs`'s
> `$SHBuiltin.moduleFactory` seam comment's false compile-premise — corrected; landmine (v)
> below (`sema-parser-dump`'s `with (o) { x; }` self-abort) — documented, deliberately not
> mirrored; (F3) `sema-dump`'s CLI usage-error exit code (0 → 1, matching hermesc). The
> re-review closeout then re-verified the fix commit's own citations against the C++ (not the
> review's own list, itself partially drifted), found `TypeParameterDeclaration`
> reachable-and-load-bearing rather than "unreachable in practice", fixed three further
> citation/shape drifts, held the gate at 200/107, and added a `flow_range_size_is_97` unit pin
> (the `keywords.rs` `count_is_133` precedent). **S5 gets a delta-capstone, not a from-scratch
> one** — this capstone already covers S0–S4a end to end. See the roadmap's Sema row for the
> full breakdown.
> **NEXT: S5 — lazy + `eval` entry points.** `resolve_ast_lazy`/`resolve_ast_in_scope`, `visitProgram`'s unported
> `SaveAndRestore` of `globalScope_` (cpp:216-217, only observable once `Program` can recur), and the THIRD
> `ScopedFunctionPromoter` call site `runInScope` (`SemanticResolver.cpp:158`, promotes BEFORE
> `processCollectedDeclarations` rather than after); a whole-component capstone. **NO S5 plan exists yet** —
> open with `superpowers:brainstorming`, THEN `superpowers:writing-plans`, and execute it subagent-driven
> (`superpowers:subagent-driven-development`). Spec: `specs/2026-07-26-sema-untyped-design.md`; the executed plans
> are `plans/2026-07-26-sema-s0-foundations.md`, `plans/2026-07-28-sema-s1-declarations-scopes.md`,
> `plans/2026-07-28-sema-s2-rest-of-walk.md`, `plans/2026-07-29-sema-s3-scoped-function-promoter.md`,
> `plans/2026-08-03-sema-s4a-standalone-frontend.md`.
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
**JS Parser ✅ (P0–P8 + Pre/Lazy passes, entire standard-JS + Flow + TypeScript + JSX grammar)** ·
**Sema 🚧 (S0 foundations + S1 declarations & scopes + S2 rest-of-the-walk + S3 `ScopedFunctionPromoter` + S4a standalone-front-end sema DONE; S5 lazy/`eval` next, S4b VM modules much later)** / IR / Optimizer / BCGen — future.

Rust workspace: **`rust/Cargo.toml`** (members: `support`, `parser`, `atom_table`, `unicode`, `ast`, `command_line`, `sema`),
toolchain pinned `rust/rust-toolchain.toml` (1.96.0).

| Crate | What | `unsafe`? |
|-------|------|-----------|
| `rust/crates/support/` | `SourceErrorManager` + buffer/locations/line-index/diagnostics; `JSONEmitter`; **+ `Deque`/`HeapSize`** (shared utilities, moved from juno); **+ `utf8` WTF-8↔UTF-16 codec** (faithful copy of the subset of `parser::utf8` the JSON dumper needs; gained a `unicode` dep). Byte-for-byte vs `hermesc` (`tests/golden.rs`). | zero (`forbid`) |
| `rust/crates/atom_table/` | juno `atom_table` verbatim + `AtomBytes`/`atom_bytes` WTF-8 path (the interner). | **encapsulated** (sanctioned) |
| `rust/crates/unicode/` | `CharacterProperties` predicates + tables generated from `UnicodeData.inc` (17.0.0) by `gen_tables.py`. | zero (`forbid`) |
| `rust/crates/parser/` | **the lexer** + token tables + number parsing + UTF codec + the JSONParser. | `cursor.rs` (scoped `*const u8`); two statement-scoped `#[allow(unsafe_code)]` at the `alloc_scope` call sites in `functions.rs` + `pre_lazy.rs` (sanctioned; no-escape contract documented) |
| `rust/crates/ast/` | **the AST** — juno **GC arena** (`Context`/`GCLock`/`NodeRc` + mark-sweep) copied+adapted; immutable children + `Cell` attributes; **full 271-node set generated from `ESTree.def`** by `gen_nodes.py` → `// @generated src/node.rs` (phase 2). `NodeKind`+ranges, `is_*`/`as_*`, `visit_children`/`mark_lists`, `new`. **Phase 3:** generated `builder` module + `VisitorMut`/`TransformResult`/`Path`/`NodeField` + `visit_children_mut` (functional rebuild); read `Visitor` unchanged. **Phase 4:** generated `node_type_str` + `dump_children`, hand-written `src/dump.rs` (`ESTreeJSONDumper`); 9 `tests/dump_golden.rs`. | only in `context.rs` (scoped) |

**Lexer modules** (`rust/crates/parser/src/`): `token_kinds.rs` (TokenKind from `TokenKinds.def`),
`number.rs` (scanNumber primitives), `html_entities.rs` (generated), `utf8.rs` (UTF-8↔16 codec),
`cursor.rs` (the `*const u8` cursor — decision B, the primary parser `unsafe`; two additional statement-scoped `#[allow(unsafe_code)]` at the `alloc_scope` call sites in `functions.rs`/`pre_lazy.rs` are also sanctioned), `token.rs`
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
cargo test  --manifest-path rust/Cargo.toml            # whole workspace (725 tests / 42 suites as of Sema S3)
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
- **`getAllocator` has no Rust analog** for the lexer (no bump allocator for decoded strings — the one remaining lexer surface gap). The `AllocationScope` discipline IS ported for the PreParse passes: `ast::AllocationScope` + `support::Deque::truncate`/`iter_from` provide arena-truncate semantics at the two C++ scope sites (cpp:516-560, cpp:7523); see `specs/2026-07-15-preparse-scoped-reclamation-design.md`.

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
