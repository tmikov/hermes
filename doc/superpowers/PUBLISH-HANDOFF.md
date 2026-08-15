# Session Handoff — Publishing the Rust `hermes-*` crates

Hand this to a new session focused **only** on publishing the Rust front-end to
crates.io. It references the authoritative files (read them; don't trust this
summary over them).

> **Date:** 2026-07-14, revised 2026-08-13. **Branch:** `rust` (the only
> branch; `rust1` was rebased into it and deleted 2026-08-13 — nothing had
> been published, so the split was no longer worth carrying).
> All publication prep is committed; working tree is clean apart from gitignored
> scratch (`clean_lex.js`, `.superpowers/`). Commit directly to `rust`; **never
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
- Family: **SEVEN crates.** `hermes-parser` + `hermes-ast` + `hermes-sema`
  carry the stable public API; `hermes-support` / `hermes-atom-table` /
  `hermes-unicode` / `hermes-command-line` are support crates. `tools` +
  `comparison` stay `publish = false`.
  (`command_line` was published as `hermes-command-line` on 2026-08-12 — scope
  extension; it is dependency-free and not in `hermes-parser`'s closure.
  `sema` was published as `hermes-sema` the same day — same scope extension,
  on the grounds that without it the port has no full front-end
  functionality; its `sema-dump` bin moved into `tools`. `hermes-sema`'s
  guarantee is partial: stable core (`resolve` façade, `resolve` module,
  `sem_context`, `ids`) + seven advanced / port-internal modules.)
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
- Publish dependency order (seven crates): `hermes-unicode` →
  `hermes-atom-table` → `hermes-command-line` → `hermes-support` →
  `hermes-ast` → `hermes-parser` → `hermes-sema`. `hermes-command-line` has
  no dependencies at all, so its position is free — it is listed third only
  to keep the leaf crates together. `hermes-sema` must come LAST: its
  `resolve` façade takes a `hermes_parser::ParsedJS`, so it depends on
  `hermes-parser` (a normal dependency, not a dev-dependency).

## 0.1.0 IS PUBLISHED (2026-08-12). Next release: 0.1.1 — runbook below

All seven crates went live on crates.io on 2026-08-12 at **0.1.0**. The
"Launch runbook" further down is kept for its rationale, but it is history —
follow this section for 0.1.1 and after.

### What the 0.1.0 publish actually cost

- **The new-crate rate limit is the only thing that slowed it down.** crates.io
  caps *brand-new crate names* (at the time: a small burst, then one new name
  per ~10 minutes). Publishing seven names cost **two ten-minute waits**
  mid-run. cargo reported it as a 429 and the multi-package invocation had to
  be re-issued for the crates that had not landed yet.
- **That limit does NOT apply to a new version of an existing crate.** Updates
  are governed by a far more generous limit. Every name in the family now
  exists, so **0.1.1 (and every later release) should go through in a single
  invocation with no waiting.** Do not pre-emptively split the command up or
  add sleeps; if a 429 does appear, it will be the ordinary publish limit and
  cargo prints the retry-after.

### 0.1.1 — the partial-family release (prepared 2026-08-15)

**Only four crates move.** Their packaged content was diffed against the real
published 0.1.0 tarballs (`~/.cargo/registry/src/index.crates.io-*/`), not
against a guess: `hermes-atom-table` (1 file), `hermes-ast` (3),
`hermes-parser` (4), `hermes-sema` (4) changed; `hermes-unicode`,
`hermes-support` and `hermes-command-line` are byte-identical and **stay at
0.1.0**. Republishing unchanged content as a new version is noise, and a user
who sees `hermes-unicode 0.1.0` correctly infers nothing changed.

**Do not name the unchanged crates.** Verified with cargo 1.96.0: naming them
is *not* a skip. `cargo publish --dry-run` with all seven prints

```
warning: crate hermes-support@0.1.0 already exists on crates.io index
warning: crate hermes-unicode@0.1.0 already exists on crates.io index
warning: crate hermes-command-line@0.1.0 already exists on crates.io index
```

and then still packages and reaches the `Uploading` step for all seven. On a
real run the duplicate version is rejected by crates.io, not by cargo — and
because cargo uploads in dependency order and each upload is irreversible,
a rejection can abort the run with part of the release already live. (The
exact server-side message was not observed here; only cargo's local
behaviour above was.) Use that warning list as a pre-flight check: it names
exactly the crates that must be dropped from the command.

So the command is the four, in dependency order:

```bash
cargo login <token>
cargo publish --manifest-path rust/Cargo.toml \
  -p hermes-atom-table -p hermes-ast -p hermes-parser -p hermes-sema
```

The "all SEVEN must be named" rule below applies *within* a release: an
omitted `-p` is silently skipped, so name every crate that is bumped.

**Tag convention: one tag per release, `hermes-crates-vX.Y.Z`, cut at the
commit you publish from, pushed to `private` before publishing.** The crate
READMEs link to GitHub through the tag, and a published README cannot be
edited without another release — so the links must already be correct when the
.crate is uploaded. For 0.1.1 the tag is **`hermes-crates-v0.1.1`** and all
ten README links were repointed to it in the release commit (the tag therefore
points at the commit that contains the links naming it — self-consistent).

- Never delete or move an old release tag: `hermes-crates-v0.1.0` must keep
  resolving forever, because the frozen 0.1.0 READMEs on crates.io point at
  it. That includes the three crates staying at 0.1.0 — their live pages keep
  their v0.1.0 links, while the in-repo READMEs are already repointed to
  v0.1.1 for whenever those crates next ship.
- Push the tag, not just the branch: `git push private hermes-crates-v0.1.1`.

**Three things in the READMEs are hand-maintained and go stale silently.**
This bit 0.1.1 — all three were nearly shipped wrong, and a shipped README
cannot be corrected without another release. Check every one of them against
the manifests before tagging:

1. `**Version:** X.Y.Z — API docs at …` at the top of every crate README.
   It must equal that crate's own manifest version — so in a partial-family
   release the unbumped crates keep the OLD number. A one-liner that must
   print nothing:
   ```bash
   for c in atom_table ast parser sema unicode support command_line; do
     m=$(grep -m1 '^version = ' rust/crates/$c/Cargo.toml | grep -o '0\.[0-9]*\.[0-9]*')
     r=$(grep -m1 '^\*\*Version:\*\*' rust/crates/$c/README.md | grep -o '0\.[0-9]*\.[0-9]*')
     [ "$m" = "$r" ] || echo "MISMATCH $c: manifest=$m readme=$r"
   done
   ```
2. The `**Version:**` line in `rust/README.md`. It is not packaged, but all
   ten shipped links point straight at it, so a reader arriving from a 0.1.1
   crates.io page must not land on a page announcing the previous version.
3. The `hermes-crates-vX.Y.Z` tag inside the ten links themselves.

**Inter-crate version pins are correctness, not cosmetics.** In 0.x,
`version = "0.1.0"` means `^0.1.0`, so a dependent would *accept* 0.1.0 of its
dependency and a lockfile or `-Z minimal-versions` resolution can pick it.
Every dependency edge that crosses newly added API must therefore be raised in
the same commit as the version bump. For 0.1.1 that was `hermes-ast` →
`hermes-atom-table` and `hermes-parser`/`hermes-sema` → `hermes-ast`; each
raised pin carries a comment in the manifest saying which API forced it, so it
does not get "tidied" back later. Edges that cross no new API were left alone
(`hermes-sema` → `hermes-parser`) — over-pinning excludes working resolutions.

### Post-publish

Same as the 0.1.0 checklist below: verify each crate page, confirm the README
links resolve (they need the pushed tag), and confirm docs.rs built
`hermes-ast` / `hermes-parser` / `hermes-sema`.

---

## Launch runbook (as of 2026-08-12 — Tasks 3,4,5,6,10 complete, final review APPROVED) — HISTORICAL, 0.1.0 shipped

All automated prep is done: crates renamed `hermes-*` @ 0.1.0, API documented
(`missing_docs` clean), `parse()` façade + examples, bins in unpublished
`tools`, dry-run 6/6, perf claims reconciled. `hermes-sema` joined the set on
2026-08-12 (scope extension) — re-run the dry run for 7/7 before launching.
Only the manual, irreversible steps remain:

1. **Skip the placeholder name reservation** (plan Step 5) unless launch is
   weeks away — the real 0.1.0 publish IS the reservation, and placeholders
   add seven extra irreversible publishes plus junk 0.0.0 version rows.
2. **Push the `hermes-crates-v0.1.0` tag before publishing.** Every crate
   README links to GitHub through that tag (`/blob/hermes-crates-v0.1.0/…`)
   rather than through a branch, so the links survive any branch being renamed,
   merged or deleted — and they resolve to exactly the source that was
   published. Two rules follow:
   - **Push to the `private` remote, not `origin`.** In this worktree
     `origin` is `facebook/hermes` (upstream) and `private` is
     `git@github.com:tmikov/hermes.git` — the fork the READMEs and every
     crate's `repository` field point at:
     ```bash
     git push private hermes-crates-v0.1.0
     ```
     Until the tag is on the fork, all ten README links 404 on the crates.io
     pages, and a published README cannot be edited without a new version.
   - The tag must point at **the commit you publish from**. If any commit
     lands after the tag was cut, move it: `git tag -f hermes-crates-v0.1.0`
     then `git push -f private hermes-crates-v0.1.0`.
   - Pushing the tag alone is enough for the links to resolve (a tag carries
     its own commit). Push `rust` as well if you want the work browsable on
     the fork.
3. **Publish with ONE multi-package invocation** (the plan's per-crate loop
   provably fails: versioned path deps resolve against the registry):
   ```bash
   cargo login <token>
   cargo publish --manifest-path rust/Cargo.toml \
     -p hermes-unicode -p hermes-atom-table -p hermes-command-line \
     -p hermes-support -p hermes-ast -p hermes-parser -p hermes-sema
   ```
   All SEVEN must be named explicitly — an omitted `-p` is silently skipped,
   not an error.
   cargo stages them in dependency order and waits for index propagation.
   Verify each on crates.io and that docs.rs builds parser/ast/sema.
4. **Post-publish checklist:** confirm the ten README links resolve on the
   live crates.io pages (they depend on the pushed tag — a 404 there needs a
   patch release to fix, so check early); trigger
   `rust-differential-nightly.yml` once via `workflow_dispatch` for its first
   observed green; sweep `doc/superpowers/{SESSION-HANDOFF,RustPortRoadmap}.md`
   `-p parser` spellings in `doc/superpowers/{SESSION-HANDOFF,RustPortRoadmap}.md`;
   human editorial pass on the blog draft before any announcement.

**Branch fate is decoupled from the crates.** Because the READMEs point at an
immutable tag rather than a branch, the branch may be renamed or moved without
breaking anything on crates.io. Keep the tag, and keep it pointing at the
commit that was actually published.
