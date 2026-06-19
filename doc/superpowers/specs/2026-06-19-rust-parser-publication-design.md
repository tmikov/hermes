# Publishing the Hermes Rust front-end — strategy & design

> **Date:** 2026-06-19 · **Author:** Tzvetan Mikov · **Status:** approved design,
> pre-execution. This is a *strategy* spec (publication, documentation, comparison),
> not a component-implementation spec. The implementation itself continues on its own
> workstream in the `rust` branch.

## 1. Goal

Publish the Rust port of the Hermes front-end (lexer + parser + AST) as a **usable,
MIT-licensed family of crates on crates.io**, with first-class documentation and an
honest comparison against the other Rust JavaScript parsers. The work is finishing in
parallel; this spec covers everything that can be **prepared now** so that launch is a
button-press once the implementation lands.

### Decisions locked during brainstorming

| Question | Decision |
|---|---|
| Primary goal | A **usable open-source crate** (leads over article/benchmark/internal-docs). |
| Ownership / license | **Independent, MIT.** Hermes credited as the source. |
| Source home | **In-place in the author's Hermes fork** (`tmikov/hermes`), branch `rust`, published from the `rust/` subdirectory. No separate repo. Whether it is ever merged into `facebook/hermes` is orthogonal and does not block publication. |
| Timing | **Prepare now, publish later.** 0.x-vs-1.0 decided at launch. |
| Comparison | **Both, feature-led**: feature/correctness matrix is the headline; perf is a secondary, honestly-caveated section. |
| Naming | **`hermes-*` family.** |
| Provenance framing | Lead on **authorship**, not a disclaimer: "A Rust port of the Hermes front-end by Tzvetan Mikov, the architect of Hermes. **Not an official Meta project and not supported by Meta.**" Do **not** emphasize the word "unofficial." |
| Blog post | Wanted. Outline **all three candidate angles**; choose the lead at drafting time once comparison data exists. |
| Support level | **Best-effort.** Issues and PRs welcome, addressed as time permits, no SLA. Stated in `README`/`CONTRIBUTING`. |
| Name reservation | **Reserve early (definite step).** Claim the `hermes-*` names on crates.io with placeholder releases before launch. |
| CI / differential | **Differential in nightly/cached CI.** Rust workspace tests on every push; the byte-for-byte differential gate runs on a nightly schedule (or against a cached `hermesc` build artifact). |

### What makes this project distinctive (the positioning thesis)

1. **A faithful 1:1 port of a *production* C++ parser** (Hermes / `hermesc`) — not a
   clean-room Rust-first design like SWC/OXC/Biome.
2. **Byte-for-byte differential testing** against the C++ reference
   (`hermesc -dump-ast`) — a conformance-rigor claim none of the alternatives make.
3. **Full Flow type grammar** — rare-to-unique among Rust parsers (SWC dropped Flow;
   OXC and Biome never had it).
4. **ESTree-compatible AST + JSON output** matching Hermes exactly.

The publication, docs, and comparison all lead with these four.

## 2. Crate family & structure (Section A)

The Rust workspace has six members: `support`, `atom_table`, `unicode`, `parser`,
`ast`, `command_line`. crates.io forbids a published crate from depending on
unpublished/path-only crates, so publishing `hermes-parser` forces publishing its whole
dependency closure. Adopt the SWC/OXC convention of a prefixed family:

| Current crate | Published name | Role | Public API stability |
|---|---|---|---|
| `parser` | `hermes-parser` | **Headline.** Lexer + parser + JSON parser | **Stable public surface** |
| `ast` | `hermes-ast` | ESTree node set + JSON dumper | **Stable public surface** |
| `support` | `hermes-support` | `SourceErrorManager`, diagnostics, JSON emitter | Support crate |
| `atom_table` | `hermes-atom-table` | String interner (copied from juno) | Support crate |
| `unicode` | `hermes-unicode` | Unicode property tables | Support crate |
| `command_line` | *(unpublished)* | Differential / CLI binaries (e.g. `ast-dump`) | `publish = false` |

`hermes-parser` and `hermes-ast` are documented as the **stable public surface**; the
three support crates are published only because the dependency closure requires it and
are labelled "support crates — depend on them directly at your own risk." This mirrors
how SWC ships `swc_*` and OXC ships `oxc_*`.

**Decision:** keep all five library crates (do *not* fold the support crates into
`hermes-parser`) — faithful to the existing structure, least churn, matches ecosystem
norms.

## 3. Documentation surface (Section B)

Three layers:

1. **In-crate (rustdoc → docs.rs):** crate-level docs on `hermes-parser` and
   `hermes-ast` (what it is, the Hermes lineage, the JS/Flow/TS/JSX support matrix, a
   short "parse → AST → JSON" example), plus doc comments on every public item
   (`#![warn(missing_docs)]` on the two public crates).
2. **Repo-level (under `rust/`):** `README.md` (positioning + quickstart + feature
   matrix + provenance banner), `CHANGELOG.md`, `CONTRIBUTING.md`, MIT `LICENSE`, a
   `NOTICE`/attribution file pointing at Hermes (and juno for the copied
   `atom_table`/`unicode`), and an `examples/` directory (parse a file, dump ESTree
   JSON, walk the AST).
3. **`ARCHITECTURE.md`:** distilled from the existing `doc/superpowers/specs/*` and
   `RustPortRoadmap.md` — the GC-arena AST, the faithful-port philosophy
   (templates→generics, RAII→guards), and the **differential-testing methodology** as a
   prominent, dedicated section. This is the credibility anchor and doubles as raw
   material for the blog post.

## 4. Comparison (Section C) — feature-led, honestly-caveated perf

**Subjects benchmarked/matrixed:** `hermes-parser` vs **SWC**, **OXC**, **Biome**,
**Boa**. Mentioned-but-not-benchmarked: `jsparagus`, `tree-sitter`, RSLint.

### Feature/correctness matrix (the headline)

Rows = the parsers; columns:
- ECMAScript coverage (+ stage-3 proposals)
- JSX
- TypeScript
- **Flow** ← the differentiator (expected: only `hermes-parser` + tree-sitter)
- AST model: **ESTree-compatible?** / own AST / lossless CST
- Error recovery / tolerant parsing
- Comment + source-location preservation
- Allocator model (our GC arena vs OXC bump vs Biome rowan-CST)
- **Conformance methodology** — plant the flag here: *byte-for-byte differential vs a
  production C++ engine.*
- Maturity / ecosystem / real-world usage

All cells must be **verified against each project's current source/docs**, not asserted
from memory — capabilities change release to release.

### Performance section (secondary, caveated)

Framing stated up front: this is a **fidelity-first** port with a GC-arena (mark-sweep)
AST, not a speed-first bump-allocated design; it may not beat OXC, and that is an
acceptable, honest outcome.

- **Methodology:** `criterion` over a shared real-world corpus (e.g. React, the
  TypeScript compiler, jQuery, a large minified bundle). Each parser builds **its own
  native AST**; report throughput in MB/s plus parse time. Warm runs, pinned versions
  recorded.
- **Apples-to-oranges box:** explicitly explain why a direct number is imperfect (OXC
  bump AST vs our GC AST vs Biome lossless CST do different amounts of work).
- **Second, fairer axis:** parse → **ESTree JSON**, where the dumper gives a more
  comparable end-to-end task for tools that can emit ESTree.
- **Reality check (a measurement task, not a claim):** we do **not** yet know where we
  land vs OXC. The harness must produce the numbers before any perf statement is written.

## 5. "Prepare now" readiness roadmap (Section D)

All location-agnostic; all doable in-tree now. These survive any later repo extraction.

1. **Crate metadata** on each published `Cargo.toml`: `description`, `license = "MIT"`,
   `repository`, `keywords`, `categories`, `readme`, `authors`; `publish = false` on
   `command_line`.
2. **License + attribution:** MIT `LICENSE` at `rust/`; `NOTICE` crediting Hermes and
   juno; verify the copied `atom_table`/`unicode` retain their original upstream
   attribution.
3. **Public-API audit:** decide the exact `pub` surface of `hermes-parser`/`hermes-ast`,
   add `#![warn(missing_docs)]`, document every public item.
4. **rustdoc + `examples/`** (parse file, dump ESTree JSON, walk AST).
5. **`README` + `ARCHITECTURE.md` + `CHANGELOG` + `CONTRIBUTING`** distilled from the
   existing specs/roadmap, with the provenance banner.
6. **CI:** GitHub Actions running the Rust workspace tests on every push (fast). The
   byte-for-byte differential gate runs **nightly** (or against a cached `hermesc` build
   artifact) so the engine build doesn't slow every push while the headline conformance
   claim is still enforced continuously.
7. **Comparison harness:** a `publish = false` bench crate pulling SWC/OXC/Biome/Boa as
   dev-deps over the shared corpus.
8. **Publish dry-run + name reservation:** `cargo publish --dry-run`; record the
   dependency-order publish list (support crates first, then `hermes-ast`, then
   `hermes-parser`). **Reserve the `hermes-*` names early** by publishing placeholder
   releases (a definite step, not optional), so the family names cannot be taken before
   launch.
9. **Blog post draft** (Section 6).

## 6. Blog post (Section E)

Wanted. Capture **all three candidate angles** as outlines in the draft; pick the lead
at drafting time once the comparison data is in (the others become supporting sections).

- **Angle 1 — Faithful-port methodology.** "How we ported a production C++ parser to
  Rust byte-for-byte." Centerpiece: the differential oracle, templates→generics,
  RAII→guards, the discipline that keeps a port honest.
- **Angle 2 — The only complete Flow parser in Rust.** Lead on the feature gap (SWC
  dropped Flow; OXC/Biome never had it), why Flow is hard, how a faithful Hermes port
  fills it.
- **Angle 3 — AI/subagent-driven port.** The workflow itself: porting tens of KLOC of
  C++ via subagent-driven development gated by a byte-for-byte differential oracle plus
  two-stage review. Likely the highest-reach angle.

Venue is TBD (personal blog / Rust community / engineering blog) — decided at draft.

## 7. Risks

- **Naming / brand.** `hermes-*` collides conceptually with Meta's official npm
  `hermes-parser` (the WASM Hermes parser). Mitigation: the authorship-led provenance
  banner ("by the architect of Hermes; not an official Meta project / not
  supported by Meta"). Residual risk accepted knowingly.
- **Perf may underwhelm** vs OXC. Mitigation: feature-led framing; honest caveats; the
  fidelity story is the value proposition, not raw speed.
- **Maintenance commitment.** Once on crates.io, expect issues and semver obligations.
  Mitigation: advertise a **best-effort** support level in `README`/`CONTRIBUTING`
  (issues/PRs welcome, addressed as time permits, no SLA) so expectations are set
  honestly from day one.
- **Copied-code licensing.** `atom_table`/`unicode` are copied from juno; preserve
  upstream license + credit in `NOTICE`.

## 8. Out of scope

- The parser implementation itself (separate `rust`-branch workstream).
- The 0.x-vs-1.0 launch decision (made at launch, per the timing decision).
- Any actual `cargo publish` (this spec stops at dry-run readiness).
- A separate dedicated repository (explicitly decided against; revisitable later since
  crates.io identity is independent of repo location).

## 9. Execution

This strategy spec feeds a `writing-plans` implementation plan covering the Section 5
readiness roadmap (metadata, license/attribution, API audit + docs, examples, repo docs,
CI, comparison harness, publish dry-run, blog draft). Each item is independently
verifiable; the comparison harness and the docs are the two largest sub-efforts.
