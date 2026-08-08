# Parser errorExpected Geometry (what/whatLoc) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.
> Closes roadmap parser-track follow-up (a) — the last known bug-for-bug gap
> between the port and hermesc on rejected input (~180 of the 188 remaining
> upstream-sweep mismatch files). Design approved inline (2026-08-08 session).

**Goal:** Error diagnostics from `need`/`eat`/`errorExpected` sites render
byte-identically to hermesc: the same-line `combineIntoRange` annotation AND the
cross-line `note:` diagnostic, both currently dropped.

**Architecture:** C++ `JSParserImpl::errorExpected` (cpp:175-227, read in full
2026-08-08) builds the message, then: if `whatLoc` is valid it compares source
coords of the error token and `whatLoc` (`findBufferLineAndLoc`); SAME line →
`sm_.error(errorLoc, combineIntoRange(whatLoc, errorLoc), msg)` (caret at the
error, tildes across the range); DIFFERENT line → plain `sm_.error(errorLoc,
msg)` plus, when `what != null`, `sm_.note(whatLoc, what)` — a separate note
diagnostic. The Rust helpers (`js/mod.rs:553` `need(kind, where_)` and friends)
dropped `what`/`whatLoc` entirely, so neither arm exists. The support crate
ALREADY has the primitives: `combine_into_range` (manager.rs:410-411, ported
from header:601-607), `note` (manager.rs:648), `error_at` with range. The fix is
helpers + ~189 call-site restorations (grep counts 2026-08-08: JSParserImpl.cpp
75 `need(`/`eat(` + flow 82 + ts 24 + jsx 8), each argument-restored FROM THE
C++ SITE, never invented.

**Tech Stack:** as the port. C++ sources of truth: `lib/Parser/JSParserImpl.h`
(:439-462 the three errorExpected overloads, :469 `need`, :498 `eat`),
`JSParserImpl.cpp:175-227` (`errorExpected` body), the four `JSParserImpl*.cpp`
call-site files, `include/hermes/Support/SourceErrorManager.h:601-607`.
Measured example (2026-08-08): `var a = (1 + 2;` → hermesc `~~~~~~^` (range from
the `(` to the error token) vs port bare `^` — same text/line:col/exit.

## Global Constraints

- NEVER `cd`; `--manifest-path rust/Cargo.toml`. Zero warnings all configs; no
  new clippy lints; 80-col new lines; every C++ citation verified before
  writing; faithful port — argument values (the `where`/`what` strings, the
  `whatLoc` choice) come from each C++ call site verbatim, NEVER invented or
  "improved". C++ default args are spec — read the header defaults at every
  overload.
- Gates that must stay green THROUGHOUT: parser differential (77+ files), BOTH
  sema differentials (driver 200/107, parser-entry 11/3), full workspace suite.
  Oracles prebuilt in `cmake-build-asan/bin/`. Debug builds for sweeps (the
  sema MANIFEST documents why `--release` lies).
- New corpus files oracle-verified FIRST, raw three channels; MANIFEST
  arithmetic exact; never curate away a fixable mismatch.
- The error-path corpus home is the SEMA corpus (three-channel comparison; the
  parser differential asserts oracle success — the S4a/recursion-branch
  precedent). Parser-crate unit tests may pin rendering oracle-free.
- Commits `rust(parser): <what>` + trailer
  `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

---

### Task 1: The errorExpected mechanism (both arms) + pilot sites

**Files:**
- Modify: `rust/crates/parser/src/js/mod.rs` (`error_expected_msg` :580,
  `need` :553, `need_at` :611, `error_expected2` :638, `error_expected3` :660,
  `error_expected4` :685, `eat` :707 — extend signatures with
  `what: Option<&str>` (C++ nullable `const char*`) and `what_loc: SMLoc`
  (invalid sentinel = "not provided"; verify the port's SMLoc validity
  convention with grep and mirror `whatLoc.isValid()`))
- Test: parser-crate unit test pinning both arms' rendering; 2 sema-corpus
  files
- Pilot call sites only (the full sweep is Task 2): the parenthesized-
  expression `eat` site (the measured example — find the C++ site by grepping
  `end of parenthesized expression` in JSParserImpl.cpp and restore its exact
  `where`/`what`/`whatLoc` args) plus ONE cross-line-note site (find a C++
  site whose `whatLoc` can be on an earlier line — e.g. a block/brace matcher —
  and derive a two-line input that makes hermesc emit the `note:`; verify with
  hermesc FIRST).

**Interfaces:**
- Consumes: `support::SourceErrorManager::{combine_into_range (manager.rs:410),
  note (:648), error_at (:630)}`; the port's coords lookup (the C++ calls
  `findBufferLineAndLoc` twice and compares `isSameSourceLineAs` — find the
  ported equivalents with grep; the AST-dumper follow-up notes a
  `find_coords`-vs-`findBufferLineAndLoc` distinction: use the faithful one and
  cite it).
- Produces: the extended helper signatures every Task-2 site restoration uses.
  ALL existing call sites keep compiling by passing `(None, SMLoc::invalid())`
  equivalents in this task (mechanical; Task 2 restores real values) — OR, if
  the crate's established pattern for such migrations differs, follow it and
  say so in the report.

- [ ] **Step 1: Read** cpp:175-227 + the .h overloads (:439-462, :469, :498) in
  full; verify the message-building half already matches `error_expected_msg`
  (the S1 33-site fix covered text — confirm) and port ONLY the missing
  geometry half: the coords comparison + the two arms, carrying the C++
  comments.
- [ ] **Step 2 (RED):** hermesc-verify then add `error-expected-same-line.js`
  (the `var a = (1 + 2;` family) and `error-expected-cross-line-note.js` (the
  derived two-line note shape) to the sema corpus. Both currently FAIL the
  differential (bare caret / missing note) — record as RED. Unit test asserting
  both rendered forms (oracle-free).
- [ ] **Step 3:** Implement; restore the two pilot sites' args from the C++;
  gate green (202 driver files); full workspace; zero warnings both configs.
- [ ] **Step 4: Commit**
  `rust(parser): errorExpected geometry — combineIntoRange + cross-line note (JSParserImpl.cpp:175-227)`.

---

### Task 2: The full call-site sweep (~189 sites + direct errorExpected calls)

**Files:**
- Modify: `rust/crates/parser/src/js/{mod,expressions,statements,functions,classes,modules,jsx}.rs`,
  `js/flow/*.rs`, `js/ts/*.rs` — every site
- Test: existing gates; spot corpus additions ONLY where a site class has no
  coverage and a shape is cheap to verify

**Interfaces:**
- Consumes: Task 1's extended helpers.
- Produces: every Rust `need`/`eat`/`error_expected*` call site carrying the
  same `where`/`what`/`whatLoc` values as its C++ counterpart.

- [ ] **Step 1: The site table.** Enumerate every C++ call site:
  `grep -n "need(\|eat(\|errorExpected(" lib/Parser/JSParserImpl.cpp
  JSParserImpl-flow.cpp JSParserImpl-ts.cpp JSParserImpl-jsx.cpp` (2026-08-08
  counts: 75/82/24/8 for need|eat; direct errorExpected extra — count them).
  For each: the C++ args (where/what/whatLoc) → the Rust site → restored args.
  The table ships in the report; reviewers re-derive samples. Sites where C++
  passes nullptr/invalid stay `(None, invalid)` — do not invent locations.
- [ ] **Step 2:** Restore in file-sized batches; after each batch run the
  parser + sema differentials (they must stay green — text doesn't change,
  only geometry on error paths the corpora now partially pin).
- [ ] **Step 3:** All gates + full workspace green; zero warnings. **Commit**
  `rust(parser): errorExpected sweep — restore where/what/whatLoc at all call sites`.

---

### Task 3: Upstream sweep re-count + docs

**Files:**
- Modify: `rust/crates/sema/tests/sema_corpus/MANIFEST.md` (+ imports),
  `doc/superpowers/RustPortRoadmap.md` (close follow-up (a); update the
  bug-for-bug statement), `doc/superpowers/SESSION-HANDOFF.md`,
  `doc/superpowers/CppDefectsFound.md` only if the sweep surfaces a new C++
  finding

**Interfaces:**
- Consumes: the S3-T3/S4a-T5 sweep method (MANIFEST-recorded; debug builds).
- Produces: refreshed buckets. EXPECTED: mismatch 188 → a small residue
  (~8 known non-errorExpected rows: the S2-documented recursion-crash pair
  reclassifications etc. — classify EVERY residual file individually; any
  residual errorExpected-geometry file means Task 2 missed its site — fix, not
  defer). Panic bucket stays 8.

- [ ] **Step 1:** Full 1416-file sweep; bucket arithmetic file-by-file vs
  1220/188/8; classify all residuals.
- [ ] **Step 2:** Import 2-3 representative newly-matching upstream files as
  corpus rows (hermesc-verified; MANIFEST rows + arithmetic).
- [ ] **Step 3:** Docs: follow-up (a) closed (in each doc's voice; the roadmap
  bug-for-bug qualifier updated to name only the remaining deviations —
  unstable-sort ties + profile-mapped limits); gates + workspace green.
  **Commit** `rust(parser): errorExpected geometry complete — sweep re-count + docs`.

---

## Self-review notes (plan-writing time)

- Both arms covered (T1); the note arm needs its own pin (T1 Step 2's second
  file) — a same-line-only fix would satisfy the measured example but not the
  C++.
- The ~189-site sweep is transcription with per-site C++ reading (T2); drift
  risk is args invented instead of copied — the site table + reviewer sampling
  is the control (the S4a audit-table precedent).
- T3's "every residual classified individually" prevents the mismatch bucket
  from silently absorbing missed sites.
- Facts pinned at plan time: the cpp:175-227 body (read), support primitives
  already ported (manager.rs:410/:648), site counts 75/82/24/8, the measured
  same-line example.
