# Parser Recursion Parity + parse() Error Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.
> Pre-publication fixes for the standalone parser; design approved inline
> (2026-08-03 session). These close the roadmap's tracked parser-phase
> follow-ups (b) (recursion stack-overflow crash) and (c) (`parse()` missing
> error-count gate).

**Goal:** Deep-nesting inputs error exactly like hermesc instead of crashing
the process, and `parse()` upholds the C++ contract of returning `None`
whenever errors were reported.

**Architecture:** The Rust parser HAS the recursion counter
(`check_recursion`/`RecursionGuard`, `MAX_RECURSION_DEPTH = 1024`,
`js/mod.rs:99-110,718-733`) but counts FEWER increments per nesting level
than C++, so hermesc's counter trips (~paren depth 150, exit 2) where ours is
still under the limit — and the native stack dies (~depth 400, exit 134,
measured 2026-08-03) before our counter fires. The fix is SITE PARITY, not
new machinery: every C++ `CHECK_RECURSION` (an RAII `TrackRecursion` that
increments for the whole METHOD SCOPE, JSParserImpl.h:1745-1762) must map to
a `check_recursion` guard in the same Rust production, plus the one special
per-iteration site in the member-expression loop (cpp:3527-3535:
`SaveAndRestore` + `++recursionDepth_` per chain link). Fix B is the
two-line cpp:168-172 tail gate in `parse()`.

**Tech Stack:** as the port. C++ sources of truth: `lib/Parser/JSParserImpl.h`
(:187-200 limit, :1745-1762 macro), `lib/Parser/JSParserImpl.cpp` (17 sites +
:3527-3535 + :168-172 + :348-352 `recursionDepthExceeded`),
`lib/Parser/JSParserImpl-ts.cpp` (3 sites). Measured facts (2026-08-03):
paren ladder `('('*N + '1' + ')'*N + ';')` — hermesc first errors between
N=100 and N=200; Rust ast-dump exits 0 at N=200 and SIGABRTs (134) at N=400.

## Global Constraints

- NEVER `cd`; `--manifest-path rust/Cargo.toml` / absolute paths.
- Zero warnings all configs; no new clippy lints; 80-col new lines; every C++
  citation verified before writing; faithful port — C++ comments carried,
  quirks preserved.
- Gates that must stay green THROUGHOUT: parser differential
  (`REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test parser_differential`),
  BOTH sema differentials
  (`REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema --features dump-bin --test sema_differential`,
  currently driver 192/103 + parser-entry 11/3), full workspace suite.
  Oracles prebuilt: `cmake-build-asan/bin/{hermesc,sema-parser-dump}`.
- Corpus discipline: every new corpus file oracle-verified FIRST (raw
  stdout+stderr+exit); never curate away a fixable mismatch; MANIFEST
  arithmetic exact.
- Commits `rust(parser): <what>` / `rust(sema): <what>` + trailer
  `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

---

### Task 1: Recursion-count site parity

**Files:**
- Modify: `rust/crates/parser/src/js/{mod,expressions,statements,functions,classes,modules}.rs` + `js/ts/*.rs` as the audit dictates
- Test: parser corpus (`rust/crates/parser/tests/parser_corpus*/`) + sema
  corpus (`rust/crates/sema/tests/sema_corpus/` + MANIFEST) + a unit test in
  the parser crate
- Modify (docs, small): `doc/superpowers/RustPortRoadmap.md` follow-up (b)
  closed; `doc/superpowers/SESSION-HANDOFF.md` matching line

**Interfaces:**
- Consumes: `check_recursion`/`RecursionGuard` (js/mod.rs:718-733);
  `recursion_depth: Rc<Cell<u32>>` (js/mod.rs:193).
- Produces: increment-site parity with C++; the deep-input error
  `Too many nested expressions/statements/declarations` (cpp:348-352 —
  verify the Rust error text matches exactly) at IDENTICAL source locations.

- [ ] **Step 1: The audit table.** List the 20 C++ `CHECK_RECURSION` sites
  (`grep -n CHECK_RECURSION lib/Parser/JSParserImpl.cpp lib/Parser/JSParserImpl-ts.cpp`
  — 17 + 3, verified 2026-08-03) plus the loop site cpp:3527-3535. For each:
  identify the containing C++ method (read the surrounding code), find its
  Rust production, and record whether the Rust side has a `check_recursion`
  guard AT THE SAME SCOPE (guard held for the whole production body, exactly
  like `TrackRecursion`). Also verify the REVERSE direction: every Rust
  `check_recursion` site maps back to a C++ site (extra Rust sites inflate
  the count and shift error locations too). Write the table into the task
  report — the reviewer will re-derive it.
- [ ] **Step 2 (RED):** Binary-search the paren ladder to hermesc's exact
  threshold N* (first N where hermesc errors; between 100 and 200 per the
  measured facts). Add a parser-corpus file `nested-parens-limit.js` at
  depth ≥ N* (hermesc-verified: the recursion error, exit 2) — currently the
  Rust side exits 0 on it (counter under limit) or crashes at high depth:
  record the divergence as RED. Also run
  `test/Parser/nested-expressions.js` through both parser binaries and
  record the current divergence.
- [ ] **Step 3:** Fix the missing/extra sites per the audit (including the
  cpp:3527-3535 per-chain-link increment — port its `SaveAndRestore` shape
  faithfully with the established save/restore idiom). After each fix batch,
  re-run the ladder: DONE when hermesc and ast-dump error at the same N*
  boundary (N*-1 clean both, N* errors both, byte-identical) and
  `test/Parser/nested-expressions.js` is byte-identical through the parser
  differential pair. Import `nested-expressions.js` into the parser corpus.
- [ ] **Step 4: Sema side.** Verify the resolver tracker
  (`resolver/mod.rs:577-579,663` `AST_MAX_RECURSION_DEPTH`) against the C++
  `SemanticResolver` limit (find its value in the C++ — SemanticResolver.h or
  RecursionDepthTracker — cite it). Then re-verify
  `test/Sema/regress-nested-expressions-error.js` against hermesc: the
  documented col-3052-vs-6124 mismatch should now MATCH (the divergence was
  parser-side counting). If it matches, import it (deferred table 4 → 3).
  If it still mismatches, diagnose whether the residue is resolver-side
  counting and fix per the same parity method; escalate if it is neither.
- [ ] **Step 5:** Re-run the 1416-file upstream sweep (the MANIFEST-recorded
  method; debug builds — the MANIFEST warns `--release` masks a defect).
  Expect: the two nested-expression mismatch rows move buckets; panic bucket
  unchanged (8). Update the MANIFEST sweep section additively with exact
  arithmetic, and close roadmap follow-up (b) (+ the handoff line), keeping
  doc voices.
- [ ] **Step 6:** All gates + full workspace green; zero warnings. **Commit**
  `rust(parser): recursion-depth site parity with CHECK_RECURSION (JSParserImpl.h:1745-1762)`.

---

### Task 2: The `parse()` error-count gate

**Files:**
- Modify: `rust/crates/parser/src/js/mod.rs` (`parse()`, :875 region)
- Test: parser-crate unit test; the existing parser-entry corpus parse-error
  files are the differential pins
- Modify (docs, small): `doc/superpowers/RustPortRoadmap.md` follow-up (c)
  closed; `doc/superpowers/SESSION-HANDOFF.md` matching line;
  `rust/crates/sema/tests/sema_corpus_parser/MANIFEST.md` gate-classification
  paragraph updated (the entry-gate reasoning simplifies once the parser
  upholds the invariant)

**Interfaces:**
- Consumes: C++ `JSParserImpl::parse` tail (cpp:168-172: `if (!res) return
  None; if (getErrorCount() != 0) return None;` — re-read and cite exactly).
- Produces: Rust `parse()` returns `None` whenever
  `sm.error_count() != 0`, matching its "Port of `JSParserImpl::parse`" doc.
  Compensating callers (`ast_dump.rs:224`, `sema_dump.rs:463` region) become
  redundant — LEAVE them in place (they mirror driver-level checks and cost
  nothing) but update their comments to say the parser now also upholds the
  gate.

- [ ] **Step 1 (RED):** Parser-crate unit test: a recoverable-error input
  (`"use strict"; var x = 010;`) parses to a recovered AST internally but
  `parse()` must return `None` with `error_count() > 0`. Runs RED against
  the current gate-less `parse()`.
- [ ] **Step 2:** Port the cpp:168-172 tail; carry the C++ comment if any.
  Unit test GREEN.
- [ ] **Step 3:** All three differentials + full workspace green (the
  parser-entry corpus parse-error files pin the end-to-end behavior; the
  driver corpus's `parse-error.js` pins the driver path). Update the two
  caller comments + the three doc sites per Files. Zero warnings. **Commit**
  `rust(parser): parse() upholds the error-count gate (JSParserImpl.cpp:168-172)`.

---

## Self-review notes (plan-writing time)

- **Coverage vs the approved design:** Fix A = T1 (site parity, both
  directions + the loop site; ladder acceptance; both nested files; resolver
  tracker check; sweep re-count; roadmap (b)). Fix B = T2 (gate; unit test;
  callers kept-with-comment; roadmap (c); MANIFEST paragraph).
- **Sequencing:** T1 first — T1 moves error locations on deep inputs, which
  T2's differential re-runs would otherwise churn against.
- **Measured facts baked in** so the implementer needn't re-derive the
  failure mode (but MUST re-derive N* exactly).
- **Risk flagged:** extra Rust sites (22 vs C++ 20) may need REMOVAL —
  removing a check is behavior-changing in the safe direction only if C++
  lacks it at that scope; the audit table must justify each removal by the
  C++ absence, and the reviewer checks both directions.
