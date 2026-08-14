# `cpp:NNNN` Citation Checker

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn silent citation rot into a loud, cheap, mechanically-repairable
test failure. Today ~2331 comments in the Rust port point at C++ line numbers;
when a C++ edit shifts lines, every citation below it silently starts pointing
at the wrong code. This has reopened three times across two plans; task 5 of the
2026-08-13 sync remapped 436 tokens by hand-built throwaway tooling.

**Architecture:** A snapshot records, per citation site, the resolved C++ path,
the cited line range, and a hash of that span's text, plus the C++ commit it was
blessed against. A standing `cargo test` re-hashes and fails naming stale sites.
A `--remap` mode uses `git diff` from the blessed commit to HEAD to shift line
numbers mechanically, then re-verifies. The tool and its test live in the
**unpublished `tools` crate**, so nothing can reach a published tarball.

**Tech Stack:** Rust (the checker + the standing test), `git diff -U0` for the
line remap, TOML/JSON for the snapshot.

## Global Constraints

- **NEVER `cd`.** `git -C /home/tmikov/work/hermes-rust …`,
  `cargo --manifest-path /home/tmikov/work/hermes-rust/rust/Cargo.toml …`,
  absolute paths. Branch **`rust`** (the only branch).
- **The tool must live in `crates/tools`** (`publish = false`). Precedent: the
  `common/mod.rs` identity test was put there deliberately so a cross-package
  read can never affect a `cargo package` archive or its verify build. Do not
  add it to a published crate.
- **Do not mass-edit citations in this plan.** The checker's job is to *find*
  staleness. Repairing today's stale citations is a separate, later decision —
  bless the current state, and report how many sites are provably wrong.
- Zero warnings in both feature configs and under `RUSTFLAGS="-D warnings"`;
  per-file **no NEW** rustfmt diff hunks.
- Gates that must stay green: sema **224 (111)** + parser-entry **17 (9)**;
  parser 8/8; json 1/1; preparse 4/4; lexer 6/6; full workspace;
  `cargo publish --dry-run` for all seven crates in ONE multi-package call.
- Commit trailers:
  `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` and
  `Claude-Session: https://claude.ai/code/session_01ERsmFoVAnZCRwfapbPMibv`

## Measured facts (verified 2026-08-14 — do not re-derive, but do re-check counts)

- **1060** qualified citations: `Basename.cpp:NNN`, `.h`, `.def`, with optional
  `-MMM` range. ~40 distinct basenames.
- **1271** bare `cpp:NNN` citations, in **74** files.
- Top cited: `flow.cpp` 243, `SemanticResolver.cpp` 108, `SemContext.h` 84,
  `JSParserImpl.h` 62, `SemanticResolver.h` 60.
- **Shorthand basenames are real and must be mapped**: `flow.cpp` →
  `lib/Parser/JSParserImpl-flow.cpp`, `ts.cpp` → `…-ts.cpp`, `jsx.cpp` →
  `…-jsx.cpp`. Others (`SemContext.h` → `include/hermes/Sema/SemContext.h`)
  are unambiguous by basename.
- **Bare `cpp:` resolution is per-module and is NOT reliably stated in the
  file's header.** `classes.rs`'s header names no file, yet its `cpp:891-907`
  is `lib/Sema/SemanticResolver.cpp` (verified). `json/parser.rs`'s `cpp:202`
  is `lib/Parser/JSONParser.cpp` (verified). Header prose varies ("Port of",
  "a faithful port of", "Port of the expression-parsing section of") and
  **wraps across `//!` lines**. Do NOT parse prose — see Task 1 Step 2.

---

### Task 1: Resolution, snapshot, and the standing check

**Files:**
- Create: `rust/crates/tools/src/citations/mod.rs` (or a `src/bin/` + lib split
  — implementer's choice, but the logic must be unit-testable)
- Create: `rust/crates/tools/citations.toml` (the resolution config)
- Create: `rust/crates/tools/citations.snapshot.json` (or `.toml` — the blessed
  snapshot)
- Create: `rust/crates/tools/tests/citations.rs` (the standing test)
- Modify: `rust/crates/tools/Cargo.toml`

**Interfaces:**
- Consumes: the Rust sources under `rust/crates/*/src` and `*/tests`, and the
  C++ tree at the repo root.
- Produces: `check` (verify), `bless` (recompute snapshot) and the standing
  test. `remap` is Task 2.

- [ ] **Step 1: Extract citation sites**

Scan `rust/crates/*/{src,tests,examples}/**/*.rs` for both forms. A site is
(rust path, line, byte span of the citation token, resolved cpp path, start
line, optional end line). Handle the **wrapped** case: a citation may be split
across consecutive `//!`/`//` lines (`lib/Parser/` … newline … `JSONParser.cpp:202-211`).
Reconstruct logical comment blocks before matching, and record the byte span in
the *file* so Task 2 can rewrite precisely.

- [ ] **Step 2: Resolve, via explicit config — not prose**

Create `citations.toml` with two tables:

```toml
# Basename -> repo-relative path, for qualified citations. Shorthands included.
[qualified]
"flow.cpp" = "lib/Parser/JSParserImpl-flow.cpp"
"SemanticResolver.cpp" = "lib/Sema/SemanticResolver.cpp"
# ... one line per distinct cited basename

# Rust path glob -> the C++ file that a BARE `cpp:` means in those files.
[bare]
"crates/sema/src/resolver/*.rs" = "lib/Sema/SemanticResolver.cpp"
"crates/parser/src/json/parser.rs" = "lib/Parser/JSONParser.cpp"
# ... one entry per module family
```

Build it by enumerating the actual basenames and the actual bare-`cpp:` files
(the measured facts above give the shape). **Verify each bare mapping** by
sampling 2-3 citations from that glob and confirming the cited lines contain
what the Rust comment says they do — a wrong mapping would bless nonsense.
Any basename or file you cannot resolve confidently goes in an explicit
`[unresolved]` list with a one-line reason; the tool reports them as skipped,
never silently drops them.

- [ ] **Step 3: Snapshot**

For each resolved site store: rust path, line, the citation text as written,
resolved cpp path, start/end line, and a hash of the cited span's exact bytes.
Store once at top level: the C++ tree commit the snapshot was blessed against
(`git rev-parse HEAD`), so Task 2's remap has a base.

Single-line citations (`cpp:891`) hash **that one line**. Ranges hash the
inclusive span. A range whose end is past EOF is an **error**, not a skip —
that is exactly the past-EOF breakage seen before.

- [ ] **Step 4: `check` + the standing test**

`check` re-hashes every site and reports mismatches as
`rust/path.rs:LINE cites SemanticResolver.cpp:891-907 — span changed`. The test
in `crates/tools/tests/citations.rs` runs `check` and fails with the full list.
Keep the failure message short per site and point at the remap command.

- [ ] **Step 5: Bless and report the truth**

Bless against the current tree, then report: how many of the 2331 resolved, how
many are in `[unresolved]` and why, and — importantly — **how many sites are
provably wrong today** (the cited span exists but the Rust comment's claim
plainly doesn't match, e.g. a citation naming a function that isn't there).
You are not fixing those here; you are measuring the debt. Sample-verify at
least 20 blessed sites by eye.

- [ ] **Step 6: Prove the check can fail**

Mutate the C++ (insert a line high in `SemanticResolver.cpp`), run the test,
confirm it fails naming many sites; revert; confirm green. Then mutate a single
cited span's *text* without changing line count and confirm exactly that site
is named. Leave the tree clean.

- [ ] **Step 7: Verify + commit**

All gates. `rust(tools): citation checker — resolve, snapshot, and check cpp:NNNN citations`

---

### Task 2: `--remap`, the mechanical repair

**Files:**
- Modify: the Task 1 tool; add remap
- Modify: `rust/crates/tools/tests/citations.rs` (failure message points here)

**Interfaces:**
- Consumes: Task 1's snapshot (which carries the blessed C++ commit).
- Produces: `remap` rewrites stale citations in place and re-verifies.

- [ ] **Step 1: Line mapping from `git diff`**

For each cpp file with stale sites, compute a line map from the snapshot's
blessed commit to the working tree using `git diff -U0 <blessed> -- <file>`
(hunk headers give old→new offsets). Map each stale citation's start/end
through it.

- [ ] **Step 2: Re-verify before writing**

A remapped citation is only accepted if the span at the new location hashes to
the stored hash. If it does not, leave it alone and report it as needing human
attention — a citation whose *text* changed is a semantic question, not a
mechanical one.

- [ ] **Step 3: Rewrite precisely**

Use the byte spans from Task 1 Step 1 so only the digits change. Handle wrapped
citations. Do not reformat, do not touch anything else on the line.

- [ ] **Step 4: Prove it end to end**

Insert 15 lines near the top of `lib/Sema/SemanticResolver.cpp` (mimicking the
task-3 insert that caused this), run the test → red; run `remap` → the Rust
citations shift by 15; run the test → green; confirm the remapped citations
point at the same constructs as before (spot-check 5). Then revert the C++ and
`remap` again, confirming it shifts back. Leave the tree clean.

- [ ] **Step 5: Verify + commit**

All gates. `rust(tools): citation remap — mechanically repair shifted citations`

**Result (measured, and load-bearing for any later repair decision): `remap`
repairs drift, not wrongness — 0 of the 20 known-wrong citations Task 1 found
are mechanically repairable.** They are not stale (the C++ at the lines they
name is exactly what was blessed, so `check` is green on all 20 and remap
never considers them), and the text at each one's *intended* location differs
from the text at its cited location, so no destination passes the hash proof.
Repairing that debt needs a different instrument — comparing each cited span
against the Rust body claiming to mirror it — plus site-by-site review and a
`bless`. The full argument lives in `crates/tools/src/citations/remap.rs`'s
module doc, which is where it stays true.

---

### Task 2b (added 2026-08-14): the colon-less `NNNN in File.cpp` shape

Task 2's fix round found a whole citation shape the scanner cannot see:
`// parseReturnTypeAnnotationFlow — 2883 in JSParserImpl-flow.cpp`. Both the
implementer and the reviewer independently counted **137** of them, and the
reviewer's spot-check found **2 of 3 already rotted** (off by 3, off by 1). At
4.5% of the corpus, concentrated in the dialect files a cherry-pick touches
most (`js/statements.rs` 38, `js/flow/declarations.rs` 23, `js/ts/types.rs` 10,
`js/modules.rs` 10, `js/flow/types.rs` 10), leaving them unprotected undercuts
the tool's purpose.

Helpfully, they are uniform: all 137 name their file explicitly, and only three
basenames appear (`JSParserImpl.cpp` 65, `JSParserImpl-flow.cpp` 47,
`JSParserImpl-ts.cpp` 25), so **one scanner rule resolves all of them** — no
per-site overrides.

**Files:** the scanner in `rust/crates/tools/src/citations/`; `citations.toml`
if a rule needs declaring; `citations.snapshot.json` (re-bless).

- [ ] **Step 1: Teach the scanner the shape.** Match `NNNN[-MMMM] in <basename>.cpp`
  and resolve the basename through the existing `[qualified]` table. Keep the
  guessing surface narrow: require the explicit basename (do not invent a
  bare-number variant), and make sure the new pattern cannot swallow prose that
  merely contains "123 in Foo.cpp".
- [ ] **Step 2: Read them before blessing.** These are trust-on-first-use, and
  at least two are known wrong. Read all 137 against their C++ — this is the
  point of the task, not overhead. Sort each into: correct (bless), **drifted**
  (the cited construct exists elsewhere in the same file — repair the digits,
  since unlike the known-wrong 20 these have a hash-verified destination and
  repairing them is what makes blessing them meaningful), or **wrong** (the
  claim does not match anything nearby — leave, and add to the measured debt).
  Report the three counts.
- [ ] **Step 3: Bless and prove.** Re-bless; confirm the snapshot grows by the
  number blessed. Then prove protection: shift `JSParserImpl-flow.cpp`, confirm
  the newly-covered sites are now named stale, `remap` repairs them, revert,
  remap back to zero residue.
- [ ] **Step 4: Verify + commit.** All gates.
  `rust(tools): cover the "NNNN in File.cpp" citation shape (137 sites)`

**Result (measured, see `task-2b-report.md`): 102 correct / 35 drifted / 0
wrong**; snapshot 3046 -> 3183. 34 of the 35 are one drift in one file: every
`JSParserImpl-flow.cpp` banner below line 1618 was off by exactly −3, caused by
`bfeeb404f` (+1 at 1413) and `be443ad10` (+2 at 1529). Both commits are already
in this repository, so that quarter of the shape had been wrong for the whole
of Tasks 1 and 2 while `check` stayed green — the pool rotted precisely because
nothing was watching it. The 35th (`js/statements.rs:45`, 948 -> 949) was
written one line high, not drifted.

**Debt found and NOT repaired here:** 15 already-blessed `(flow.cpp:NNNN-MMMM)`
doc citations sitting next to those banners are short by exactly 2 (written +1
high, then moved −3). `check` cannot see them — their spans have not changed —
and `remap` cannot either (drift is not wrongness, Task 2 §5). 15 is a floor:
it was measured only over the 60 banners that have such a sibling. This belongs
to the repair decision Task 1 §5 called for.

---

### Task 3: Documentation, wiring, and closing the follow-up

**Files:**
- Create: `rust/crates/tools/citations/README.md` (or a doc comment — one
  discoverable place)
- Modify: `doc/superpowers/UpstreamSyncState.md` (close the OPEN item)
- Modify: `rust/CONTRIBUTING.md` (the gate section)
- Modify: `doc/superpowers/SESSION-HANDOFF.md` and/or `RustPortRoadmap.md`

**Interfaces:**
- Consumes: Tasks 1-2.
- Produces: a future session can find, run, and trust the tool.

- [ ] **Step 1: Document it**

What a citation is and why the port has them; how to run `check`, `bless`,
`remap`; what to do when remap declines a site; the resolution config and how
to add an entry when a new C++ file gets cited; and the `[unresolved]` list's
meaning. Write it for someone who has never seen this convention.

- [ ] **Step 2: Make the workflow explicit where people will hit it**

`CONTRIBUTING.md`: after any commit that changes the C++ tree, run `remap` then
`check` — the same breath as rebuilding the oracle. State that a C++-only edit
should be cheap, and that the remap is the reason it is.

- [ ] **Step 3: Close the follow-up**

`doc/superpowers/UpstreamSyncState.md`'s "checked-in citation checker" item goes
from OPEN to done, naming where the tool lives and what it does. Keep the
analysis that motivated it.

- [ ] **Step 4: Verify + commit**

All gates, plus `cargo publish --dry-run` all seven — confirm the `tools`
additions changed no published tarball.
`doc(rust): document the citation checker; close the follow-up`

---

## Self-Review

- **Scope discipline:** the plan measures existing citation debt but does not
  mass-repair it — that stays a separate decision, so a review can tell a tool
  bug from a pre-existing wrong citation.
- **The riskiest assumption** is the bare-`cpp:` resolution config; Task 1 Step 2
  requires sampling to verify each mapping rather than trusting a glob.
- **Placement in `crates/tools`** follows the established precedent and keeps
  published tarballs untouched (verified by the dry-run in Task 3).
- **Non-vacuity is required twice** (Task 1 Step 6, Task 2 Step 4), because a
  checker that cannot fail is worse than none.
