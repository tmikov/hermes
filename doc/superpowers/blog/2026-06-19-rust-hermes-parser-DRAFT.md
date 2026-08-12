# Blog Post Draft — Three Candidate Angles

**Date:** 2026-06-19
**Author:** Tzvetan Mikov
**Status:** DRAFT — outlines only; lead angle chosen at drafting time once final
comparison data is confirmed.
**Venue:** TBD (personal blog / Rust community / engineering blog)

---

## How to Use This Draft

Three angle outlines are presented below. Each stands on its own; pick one as the lead
article and fold the others into supporting sections. All three draw from the shared
data appendix at the bottom. Do not write full prose until the lead is chosen.

---

## Angle 1 — Faithful-port Methodology

**Working title:** "How We Ported a Production C++ Parser to Rust, Byte-for-Byte"

**Hook (1-2 paragraphs):**
Hermes is the JavaScript engine running React Native on hundreds of millions of
Android and iOS devices. Its C++ front-end — lexer, parser, AST — is roughly 20 000 lines [verify before publishing]
of battle-tested, production-hardened code. This is the story of translating
it into Rust without inventing a new design, changing the algorithm, or drifting from
the original, and how a byte-for-byte differential oracle against the real
`hermesc` binary enforced that discipline every step of the way.

**Section beats:**

1. **Why "faithful port" and not "inspired by"**
   - The goal: a Rust crate that is Hermes, not a Rust parser that resembles Hermes.
   - What fidelity buys: the correctness story is `hermesc` correctness; no new bugs
     introduced by redesign; traceable correspondence to the C++ source for every
     function.
   - Trade-offs accepted: not a clean-slate Rust-idiomatic design; the GC-arena AST
     and cursor choices flow from the C++ model, not from Rust community norms.

2. **The differential oracle: `hermesc -dump-ast`**
   - Oracle command: `hermesc -dump-ast -dump-source-location=both` produces raw
     parse AST as JSON before semantic analysis — no separate tool needed.
   - Gate command (see Appendix D). CI fails if a single byte differs.
   - 166 corpus files across 8 dialect corpora (plain JS, Flow, component, records,
     match, TypeScript, JSX, JSX+Flow).
     What the differential caught that code review did not: facts/figures needed
     here — find a concrete example from the port history where the differential caught
     a real divergence that review missed.
   - What review caught that the differential could not: correctness of the
     templates-as-generics mapping (the differential cannot detect if a compile-time
     template was silently collapsed into a runtime parameter).

3. **C++ templates → Rust generics**
   - Rule: any `template <` in the C++ source must remain a generic in Rust.
   - Three concrete examples: `lookahead1/2<const REQUIRE_NO_NEWLINE: bool>`,
     `IdMode` marker-trait ZSTs for `JsMode`/`JsxMode`/`FlowMode`, `scan_string<const JSX: bool>`.
   - Why a runtime bool is not acceptable even though the differential test cannot
     detect it: structural fidelity is the non-negotiable constraint.

4. **C++ RAII guards → explicit guard types**
   - The borrow-checker prevents a `&mut`-holding guard and a method call through
     `&mut self` from coexisting.
   - Mapping: `SaveAndRestore<bool>` → `Rc<Cell<bool>>` + `ParamFlagGuard` with
     `Drop`; `SaveFunctionState` → explicit save/restore wrappers;
     `SaveAndSuppressMessages` → explicit begin/end APIs on `SourceErrorManager`.
   - The feature is fully implemented, just without syntactic sugar.

5. **The `*const u8` cursor decision**
   - The only `unsafe` in the `hermes-parser` crate: a raw pointer cursor in
     `src/cursor.rs`, confined to that module.
   - Rationale: lexer speed depends on in-register pointer arithmetic; the
     `NullTerminatedBuf` trailing NUL makes one-byte lookahead unconditionally
     in-bounds; encapsulation makes the unsafety reviewable.
   - Contrast with the rest of the codebase: `hermes-support` and `hermes-unicode`
     forbid `unsafe`
     entirely via `[lints.rust] unsafe_code = "forbid"`.

6. **What fidelity costs and what it buys**
   - Costs: GC-arena AST and atom interning produce a 1.7–2.4× parse-only gap vs
     OXC (1.3–1.7× on the equal-work parse+binding comparison) — but
     this gap exists between OXC and C++ Hermes too; it is a Hermes design constraint,
     not a port regression (see Appendix B). On small and medium files the port runs
     at 83–85% of C++ Hermes. On the 8.7 MB typescript fixture the port reaches 61% of
     C++ Hermes due to AST node footprint at scale — a candidate optimization (boxing
     large variants) that has not yet been validated. Fail-fast error handling mirrors
     C++ Hermes, not IDE-friendly error recovery.
   - Buys: the correctness claim is the C++ production engine, not a test suite. The
     port inherits 10+ years of Hermes bug-fixes. The differential gate is a
     continuous conformance certificate.

**Facts/figures this angle needs from the appendix:**
- The hook's "~20,000 lines" figure must be verified before publishing.
- Differential gate command and corpus counts (Appendix D).
- A concrete differential catch (not yet recorded — needs one real example from port
  history; find in commit log or session notes).
- The three generics examples (already in ARCHITECTURE.md).
- The `*const u8` cursor note (ARCHITECTURE.md).
- Directional perf numbers (Appendix B) for the "what it costs" beat.

---

## Angle 2 — The Only Complete Flow Parser in Rust

**Working title:** "Flow Types in Rust: Filling the Gap Every Other Parser Left Open"

**Hook (1-2 paragraphs):**
If you write Flow-typed JavaScript, your options in the Rust tooling ecosystem are
grim. SWC's Flow support is shallow (type-stripping via `Syntax::Flow`); OXC and Biome have none. The
`hermes-parser` crate is, as far as is known, the only Rust parser with a complete
Flow type grammar — not a stripping pass, not an approximation, but the full language
as Facebook ships it. This post explains why Flow is harder to parse than it looks,
what "complete" actually means in the corpus, and how a byte-for-byte differential
against the Hermes engine proves it.

**NOTE:** The claim is "the only *complete* Flow parser in Rust", not "the only Flow
parser." SWC has partial Flow support (`Syntax::Flow`, shallower coverage, focused on
type-stripping). The distinction must be precise in the post.

**Section beats:**

1. **Why Flow is hard to parse**
   - Flow extends JavaScript with a type grammar that is structurally unlike
     TypeScript: opaque types, `component` and `hook` declarations, `record` and
     `tuple` types, `match` expressions, the `declare` family.
   - Some Flow syntax is ambiguous with JavaScript expression grammar (e.g.,
     type-cast `(x: T)` vs parenthesized expression). The parser must track the
     grammar mode explicitly.
   - The `component`/`hook`/`record`/`match` syntax is under active development
     and does not appear in any spec other than the Hermes source.

2. **What "complete" means here**
   - The full `JSParserImpl-flow.cpp` surface (the C++ file that is the definitive
     spec): type annotations, conditional/union/intersection types, function and
     object types, generics and type parameters, predicates (`%checks`, `%type`),
     `type`/`opaque type`/`interface`, `declare` family (`declare var`, `declare
     function`, `declare class`, `declare module`, `declare export`), `enum`,
     `import type`/`export type`, `as`/`as const` casts, typed arrows, `component`,
     `hook`, `record`, `match`.
   - All behind a `parse_flow: bool` flag, off by default.

3. **The competitive landscape**
   - SWC: `Syntax::Flow` opt-in, `components`/`enums`/`patternMatching` options;
     targets type-stripping for compilation; coverage is shallower than this port.
     (Verified against swc source, 2026-06-19; see Appendix C.)
   - OXC: no Flow support; explicitly recommends Prettier's Hermes plugin for Flow
     users.
   - Biome: no Flow support.
   - Boa: no Flow support.
   - tree-sitter-javascript: has some Flow grammar; not a Rust library crate.

4. **The corpora: how "complete" is verified**
   - Five Flow-specific corpus directories (see Appendix D): `parser_corpus_flow/`
     (42 files), `parser_corpus_flow_component/` (8), `parser_corpus_flow_records/`
     (5), `parser_corpus_flow_match/` (7) — 62 Flow corpus files total.
   - Byte-for-byte differential against `hermesc -parse-flow` (and dialect flags).
     Any failure is a bug.

5. **Why this port and not a standalone effort**
   - The Hermes C++ source is the authoritative Flow spec; a faithful port of that
     source is the only way to track the language as it evolves.
   - The differential oracle makes the completeness claim verifiable rather than
     asserted.

**Facts/figures this angle needs from the appendix:**
- SWC/OXC/Biome Flow rows from the feature matrix (Appendix C).
- Flow corpus counts and dialect flags (Appendix D).
- The full list of Flow constructs implemented (README.md "only complete Flow parser"
  paragraph — verify list is still accurate at drafting time).
- The competitive framing from FEATURE-MATRIX.md notes on SWC.

---

## Angle 3 — AI/Subagent-Driven Port

**Working title:** "Porting 20 000 Lines of C++ to Rust with Subagents and a
Byte-for-Byte Differential Oracle"

**Hook (1-2 paragraphs):**
The Hermes JavaScript parser is roughly 20 000 lines [verify before publishing] of C++. Porting it to Rust
manually — function by function, with a senior engineer reviewing each change —
would take many months. This port was done differently: with subagent-driven
development, where each subagent takes a bounded implementation task and a
two-stage human review follows every phase, all gated by a byte-for-byte differential
oracle that catches any behavioral drift. The result is a complete, production-quality
port that has already outlasted most hand-ported projects by the only metric that
matters: `diff <(hermesc -dump-ast ...) <(ast-dump ...)` exits zero on all 166
corpus files.

**NOTE:** This angle has the highest reach (developer-workflow, AI tooling, Rust
community audiences simultaneously) and is likely the strongest choice once the
project is published. It also carries the most novelty risk — the workflow story is
only compelling if the outcome is credible. Lead with the outcome (byte-for-byte
differential, complete Flow grammar), then explain the process.

**Section beats:**

1. **The problem: scale vs. quality**
   - ~16 900 lines of JS parser C++ [verify before publishing] (JSParserImpl-*.cpp), plus ~3 700 lines of
     lexer (JSLexer) [verify before publishing]. Total ported: tens of KLOC.
   - Naive approach: hand-translate line-by-line. Problem: tedious, error-prone, hard
     to review in bulk.
   - The insight: subagents can handle bounded porting tasks (one module, one
     grammar section, one data structure) if and only if there is a hard oracle to
     catch drift.

2. **The oracle as the enabling constraint**
   - The differential gate (Appendix D) is what makes subagent-driven porting safe.
     Without a byte-for-byte oracle, the only check is human review, which does not
     scale.
   - Every subagent task: port a bounded section, pass the differential gate, pass
     two-stage human review (first-pass correctness + second-pass style/fidelity).
     No task is declared done until the gate is green.
   - The gate command is deterministic and hermetic: same inputs, same expected
     output. A subagent cannot fake a passing gate.

3. **Task decomposition and capstones**
   - How the work was sliced: by grammar section (expressions, statements, functions,
     classes, modules, Flow declarations, Flow types). Each section is a subagent task
     with a written spec and a defined exit criterion (differential + review).
   - Capstone tasks: after each major grammar area, a capstone subagent runs the full
     differential gate and writes a session-handoff document that is the ground truth
     for the next subagent.
   - Port conventions (templates-as-generics, RAII guards, default-args-are-spec) are
     written once and referenced in every task brief, so each subagent works from the
     same binding rules.

4. **Two-stage review: what it catches**
   - First stage: correctness — does the port match the C++ source at the function
     level? Are all `template <` specializations preserved as generics? Are default
     arguments looked up in the header?
   - Second stage: fidelity and style — are the Rust idioms appropriate? Is the
     `unsafe` surface minimal and contained?
   - What the differential catches that review misses: runtime behavioral divergences
     (a missed `lookahead1` with `RequireNoNewLine = true` that only fires in a
     specific token sequence).
   - What review catches that the differential misses: structural deviations (a
     template collapsed to a runtime parameter) that produce the same output on all
     corpus files but differ in correctness guarantees.

5. **Workflow lessons and limits**
   - What works well: bounded, specced tasks with a hard oracle; tasks that map one
     C++ file to one Rust module; tasks with a clear "done" criterion.
   - What is harder: tasks that require architectural judgment (e.g., the
     `*const u8` cursor decision, the GC-arena design); these are human decisions
     written into specs and handed to subagents as binding constraints, not decisions
     left to the subagent.
   - Scale: the workflow can sustain tens of KLOC of porting because each task is
     independently reviewable and the oracle catches regressions across the whole
     corpus.

6. **Outcome and the honest accounting**
   - JavaScript and Flow: complete and differential-tested.
   - TypeScript and JSX: complete and differential-tested (see Appendix A), each
     with its own corpus in the same gate.
   - Performance: directional numbers (Appendix B). The GC-arena design is a
     deliberate fidelity choice, not a performance optimization.
   - This is not a claim that subagent-driven development is always better than
     manual porting. It is a claim that a byte-for-byte oracle makes it tractable
     for a large, well-specified C++ codebase.

**Facts/figures this angle needs from the appendix:**
- LOC counts (JS parser: ~16 900 C++ lines; lexer: ~3 700; total ported). Verify
  against current source; these are approximations from ARCHITECTURE.md.
- Corpus totals: 166 files, 8 corpora (Appendix D).
- The gate command (Appendix D).
- The "what differential catches vs. what review catches" contrast — a concrete
  example from the port history would make this vivid. Find in commit log.
- Support matrix (Appendix A) for the honest accounting beat.

---

## Shared Facts and Data Appendix

*All three angles draw from this appendix. Verify all numbers at drafting time.*

---

### Appendix A — Support Matrix

| Language | Status |
|---|---|
| JavaScript / ECMAScript (ES2025+) | Complete |
| Flow type grammar | Complete (full grammar, all dialects, differential-tested) |
| TypeScript | Complete (P7), differential-tested |
| JSX | Complete (P8), differential-tested |

All four dialects now pass the byte-for-byte gate; the earlier "TS/JSX in
progress" framing in older drafts is obsolete.

---

### Appendix B — Directional Performance

> These numbers are directional only. Each parser does different amounts of work:
> different AST shapes, different interning strategies, arena vs. heap allocation,
> presence or absence of scope resolution during parse. A faster number does not mean
> a better parser for your use case.

Benchmarked with Criterion.rs (`opt-level = 3`, Linux x86-64) for Rust parsers, and
the **Clang-built** Release C++ `parse-bench` tool for the C++ Hermes baseline (a
bare `cmake -DCMAKE_BUILD_TYPE=Release` picks GCC on Ubuntu and understates C++
Hermes). Per-iteration fresh `Context`; FullParse/eager; median; same machine; one
process per (parser, fixture). Four plain-JS fixtures (TS/JSX not
exercised — in progress). Full methodology and trailing-error fairness guard in
`rust/crates/comparison/BENCH-RESULTS.md`. Re-measured 2026-08-12.

| Parser | react 107K | jquery 278K | three.min 654K | typescript 8.7M |
|---|---|---|---|---|
| hermes-parser (this port) | 95.6 MiB/s | 72.5 MiB/s | 42.1 MiB/s | 61.5 MiB/s |
| C++ Hermes (Clang, Release) | 113.1 MiB/s | 86.9 MiB/s | 49.9 MiB/s | 100.7 MiB/s |
| oxc_parser 0.137.0 | 192.9 MiB/s | 124.1 MiB/s | 75.6 MiB/s | 149.1 MiB/s |
| swc_ecma_parser 41.1.1 | 97.1 MiB/s | 70.5 MiB/s | 36.2 MiB/s | 62.4 MiB/s |
| boa_parser 0.21.1 | 12.1 MiB/s | 10.6 MiB/s | 4.7 MiB/s | 5.0 MiB/s |
| biome_js_parser 0.5.7 | not benchmarked (build failure — crates.io publish mismatch) | — | — | — |

**Key directional takeaways for the post:**

- **The Rust port tracks the C++ Hermes baseline.** It reaches 83–85% of
  Clang-built C++ Hermes on react, jquery and three.min. For a faithful port,
  landing in the same throughput class as the original engine is the goal — but
  the port does not beat it anywhere.
- **On the 8.7 MB typescript fixture the port reaches 61% of C++ Hermes**
  (61.5 vs 100.7 MiB/s). This is a real gap, and measurement surfaced a concrete
  explanation: every AST node is a uniform 128-byte `Node` enum (confirmed via
  `std::mem::size_of`). The typescript fixture produces ~904,000 nodes, totalling
  ~123 MiB of live AST — roughly 14× the source size. At that scale the working set
  far exceeds CPU cache and memory bandwidth becomes the bottleneck. The parser never
  runs a GC collection during parse (the arena is freed in one drop). Boxing the
  large `Node` variants to shrink the average node size is a candidate optimization —
  but boxing trades footprint for indirection, and the net effect on throughput must
  be measured. This is a follow-up for the maintainer, not a done fix.
- **OXC's lead is a design difference, not a port regression — and smaller than
  parse-vs-parse suggests.** Parse-only, OXC is 1.7–2.4× faster than this port.
  On the equal-work comparison (parse + binding/semantic), OXC leads C++ Hermes
  by 1.3–1.7×. OXC's bump allocator and zero-copy `Atom` type are structurally
  different from Hermes's atom interning and GC-arena AST; C++ Hermes carries the
  same gap vs OXC, and any faithful port inherits it. Against SWC the port is
  comparable: ahead on jquery and three.min, within ~2% on react and typescript.
- Boa is roughly 8× slower than this port; its parser performs scope resolution
  during parse, which the others defer.
- Biome's lossless CST does fundamentally different work; throughput comparison is
  not meaningful.

**Framing rule:** always present perf as secondary to correctness. The headline is
byte-for-byte agreement with the production C++ engine. Use the "apples-to-oranges"
caveat every time a number appears.

---

### Appendix C — Feature/Correctness Matrix Summary

Full matrix: `rust/crates/comparison/FEATURE-MATRIX.md`

Key cells relevant to the post:

| Feature | hermes-parser | SWC | OXC | Biome | Boa |
|---|---|---|---|---|---|
| ECMAScript coverage | Complete (ES2025+) | Complete | Complete | Complete | Complete |
| JSX | **Complete** | Complete | Complete | Complete | None |
| TypeScript | **Complete** | Complete (TS 5.x) | Complete (TS 5.x) | Complete | None |
| Flow | **Complete** (full grammar, differential-tested) | Partial (type-stripping focus; `Syntax::Flow` opt-in; shallower than this port) | None | None | None |
| AST model | GC-arena, ESTree-compatible | Own AST (ESTree-inspired, not compatible) | Bump-arena, ESTree-compatible | Lossless CST (rowan fork) | Own AST |
| Error recovery | Fail-fast (mirrors C++ Hermes) | Partial | Advanced | Fully tolerant | Limited |
| Conformance methodology | Byte-for-byte vs live hermesc binary | tc39/test262 | tc39/test262 + own suite | Own suite + test262 subset | test262 (~94%) |

Versions verified: `swc_ecma_parser 41.1.1`, `oxc_parser 0.137.0`,
`biome_js_parser 0.5.7`, `boa_parser 0.21.1`. Research date: 2026-06-19.

**Flow precision note:** SWC has `Syntax::Flow` with `components`, `enums`, and
`patternMatching` options. Its coverage targets type-stripping for compilation and is
shallower than this port. Do not describe SWC as having "no Flow support" — describe
it as "partial / type-stripping focus." OXC and Biome genuinely have no Flow support.

---

### Appendix D — Differential Testing Method

**Oracle:** `hermesc -dump-ast -dump-source-location=both` produces the raw parse AST
as ESTree JSON before semantic analysis. The Rust `ast-dump` binary (in the
unpublished `rust/crates/tools`) produces output in the identical format.

**Corpora:**

| Corpus directory | Flags | File count |
|---|---|---|
| `parser_corpus/` | (none — plain JS) | 77 |
| `parser_corpus_flow/` | `-parse-flow` | 42 |
| `parser_corpus_flow_component/` | `-parse-flow -Xparse-component-syntax` | 8 |
| `parser_corpus_flow_records/` | `-parse-flow -Xparse-flow-records` | 5 |
| `parser_corpus_flow_match/` | `-parse-flow -Xparse-flow-match` | 7 |
| `parser_corpus_ts/` | `-parse-ts` | 20 |
| `parser_corpus_jsx/` | `-parse-jsx` | 6 |
| `parser_corpus_jsx_flow/` | `-parse-jsx -parse-flow` | 1 |
| **Total** | | **166** |

All eight are live in the gate (`parser_differential` is 8/8). A ninth
directory, `parser_corpus_lazy/` (13 files), backs the pre-parse/lazy-parse
gate rather than the AST differential.

**Gate command:**

```bash
# Build the Rust ast-dump binary and the C++ hermesc oracle:
cargo build --manifest-path rust/Cargo.toml -p tools --bin ast-dump
cmake --build cmake-build-asan --target hermesc

# Run the differential gate (fails if the oracle binary is absent):
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml \
    -p hermes-parser --test parser_differential
```

Any single-byte difference between the Rust output and the `hermesc` output is a CI
failure. This gate runs on a nightly/cached schedule so the `hermesc` build does not
slow every push, while the conformance claim is enforced continuously.

---

### Appendix E — Provenance Statement

Use this text verbatim (or close to it) in the post:

> `hermes-parser` is a Rust port of the Hermes front-end by Tzvetan Mikov, the
> architect of Hermes. It is not an official Meta project and is not supported by
> Meta.

**Do not emphasize "unofficial."** Lead with the authorship. The credibility story
is that the port was written by the person who designed the original system.

---

### Appendix F — Crate Family

| Published crate | Role | Stability |
|---|---|---|
| `hermes-parser` | Lexer + parser + JSON parser | Stable public surface |
| `hermes-ast` | ESTree node set + JSON dumper | Stable public surface |
| `hermes-support` | SourceErrorManager, diagnostics, JSON emitter | Support crate — depend at your own risk |
| `hermes-atom-table` | String interner (adapted from juno) | Support crate |
| `hermes-unicode` | Unicode property tables | Support crate |

Not published: `command_line` (CLI binaries), `comparison` (benchmark harness).

---

*End of draft. Lead angle TBD pending final comparison data confirmation.*
