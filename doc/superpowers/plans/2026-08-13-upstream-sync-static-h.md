# Upstream C++ Sync — `static_h` @ `2d3e9018b` + `private/export-D115669841`

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the Rust port back into bug-for-bug agreement with upstream
`static_h`: correct two fixes that upstream revised before landing, port the
seven new front-end commits, and port the pending `try/catch/finally` fix that
closes `CppDefectsFound.md` item 12.

**Architecture:** The port is validated by byte-for-byte differentials against
binaries built from this repo's C++ tree, so every task moves the C++ tree
first (cherry-pick or correction), rebuilds the oracle, and then moves the Rust
mirror until the gates are green again. Task 2 deliberately reds all 232 sema
corpus comparisons and is sequenced alone so any mismatch is attributable.

**Tech Stack:** C++ (CMake/Ninja, ASan+Debug+clang), Rust 1.96 workspace, the
`sema_differential` / `parser_differential` / `json_differential` /
`preparse_differential` / lexer `differential` harnesses.

## Global Constraints

- **NEVER `cd`.** Use `git -C /home/tmikov/work/hermes-rust …`,
  `cargo --manifest-path /home/tmikov/work/hermes-rust/rust/Cargo.toml …`,
  `cmake --build /home/tmikov/work/hermes-rust/cmake-build-asan …`, absolute
  paths. All work happens on branch **`rust`** (the only branch; `rust1` was
  consolidated away 2026-08-13).
- **Upstream is the spec.** For every port, read the upstream commit
  (`git -C … show <hash>`) and mirror it faithfully — same logic shape, error
  strings byte-identical, comments adapted. Cite the upstream hash in the Rust
  comment or commit message.
- **Bug-for-bug means bug-for-bug.** Where upstream keeps a defect, the port
  keeps it, pinned by a test that asserts the defective behavior.
- **Gate figures that must return to green** after every task: sema driver
  corpus **219 files matched (109 succeeded)**, parser-entry **13 (5)**;
  parser differential **8/8**; json **1/1**; preparse **4/4**; lexer **6/6**;
  full workspace `cargo test`. Task 2 changes the *dump content* but not these
  counts.
- Zero warnings in both feature configs and under `RUSTFLAGS="-D warnings"`.
  fmt rule: per-file **no NEW** rustfmt diff hunks (the workspace is not
  rustfmt-default-clean).
- **Debug builds for all differential work** (`--release` masks
  `debug_assert` repros and the ASan oracle's recursion limits differ).
- C++ lit expectations: several upstream commits rewrite `test/Sema/*.js`
  `CHECK` lines. Do **not** hand-edit them and do **not** blindly regenerate.
  Take the source hunks, then
  `cmake --build /home/tmikov/work/hermes-rust/cmake-build-asan --target update-lit`
  and **review the regenerated diff** against what the upstream commit
  contains (CLAUDE.md: only use `update-lit` when you understand the cause).
- Rebuild the oracle after every C++ change:
  `cmake --build /home/tmikov/work/hermes-rust/cmake-build-asan --target hermesc sema-parser-dump json-parse-dump preparse-dump js-lexer-dump`
- Commit trailers on every commit:
  `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` and
  `Claude-Session: https://claude.ai/code/session_01ERsmFoVAnZCRwfapbPMibv`
- After each task, update the relevant row of
  `doc/superpowers/UpstreamSyncState.md` (move it out of the backlog). The
  final task updates "Ported through".

---

### Task 1: Correct the two divergences from what upstream actually landed

The port mirrors a pre-landing variant of two of the eleven cherry-picked
fixes. These are regressions against upstream, not new features.

**Files:**
- Modify: `include/hermes/Parser/JSONParser.h` (limit ladder)
- Modify: `lib/Sema/SemanticResolver.cpp` (two `SaveAndRestore` placements)
- Modify: `rust/crates/parser/src/json/parser.rs` (limit + module doc + the
  doc comment at ~line 33 that says "The values match
  `JSParserImpl::MAX_RECURSION_DEPTH`")
- Modify: `rust/crates/sema/src/resolver/classes.rs` (~1111-1132 and
  ~1285-1306)

**Interfaces:**
- Consumes: nothing.
- Produces: a C++ tree whose `304c1533c` and `dee8c5ce0` content matches
  upstream, and Rust mirrors to match.

- [ ] **Step 1: Correct the C++ JSON recursion limit to upstream's**

Upstream `304c1533c` landed **4× the `JSParserImpl` limits off Windows**
(a JSON nesting level costs far less stack); Windows values are unchanged.
Replace our ladder in `include/hermes/Parser/JSONParser.h` with upstream's
verbatim:

```bash
git -C /home/tmikov/work/hermes-rust show 304c1533c:include/hermes/Parser/JSONParser.h
```

Take its `MAX_RECURSION_DEPTH` block **and its explanatory comment** (which
records the measured stack-per-level figures justifying 4×). Net effect:
`HERMES_LIMIT_STACK_DEPTH` 128 → **512**, default 1024 → **4096**.

- [ ] **Step 2: Correct the C++ field-init `SaveAndRestore` placement**

Upstream `dee8c5ce0` places the `SaveAndRestore` **after** `declareArguments()`
in both `visit(ClassPrivatePropertyNode*)` and `visit(ClassPropertyNode*)`; we
placed it **before**. This decides which binding-table scope the `arguments`
decl lands in — the class scope (upstream) or the initializer function's body
scope (ours). Reorder to match upstream exactly, including upstream's comment
placement (`git show dee8c5ce0` is the spec).

- [ ] **Step 3: Rebuild the oracle and see what moved**

Rebuild (Global Constraints), then run the sema differential. **Expect the
field-init change to move dump output** for class-field files. Record which
corpus files change and how; that is the evidence the reorder is behavioral.

- [ ] **Step 4: Mirror the JSON limit in Rust**

`rust/crates/parser/src/json/parser.rs`: the port maps the C++ `#ifdef` ladder
onto `cfg!(debug_assertions)` (debug pairs with the ASan oracle's
`HERMES_LIMIT_STACK_DEPTH`, release with the default arm). With upstream's new
values that becomes **512 debug / 4096 release**. Update the constant, the
module doc (lines ~9-16) and the doc comment (~33-40) that currently assert
the limits match `JSParserImpl` — they no longer do, and saying so is now
wrong. Cite `304c1533c`.

- [ ] **Step 5: Mirror the field-init reorder in Rust**

`rust/crates/sema/src/resolver/classes.rs`: at both sites the port does
`old_scope = cur_scope; cur_scope = Some(body_scope); …; declare_arguments(); …`.
Move `declare_arguments()` **before** the `cur_scope` assignment so it runs in
the outer scope, matching upstream. Keep the restore where it is.

- [ ] **Step 6: Update the tests that pinned the old values**

Find the JSON recursion tests (`grep -rn "Too many nested" rust/crates/parser/`)
— any test asserting the old boundary (e.g. "129 fails") must move to the new
one. The `err_deep_nesting.json` corpus file nests 2000 deep, which still
exceeds 512, so it should still be an error on both sides — **verify, don't
assume**. Add or adjust a test that pins the *new* boundary and prove it fails
against the old constant.

- [ ] **Step 7: Verify**

All gates green (Global Constraints). Report the corpus-output changes from
Step 3 explicitly.

- [ ] **Step 8: Commit**

Two commits: `hermes: correct JSON recursion limit and field-init scope placement to match upstream`
(C++) and `rust: mirror the corrected JSON limit and field-init scope placement`
(Rust).

---

### Task 2: Port `04f1f53a8` — dump `mayReachImplicitReturn` (+ `-Xcompile`)

**The highest-value task.** The port already *computes*
`may_reach_implicit_return` but has never dumped it, so this byte-verifies the
port's `CheckImplicitReturn` across 232 corpus files for the first time. Any
disagreement with C++ surfaces here.

**Files:**
- Modify (cherry-pick): `lib/CompilerDriver/CompilerDriver.cpp`,
  `lib/Sema/SemContext.cpp`, ~100 `test/Sema/**/*.js` expectations
- Modify: `rust/crates/sema/src/dump_context.rs` (`print_function`, ~line 213)

**Interfaces:**
- Consumes: Task 1's corrected tree.
- Produces: dump lines of the form `Func loose mayReachImplicitReturn` /
  `Func strict noImplicitReturn` (and the same for `StaticBlock`) on both
  sides.

- [ ] **Step 1: Cherry-pick the C++ commit**

```bash
git -C /home/tmikov/work/hermes-rust cherry-pick 04f1f53a8
```

Expect conflicts in `test/Sema/**` expectations (upstream computed them
against a tree we do not have). Resolve by taking the **source** hunks
(`CompilerDriver.cpp`, `SemContext.cpp`) verbatim, then regenerating
expectations with `update-lit` per the Global Constraints and reviewing the
regenerated diff.

- [ ] **Step 2: Rebuild the oracle and confirm the expected red**

Rebuild, then run the sema differential. It **must** now fail — every `Func`
line gained a suffix the port does not emit. Record how many files mismatch
(expected: all that produce a dump). A green run here means the cherry-pick
did not take effect; investigate rather than proceeding.

- [ ] **Step 3: Mirror in the Rust dumper**

`rust/crates/sema/src/dump_context.rs::print_function` currently pushes
`"StaticBlock "`/`"Func "` then `strict`/`loose`. Upstream appends:

```cpp
<< (f.mayReachImplicitReturn ? " mayReachImplicitReturn" : " noImplicitReturn")
```

Mirror it exactly (leading space, both spellings). The value is
`info.may_reach_implicit_return` on the port's `FunctionInfo`
(`sem_context.rs:451`).

- [ ] **Step 4: Run the differential and treat every mismatch as a finding**

This is the point of the task. Re-run the sema differential. If files still
mismatch, the port's `CheckImplicitReturn` disagrees with C++ — for each
distinct shape, reduce it to a minimal repro, determine which side is right
(the C++ is the oracle), and fix the port. **Do not** paper over a mismatch by
special-casing the dumper. Report every divergence found, with its minimal
repro, even if the fix is small.

- [ ] **Step 5: Verify**

Sema differential back to **219 (109)** and **13 (5)**; all other gates green.

- [ ] **Step 6: Commit**

`rust(sema): dump mayReachImplicitReturn (upstream 04f1f53a8)` — plus separate
commits for any port bug this uncovers, each with its repro in the message.

---

### Task 3: Port the three new behavioral fixes

**Files:**
- Modify (cherry-pick): `lib/Sema/CheckImplicitReturn.cpp`,
  `lib/Sema/SemanticResolver.{cpp,h}`, `lib/Parser/JSParserImpl-flow.cpp`,
  their tests
- Modify: `rust/crates/sema/src/check_implicit_return.rs`
- Modify: the port's resolver (a new `MatchStatement`/`MatchExpression` visit
  pair — the port has **no** such visits today; `grep -rn "MatchStatement"
  rust/crates/sema/src` finds only a comment)
- Modify: the flow-match parser (`rust/crates/parser/src/js/flow/match_.rs`)

**Interfaces:**
- Consumes: Task 2's tree.
- Produces: Flow `match` handled in `CheckImplicitReturn`; `match` rejected
  under `compile_`; the match-object-property parse guarded.

- [ ] **Step 1: Cherry-pick all three C++ commits**

```bash
git -C /home/tmikov/work/hermes-rust cherry-pick 653e49c60 90f4a3ac6 ca6de21ce
```

(Chronological order.) Rebuild the oracle.

- [ ] **Step 2: Mirror `653e49c60` — Flow match in `CheckImplicitReturn`**

+61 lines in `CheckImplicitReturn.cpp`. Port into
`rust/crates/sema/src/check_implicit_return.rs` following that file's existing
structure (`check_termination`, the per-node `check_termination_*` helpers).
Note the file already carries a comment at ~line 249 about `MatchStatement`
being unhandled — update it to describe reality.

- [ ] **Step 3: Mirror `90f4a3ac6` — reject Flow match when compiling**

Two new visits emitting, under `compile_` only:
`"match statements are unsupported"` and `"match expressions are unsupported"`
(byte-identical), then visiting children. Parser mode (`compile = false`) must
still resolve them — that asymmetry is the point of the fix.

- [ ] **Step 4: Mirror `ca6de21ce` — check the parsed match object property**

The C++ adds `if (!optPattern) return false;` after
`parseMatchPatternFlow()` in `parseMatchObjectPatternPropertiesFlow`. Same
class as the already-ported `8d786acbe`. Mirror in the port's equivalent.

- [ ] **Step 5: Import the upstream tests as corpus files**

`test/Sema/flow/match-implicit-return.js`, `test/Sema/flow/match-unsupported.js`,
`test/Parser/flow/match/pattern-object-{binding,value}-error.js`. Oracle-verify
raw bytes on all three channels **before** importing, add the `// FLAGS:` line
each needs, and update the MANIFEST arithmetic exactly.

- [ ] **Step 6: Verify + commit**

All gates green; corpus counts increase by the number imported.
`rust: port Flow-match fixes (upstream 653e49c60/90f4a3ac6/ca6de21ce)`

---

### Task 4: Port `5ae5260c8` — `try/catch/finally`, closing defect 12

**Files:**
- Modify (cherry-pick from `private/export-D115669841`):
  `lib/Sema/CheckImplicitReturn.cpp`, `test/Sema/implicit-return-try-catch-finally.js`
- Modify: `rust/crates/sema/src/check_implicit_return.rs` (the
  `check_termination_try_statement` assert at ~line 342)
- Modify: `rust/crates/sema/tests/facade_agreement.rs` (delete the skip)
- Modify: `doc/superpowers/CppDefectsFound.md` (item 12 → fixed)

**Interfaces:**
- Consumes: Task 3's tree.
- Produces: `function f() { try {} catch (e) {} finally {} }` resolves cleanly
  through the parser entry on both sides.

- [ ] **Step 1: Cherry-pick and rebuild**

```bash
git -C /home/tmikov/work/hermes-rust cherry-pick 5ae5260c8
```

Then rebuild and confirm the item-12 repro is fixed:

```bash
printf 'function f() { try {} catch (e) {} finally {} }\n' > /tmp/bug12.js
/home/tmikov/work/hermes-rust/cmake-build-asan/bin/sema-parser-dump /tmp/bug12.js
# was: assert at CheckImplicitReturn.cpp:250, exit 134
```

- [ ] **Step 2: Mirror in Rust**

The port panics identically at `check_implicit_return.rs:340-342` ("try-catch-finally
should have been transformed by …"). Replace with the upstream fix's logic
(+78/−29 in C++ — it teaches the checker to walk an unsplit
try-catch-finally rather than assume the split). Faithful port, not an
approximation: the Release consequence of the old code was a *wrong answer*
(the finalizer was ignored), so the new traversal semantics matter.

- [ ] **Step 3: Delete the agreement-sweep skip**

`rust/crates/sema/tests/facade_agreement.rs` skips one corpus file via a
documented named constant (`PARSER_ENTRY_SKIP`). Remove the constant and its
use; the file must now pass. If it does not, the mirror is wrong.

- [ ] **Step 4: Flip the pin in the defect record**

`doc/superpowers/CppDefectsFound.md` item 12: add `Fixed upstream` (hash
`5ae5260c8`, `private/export-D115669841`, cherry-picked here 2026-08-13) and
`Pin flipped` lines in the same style as items 1–11, update the summary-table
row from **OPEN**, and update the "all 11 items fixed… item 12 is OPEN" note
in the header.

- [ ] **Step 5: Import the upstream test + verify**

Import `test/Sema/implicit-return-try-catch-finally.js` (oracle-verified) into
the corpus; MANIFEST arithmetic exact. All gates green.

- [ ] **Step 6: Commit**

`rust(sema): handle try-catch-finally in CheckImplicitReturn (upstream 5ae5260c8) — closes defect 12`

---

### Task 5: Tail items, sweep, and sync-state update

**Files:**
- Modify (cherry-pick): `lib/Sema/SemanticResolver.cpp` (×2 reverts),
  `unittests/AST/ResolverTest.cpp` + `test/Sema/parser-mode-*.js`
- Modify: `rust/crates/sema/tests/sema_corpus_parser/` (+ MANIFEST)
- Modify: `doc/superpowers/UpstreamSyncState.md`

**Interfaces:**
- Consumes: Tasks 1-4.
- Produces: the port synced through `origin/static_h` @ `2d3e9018b` plus the
  pending fix, recorded as such.

- [ ] **Step 1: Cherry-pick the two `#if 0` reverts and the test move**

```bash
git -C /home/tmikov/work/hermes-rust cherry-pick 6fbc3706d 8f9e357fd 26872f6e9
```

`6fbc3706d`/`8f9e357fd` restore `if ((false))` in place of `#if 0` around two
permanently-dead blocks (local-eval, `arguments` redeclaration). Dead in both
forms, so no Rust behavior changes — but the port **cites** these sites
(`CppDefectsFound.md` item 10b, and any `cpp:` line citations into that
region). Re-verify those citations against the post-cherry-pick line numbers
and fix the stale ones.

- [ ] **Step 2: Replace the two authored parser-entry corpus files**

`26872f6e9` gives upstream real files for the two shapes the port had to
author: `test/Sema/parser-mode-export-default-anon.js` and
`test/Sema/parser-mode-with-statement.js`. Replace
`sema_corpus_parser/{anon-export-default,with-statement}.js` with the upstream
files (oracle-verify first), keeping the corpus count at 13, and update the
MANIFEST rows to record that they are now upstream imports rather than
authored gap-fillers. If the upstream files need a flag the harness cannot
supply, keep ours and say why.

- [ ] **Step 3: Re-run the upstream sweep**

Re-run the sweep over the upstream `test/` corpus the roadmap describes (last
figures: 1408 identical / 3 mismatch / 7 panic over 1418 files). The C++ tree
gained files, so the population grows. Classify any **new** residual — do not
record it silently.

- [ ] **Step 4: Update `UpstreamSyncState.md`**

Move every ported row out of the backlog. Update "Ported through": the tree is
now fork point `60b5c73db` + the 11 original cherry-picks (2 corrected) + the
7 new ones + `5ae5260c8`. State plainly that the port is **not** at a single
upstream commit and what the residual gap is. Leave the deferred
`sema-parser-dump` follow-up OPEN.

- [ ] **Step 5: Full verification**

Every gate (Global Constraints), plus `cargo publish --dry-run` for all seven
crates in one multi-package invocation — the published crates' behavior
changed (JSON limit, dump format), so confirm packaging still works.

- [ ] **Step 6: Commit**

`doc(rust): sync state — ported through static_h 2d3e9018b + D115669841`

---

## Self-Review

- **Spec coverage:** all 7 new front-end commits mapped (T2: `04f1f53a8`;
  T3: `653e49c60`/`90f4a3ac6`/`ca6de21ce`; T5: `6fbc3706d`/`8f9e357fd`/
  `26872f6e9`), plus the pending `5ae5260c8` (T4) and both divergences (T1).
- **Sequencing rationale:** T1 first (small, corrects the baseline); T2 alone
  (it reds every sema comparison, so nothing else may be in flight); T3/T4
  additive; T5 cosmetic + bookkeeping.
- **The deferred `sema-parser-dump` retirement is deliberately NOT in this
  plan** — recorded in `UpstreamSyncState.md`, to be done after the dump-format
  change is green and attributable.
- **Discovery risk is concentrated in T2 Step 4**, which is written to treat
  mismatches as findings to investigate rather than noise to suppress.
