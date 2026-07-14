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
  `hermes-atom-table` / `hermes-unicode` (support crates). `command_line` +
  `comparison` stay `publish = false`.
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
  `hermes-support` → `hermes-ast` → `hermes-parser`.
