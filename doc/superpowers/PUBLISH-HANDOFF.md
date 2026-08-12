# Session Handoff — Publishing the Rust `hermes-*` crates

Hand this to a new session focused **only** on publishing the Rust front-end to
crates.io. It references the authoritative files (read them; don't trust this
summary over them).

> **Date:** 2026-07-14. **Branch:** `rust1` (base is the `rust` impl branch).
> All publication prep is committed; working tree is clean apart from gitignored
> scratch (`clean_lex.js`, `.superpowers/`). Commit directly to `rust1`; **never
> open a PR or merge** (project rule). Execute subagent-driven (user preference).

## Read first (in order)

1. `doc/superpowers/plans/2026-06-19-rust-parser-publication-readiness.md` — THE plan.
2. `doc/superpowers/specs/2026-06-19-rust-parser-publication-design.md` — locked decisions.
3. Memory `rust-parser-publication-plan.md` (loads via MEMORY.md) — decision summary.
4. `CLAUDE.md` + memory `rust_port_conventions.md`, `always-build-with-clang.md`,
   `prefer-subagent-driven-execution.md`, `dont-pronounce-on-hermes-internals.md`.

## Status

**Phase A — DONE** (committed): crate metadata, `rust/LICENSE` + `rust/NOTICE`,
`rust/README.md` + `ARCHITECTURE.md` + `CHANGELOG.md` + `CONTRIBUTING.md`,
comparison harness (`rust/crates/comparison/`, excluded from the workspace) +
`FEATURE-MATRIX.md`, CI (`.github/workflows/rust*.yml`), blog draft
(`doc/superpowers/blog/2026-06-19-rust-hermes-parser-DRAFT.md`).

**Deferred to launch (Task 10 runbook + Tasks 3,4,5,6):** the `hermes-*` package
rename, `#![warn(missing_docs)]` API docs, `examples/`, moving the differential/CLI
bins into an unpublished `tools` crate (removes the `command_line` publish blocker),
`cargo publish --dry-run`, name reservation, publish. These were deferred to run
**after the `rust` implementation branch merges**, to avoid churn conflicts.

## Locked decisions (don't relitigate)

- Independent **MIT** crate, published **in-place from `rust/`** in the `tmikov/hermes`
  fork. No separate repo. Upstream merge is orthogonal.
- Family: **`hermes-parser` + `hermes-ast`** (stable public API) + `hermes-support` /
  `hermes-atom-table` / `hermes-unicode` / `hermes-command-line` (support crates).
  `tools` + `comparison` stay `publish = false`.
  (`command_line` was published as `hermes-command-line` on 2026-08-12 — scope
  extension; it is dependency-free and not in `hermes-parser`'s closure.
  `sema` was published as `hermes-sema` the same day — same scope extension,
  on the grounds that without it the port has no full front-end
  functionality; its `sema-dump` bin moved into `tools`.)
- Provenance wording (verbatim, everywhere): "A Rust port of the Hermes front-end by
  Tzvetan Mikov, the architect of Hermes. Not an official Meta project and not
  supported by Meta." Do **not** emphasize the word "unofficial."
- Support level (verbatim): "Issues and PRs are welcome and addressed as time
  permits. There is no SLA."
- **Reserve the `hermes-*` names early** (placeholder releases). CI differential runs
  nightly, not per-push.
- **Lib-naming = Option B:** publish with `use hermes_parser::` (full lib-name rename +
  the ~49-file `use parser::`→`use hermes_parser::` sweep), done AFTER the impl merges.
  See Task 10 Step 1.
- Prepare-now/publish-later: the 0.x-vs-1.0 launch version and blog venue are decided
  at launch.

## ⚠️ MUST reconcile before publishing any perf claim

> **SUPERSEDED by user decision 2026-08-12 (later the same day): public docs
> and READMEs carry NO performance mentions at all.** The reconciled perf note
> below was subsequently REMOVED from `rust/README.md`, and FEATURE-MATRIX's
> perf section was replaced with a "not published at this time" stub, after
> the measurement was found too noisy to publish (±30% session-to-session
> swing on the C++ react number: 149.9 → 113.1 MiB/s across sessions;
> full-lifecycle timing compresses ratios; the "1.3×" SWC floor didn't
> reproduce). Internal data stays in `BENCH-RESULTS.md` + the 06-30
> investigation doc; the blog draft carries a perf-claim gate banner. To
> publish perf claims later: pinned CPU + performance governor, PARSE-ONLY
> timing both sides, more fixtures, fresh runs.
>
> **RECONCILED 2026-08-12** (commits `f39215889`, `5e5cba67c`): the README perf
> note, BENCH-RESULTS.md, FEATURE-MATRIX.md and the blog draft were rewritten
> against a re-measured **Clang-built** C++ baseline. The old GCC numbers are
> marked superseded. Two June claims did not survive re-measurement and were
> withdrawn: the port is NOT faster than C++ Hermes anywhere (it reaches 83–85%
> on small/medium fixtures, 61% on the 8.7 MB typescript fixture), and the
> *port* does not beat SWC on every fixture (ahead on jquery/three.min, ~2%
> behind on react/typescript; the *C++ front-end* still beats SWC 1.3–1.9×).
> The section below is kept for the policy rationale.

The publication perf artifacts (`rust/README.md` perf note, `FEATURE-MATRIX.md`,
`rust/crates/comparison/BENCH-RESULTS.md`, the blog draft) **predate** the deep perf
investigation and do not reflect its conclusions. Before shipping perf numbers:

- The fair comparison is **parse + binding/semantic**, where OXC is ~**1.3–1.7×**
  faster — NOT the ~2× a naive parse-vs-parse shows. Parse-vs-parse is unfair: it
  penalizes the port for interning + number parsing that OXC defers to `oxc_semantic`.
- **The Rust port cannot be fairly benchmarked against OXC until a Sema/binding pass
  is ported to Rust.** Until then, lead perf with the genuinely favorable, defensible
  fact: **Hermes beats SWC by 1.3–1.9×** on every workload.
- Full detail + methodology (and the "always build C++ with Clang, not GCC" lesson):
  `doc/superpowers/2026-06-30-hermes-vs-oxc-parser-perf.md`.

If the initial crate ships without perf claims, this is moot — but do not publish a
raw OXC parse-vs-parse number; it makes the port look ~2× slower for doing more work.

## Validate / build

- Rust workspace: `cargo test --manifest-path rust/Cargo.toml` (comparison crate is
  excluded; build it via `--manifest-path rust/crates/comparison/Cargo.toml`).
- C++ (only if touching the differential/tools): configure with Clang —
  `cmake -B cmake-build-release -G Ninja -DCMAKE_BUILD_TYPE=Release
  -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++`.
- Publish dependency order (Task 10): `hermes-unicode` → `hermes-atom-table` →
  `hermes-support` → `hermes-ast` → `hermes-parser`, with `hermes-sema` anywhere
  after `hermes-ast`. `hermes-command-line` has no dependencies, so its position
  is free.

## Launch runbook (as of 2026-08-12 — Tasks 3,4,5,6,10 complete, final review APPROVED)

All automated prep is done: crates renamed `hermes-*` @ 0.1.0, API documented
(`missing_docs` clean), `parse()` façade + examples, bins in unpublished
`tools`, dry-run 6/6, perf claims reconciled. `hermes-sema` joined the set on
2026-08-12 (scope extension) — re-run the dry run for 7/7 before launching.
Only the manual, irreversible steps remain:

1. **Skip the placeholder name reservation** (plan Step 5) unless launch is
   weeks away — the real 0.1.0 publish IS the reservation, and placeholders
   add seven extra irreversible publishes plus junk 0.0.0 version rows.
2. **Publish with ONE multi-package invocation** (the plan's per-crate loop
   provably fails: versioned path deps resolve against the registry):
   ```bash
   cargo login <token>
   cargo publish --manifest-path rust/Cargo.toml \
     -p hermes-unicode -p hermes-atom-table -p hermes-support \
     -p hermes-ast -p hermes-parser -p hermes-sema -p hermes-command-line
   ```
   Every crate to publish must be named explicitly — an omitted `-p` is
   silently skipped, not an error.
   cargo stages them in dependency order and waits for index propagation.
   Verify each on crates.io and that docs.rs builds parser/ast.
3. **Post-publish checklist:** re-point the crate READMEs' pinned
   `blob/rust1/` links if the branch is renamed/merged (needs a patch release
   — decide branch fate first if that matters); trigger
   `rust-differential-nightly.yml` once via `workflow_dispatch` for its first
   observed green; sweep `doc/superpowers/{SESSION-HANDOFF,RustPortRoadmap}.md`
   `-p parser` spellings when `rust1` merges into `rust`; human editorial pass
   on the blog draft before any announcement.
