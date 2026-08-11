# C++ Defect-Fix Propagation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Propagate the 11 upstream C++ defect fixes (branch `cpp-defect-fixes`, commits `4ad67c992..550aafe33`) into the `rust` branch: cherry-pick the C++ commits so the oracle binaries match, mirror every behavior change in the Rust port, flip each named bug-for-bug pin, and re-verify all differential gates green.

**Architecture:** The rust branch carries both the C++ tree (the oracle: `cmake-build-asan/bin/hermesc`, `sema-parser-dump`, `json-parse-dump`) and the Rust port (`rust/crates/*`). Differential gates run BOTH binaries live at test time, so the C++ cherry-picks land first (Task 1), turning some gates red; Tasks 2–5 mirror the fixes in Rust area by area, re-greening each gate; Task 6 re-verifies everything and syncs docs.

**Tech Stack:** git cherry-pick, CMake/Ninja (ASan Debug + clang), cargo test (debug), lit via `LIT_FILTER`.

## Global Constraints

- **NEVER `cd`.** Use `git -C /home/tmikov/work/hermes-rust …`, `cargo … --manifest-path rust/Cargo.toml`, absolute paths. (CLAUDE.md)
- **The C++ commit diff is the spec.** For every mirror, `git -C /home/tmikov/work/hermes-rust show <hash>` (the commits are in this repo's object store) and port it faithfully — same logic shape, same error strings byte-for-byte, comments adapted.
- Oracle binaries: `cmake-build-asan/bin/{hermesc,sema-parser-dump,json-parse-dump}` in `/home/tmikov/work/hermes-rust`. Rebuild before relying on them.
- Rust verification in **debug** (`cargo test`), never `--release` (masks debug_assert repros). Zero warnings in both feature configs (`cargo build` + `cargo build --all-features` per crate touched, or the workspace-standard check).
- New lines ≤ 80 columns, 2-space indent style rules apply to C++ only; Rust uses rustfmt (`cargo fmt --check`).
- Corpus files are imported ONLY after oracle-verifying raw bytes (run both binaries, byte-compare stdout/stderr/exit). MANIFEST arithmetic must stay exact (imported + deferred = total).
- Commit directly to `rust`. Trailer on every commit:
  `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` and the Claude-Session line used by prior commits on this branch.
- Every pin flip cites the C++ commit hash in the Rust-side comment or commit message.
- Interim redness is expected: after Task 1, corpus files whose oracle output changed will fail until the matching Rust task lands. Each of Tasks 2–5 must leave its OWN area's gate green; Task 6 requires ALL gates green.

---

### Task 1: Cherry-pick the 11 C++ fixes and rebuild the oracles

**Files:**
- Modify (via cherry-pick): `lib/Sema/*`, `lib/Parser/*`, `lib/Support/SourceErrorManager.cpp`, `include/hermes/Parser/JSONParser.h`, `test/Sema/*`, `test/Parser/*`, `unittests/AST/*`

**Interfaces:**
- Consumes: commits `4ad67c992, 6b59daf0d, 918158cb0, 07efab88d, b351e1184, f90a83146, b21856de4, 5f313a13a, 9232443cf, 37520ccef, 550aafe33` (chronological order — cherry-pick in exactly this order).
- Produces: rebuilt oracle binaries with all 11 fixes; the six new C++ test files under `test/` that later tasks import into corpora.

- [ ] **Step 1: Cherry-pick in chronological order**

```bash
git -C /home/tmikov/work/hermes-rust cherry-pick 4ad67c992 6b59daf0d 918158cb0 07efab88d b351e1184 f90a83146 b21856de4 5f313a13a 9232443cf 37520ccef 550aafe33
```

The fixes were authored on a base 127 commits ahead of this branch's C++ base; only `lib/Sema/SemanticResolver.cpp` overlaps with the intervening commits. If a pick conflicts, resolve minimally, preserving the fix's intent exactly (re-read that commit's diff), then `git -C … cherry-pick --continue`. Keep the original commit messages (they already carry the required trailers).

- [ ] **Step 2: Rebuild the oracle binaries**

```bash
cmake --build /home/tmikov/work/hermes-rust/cmake-build-asan --target hermesc sema-parser-dump json-parse-dump
```

- [ ] **Step 3: Verify all 11 repros are fixed**

Run every repro from `doc/superpowers/CppDefectsFound.md` (same commands, same input files) against the rebuilt binaries. Each must now exit cleanly with the documented fixed behavior — no SIGABRT, no ASan crash. Specifically confirm: bug 6 `-commonjs` compile of `export default async function () { await 0; }` is clean; bug 8 both export errors say `'export' statement requires module mode`; bug 2 dump prints `*default*`; bug 3 dump prints ` UNR` and exits cleanly.

- [ ] **Step 4: Run the affected lit tests**

```bash
LIT_FILTER="Sema|Parser" cmake --build /home/tmikov/work/hermes-rust/cmake-build-asan --target check-hermes
```

Expected: 0 unexpected failures (the six new tests pass; pre-existing expected failures unchanged).

- [ ] **Step 5: No new commit needed** — the cherry-picks are the commits. Record the new HEAD in your report.

---

### Task 2: Rust parser mirrors — JSX attr names, flow-match recovery, JSON recursion limit

**Files:**
- Modify: `rust/crates/parser/src/js/jsx.rs` (or wherever `parseJSXElementName`'s member-expression check was ported)
- Modify: the Rust flow-match binding-pattern parser (search `in match binding pattern` in `rust/crates/parser/src/js/`)
- Modify: `rust/crates/parser/src/json/parser.rs` (+ its module doc)
- Modify: `rust/crates/parser/src/js/token.rs` area pin (the `token.rs:133` parity panic for defect 11)
- Test: Rust unit tests mirroring the C++ tests; parser differential corpus additions

**Interfaces:**
- Consumes: Task 1's rebuilt oracles; C++ commits `37520ccef`, `550aafe33`, `b21856de4` (the spec).
- Produces: parser differential gate green including two newly imported corpus files.

- [ ] **Step 1: Mirror `37520ccef` (JSX).** The C++ check in `parseJSXElementName` changed `isa<MemberExpressionNode>` → `isa<JSXMemberExpressionNode>`, making `<foo a.b="1"/>` report `unexpected member expression`. Find the ported check in the Rust JSX parser and make the identical change. If the Rust port faithfully carried the dead `MemberExpression` check, it changes the same way; if the port omitted/annotated it as dead (check comments), implement the now-live check.
- [ ] **Step 2: Mirror `550aafe33` (flow match).** Add the `return None;` after the errorExpected call in the match-binding-pattern parse (Rust equivalent: return `None` after the `error_expected` for `'identifier' expected in match binding pattern`). Remove/flip the defect-11 parity pin: the pinned test that asserted the panic at `token.rs:133` must now assert the clean diagnostic + `None` recovery, citing `550aafe33`.
- [ ] **Step 3: Mirror `b21856de4` (JSON limit).** Add `recursion_depth` + `MAX_RECURSION_DEPTH` to the Rust `JSONParser`: split `parse_value` into a depth-checking wrapper (`error("Too many nested JSON values")` then `None` at the limit) and a `parse_value_impl`. Limit constant follows the port's established mapping of the C++ `#ifdef` ladder: `128` under `cfg!(debug_assertions)`, `1024` release (same rule as the JS parser's `MAX_RECURSION_DEPTH` — cite the user ruling and `b21856de4`). Update the module doc that recorded parity-by-absence.
- [ ] **Step 4: Mirror the C++ tests as Rust unit tests** — one test per fix, asserting the exact diagnostics: JSX `unexpected member expression` on `<foo a.b="1"/>`; match `'identifier' expected in match binding pattern` with clean recovery on `const e = match (x) { const [y]: 2 };`; JSON `Too many nested JSON values` on input nested past the debug limit (e.g. 200 `[`), plus a passing case just under a small nesting (do NOT try to hit 1024 in debug).
- [ ] **Step 5: Import the two new C++ parser tests into the parser differential corpus** (`test/Parser/jsx-error-attr-member.js`, `test/Parser/flow/match/pattern-binding-error.js`), oracle-verified first, with the needed `// FLAGS:` lines (jsx: `-parse-jsx`; match: `-parse-flow -Xparse-flow-match` — probe the actual flags the oracle needs). Update the parser corpus MANIFEST arithmetic.
- [ ] **Step 6: Run the parser crate tests + differential gate**

```bash
cargo test --manifest-path /home/tmikov/work/hermes-rust/rust/Cargo.toml -p hermes_parser
```

(plus the differential test binary the corpus uses — see the corpus README/MANIFEST for the exact invocation). Expected: green.
- [ ] **Step 7: Commit** `rust(parser): mirror upstream fixes 37520ccef/550aafe33/b21856de4 — JSX attr member, match recovery, JSON recursion limit`

---

### Task 3: Rust resolver mirrors — promoter `using`, promoter dead code, anon async default, export wording

**Files:**
- Modify: `rust/crates/sema/src/resolver/promoter.rs`
- Modify: `rust/crates/sema/src/resolver/modules.rs`
- Modify: `rust/crates/sema/tests/resolver.rs` (flip `export_default_anonymous_function_is_rewritten_to_an_expression`, ~line 2602)
- Test: sema corpus imports of `test/Sema/using-scoped-fn-promotion.js`, `test/Sema/export-default-anon-async.js`; MANIFEST update

**Interfaces:**
- Consumes: Task 1's oracle; C++ commits `4ad67c992`, `9232443cf`, `6b59daf0d`, `f90a83146`.
- Produces: sema driver gate green for these areas; corpus grows by the two new files (plus any previously-deferred rows these fixes unblock).

- [ ] **Step 1: Mirror `4ad67c992` (promoter).** In the Rust `extract_declared_idents` equivalent, replace the let/const-only `debug_assert!` pin with the C++ fix's logic: `var` kind → var-scoped ident list; every other kind (let, const, `using`, `await using`) → lexical. Cite `4ad67c992` where the pin comment was.
- [ ] **Step 2: Mirror `9232443cf` (dead code).** Check whether the Rust promoter ported the write-only `newDecls` local and the stale header comment describing delete/re-add behavior. Remove/fix whatever was carried over; if the port never carried it, note that in the report (no change).
- [ ] **Step 3: Mirror `6b59daf0d` (async).** In the anonymous-default-export rewrite in `modules.rs`, the pinned `/* async */ false` becomes the function's real `async` flag. Flip the `resolver.rs:2602` test to assert `async == true` for an async input (and keep a non-async case asserting `false`), citing `6b59daf0d`.
- [ ] **Step 4: Mirror `f90a83146` (wording).** The `ExportAllDeclaration` error string changes to `'export' statement requires module mode` (byte-identical to the Named/Default sites). Update the string and any test asserting the old wording.
- [ ] **Step 5: Import the two new C++ tests into `rust/crates/sema/tests/sema_corpus/`**, oracle-verified (raw-byte compare of `hermesc -dump-sema` vs `sema-dump`; `export-default-anon-async.js` may need `// FLAGS: -commonjs` — probe its C++ RUN line and the harness). Re-probe any Deferred MANIFEST rows these fixes unblock (search MANIFEST for `using`/promoter/export-default rows) and import what now matches. Update MANIFEST arithmetic exactly.
- [ ] **Step 6: Run the sema crate tests + driver differential gate**

```bash
cargo test --manifest-path /home/tmikov/work/hermes-rust/rust/Cargo.toml -p hermes_sema
```

Expected: green, corpus count strictly greater than 212.
- [ ] **Step 7: Commit** `rust(sema): mirror upstream fixes 4ad67c992/9232443cf/6b59daf0d/f90a83146 — promoter using, dead code, anon async default, export wording`

---

### Task 4: Rust resolver mirrors — `$SHBuiltin.#x()` rejection, field-init scope parenting

**Files:**
- Modify: `rust/crates/sema/src/resolver/calls.rs` (`sh_builtin_property_name` panic → error path)
- Modify: the Rust `ClassProperty`/`ClassPrivateProperty` visits (field-initializer scope handling) in `rust/crates/sema/src/resolver/`
- Test: sema corpus imports of `test/Sema/shbuiltin-private-name.js`, `test/Sema/class-field-class-expr.js`; MANIFEST landmine-row update

**Interfaces:**
- Consumes: Task 1's oracle; C++ commits `07efab88d`, `b351e1184`.
- Produces: driver gate green; the defect-4 MANIFEST landmine row resolved.

- [ ] **Step 1: Mirror `07efab88d`.** Read the full diff (it restructures ~50 lines): the `cast<IdentifierNode>` on `$SHBuiltin`'s callee property becomes a checked path that reports an error for non-Identifier properties (PrivateName). Port the exact control flow and error string. Replace the `sh_builtin_property_name` explicit panic pin, citing the commit.
- [ ] **Step 2: Mirror `b351e1184`.** In both class-property visits, wrap the field-initializer *value* visit so the initializer function's body scope is the current scope (`SaveAndRestore<LexicalScope*>` in C++ — use the port's established scope-save idiom), matching what the static-block visit already does. This CHANGES SEMA DUMP SHAPE for class expressions in field initializers.
- [ ] **Step 3: Corpus + MANIFEST.** Import `shbuiltin-private-name.js` and `class-field-class-expr.js` (oracle-verified). Find the defect-4 landmine row in the sema corpus MANIFEST (`class-field-class-expr` / scope-walk abort note) and any rows deferred on it; re-probe and update. MANIFEST arithmetic exact.
- [ ] **Step 4: Run sema tests + driver gate** (same command as Task 3 Step 6). Expected: green. If any pre-existing corpus file's dump shape changed under `b351e1184`'s mirror (field-init + class-expression files — e.g. `field-init-bindings.js`), the live differential re-greens only when Rust matches the new oracle bytes; verify explicitly that previously-imported class-field files still match.
- [ ] **Step 5: Commit** `rust(sema): mirror upstream fixes 07efab88d/b351e1184 — $SHBuiltin private-name rejection, field-init scope parenting`

---

### Task 5: Rust dumper mirrors + divergence-note cleanup

**Files:**
- Modify: `rust/crates/sema/src/dump_context.rs` (~line 304 panic pin → `*default*`; ~line 241 note if affected)
- Modify: `rust/crates/sema/src/dump.rs` (unresolvable-identifier comment/pin, ~lines 82–101)
- Modify: `rust/crates/support/src/manager.rs` (~lines 903–909: stable-sort divergence note)
- Test: `rust/crates/sema/tests/sema_corpus_parser/` imports + MANIFEST

**Interfaces:**
- Consumes: Task 1's oracle (`sema-parser-dump` now survives bugs 2/3 in debug); C++ commits `918158cb0`, `5f313a13a`.
- Produces: parser-entry gate green with previously-deferred error-path files imported.

- [ ] **Step 1: Mirror `918158cb0` in `dump_context.rs`.** Null-id hoisted function prints `*default*` instead of panicking; copy the C++ comment's substance (anonymous `export default function` only rewritten when compiling). Cite the commit.
- [ ] **Step 2: Reconcile `dump.rs`.** The Rust AST printer already prints ` UNR` (Release parity). Update the comment/pin that documented the C++ debug-assert divergence: debug now matches Release upstream (`918158cb0`). Assert via a unit test that `with(o){x;}` under the parser entry dumps ` UNR` cleanly.
- [ ] **Step 3: Mirror `5f313a13a` note.** `manager.rs` already uses a stable sort; rewrite the comment that recorded this as a known divergence — upstream now stable-sorts too (`5f313a13a`), divergence gone.
- [ ] **Step 4: Parser-entry corpus.** Re-probe the `sema_corpus_parser` MANIFEST rows deferred on the C++ debug crashes (bug 2/3 files) against the rebuilt `sema-parser-dump`; import everything that now byte-matches. Update MANIFEST arithmetic.
- [ ] **Step 5: Run the parser-entry differential gate** (`sema_differential.rs`'s parser-entry portion / the corpus README's invocation). Expected: green, count strictly greater than 11 files if rows unblocked.
- [ ] **Step 6: Commit** `rust(sema): mirror upstream fix 918158cb0 — parser-entry dump *default* + UNR; retire stable-sort divergence note (5f313a13a)`

---

### Task 6: Full gate re-verification + docs/memory sync

**Files:**
- Modify: `doc/superpowers/CppDefectsFound.md`, `doc/superpowers/RustPortRoadmap.md`, `doc/superpowers/SESSION-HANDOFF.md`
- (Controller, not implementer, updates session memory afterward.)

**Interfaces:**
- Consumes: Tasks 1–5 complete.
- Produces: all gates green with final counts; docs carry the new live figures.

- [ ] **Step 1: Full Rust workspace test run**

```bash
cargo test --manifest-path /home/tmikov/work/hermes-rust/rust/Cargo.toml
```

Expected: green, zero failures. Also `cargo fmt --check` and the zero-warnings check both feature configs.
- [ ] **Step 2: Re-run the upstream sweep** (the harness prior sweeps used — see RustPortRoadmap's sweep section; last figures 1405 matched / 3 residual / 8 corpus). The six new C++ test files enter the sweep population. Record new matched/residual figures and classify any residual changes (expected: residuals shrink or hold; any NEW residual must be investigated, not recorded silently).
- [ ] **Step 3: Update `CppDefectsFound.md`.** Per defect: add a `Fixed upstream` line (commit hash, 2026-08-08 branch, cherry-picked to rust 2026-08-10) and `Pin flipped:` line naming the Rust change. Do not delete the original analysis.
- [ ] **Step 4: Update roadmap + handoff live figures.** New gate counts (driver corpus N/M succeeded, parser-entry N/M, parser corpus N, sweep figures), the defect-propagation event, and close the "flip Rust pins when syncing" coordination note. Per standing process rule: sync EVERY live figure, both docs.
- [ ] **Step 5: Commit** `doc(rust): C++ defect fixes propagated — pins flipped, gates re-verified, live figures synced`

---

## Self-Review

- Spec coverage: all 11 defects mapped — 1→T3S1, 2→T5S1, 3→T5S2, 4→T4S2, 5→T4S1, 6→T3S3, 7→T2S3, 8→T3S4, 9→T5S3, 10→T2S1+T3S2, 11→T2S2. C++ sync → T1. Gates/docs → T6.
- Exact strings embedded where known (`*default*`, `Too many nested JSON values`, `'export' statement requires module mode`, `unexpected member expression`); commit diffs are the authoritative spec for the rest.
- Interim-redness policy stated in Global Constraints; every Rust task ends with its own gate green.
