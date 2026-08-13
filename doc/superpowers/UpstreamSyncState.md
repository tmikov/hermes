# Upstream C++ sync state

**What this file is:** the record of *which upstream C++ state the Rust port
mirrors*. The port is validated by byte-for-byte differential testing against
binaries built from this repo's C++ tree, so "which upstream commit are we
bug-for-bug equal to" is a load-bearing fact, not trivia. Update it whenever
upstream fixes are ported.

---

## Ported through (as of 2026-08-13)

| | |
|---|---|
| **Fork point from `static_h`** | **`60b5c73db`** ("Deploy 0.316.0 to xplat") |
| **Plus** | the 11 defect fixes cherry-picked 2026-08-10 (below) |
| **Plus** | `04f1f53a8` (`-Xcompile` + dump `mayReachImplicitReturn`), cherry-picked 2026-08-13 as `1e3806f47`, mirrored in the port by `de917f249` |
| **Plus** | the three Flow-`match` fixes `653e49c60`/`90f4a3ac6`/`ca6de21ce`, cherry-picked 2026-08-13 as `acf86bf51`/`502bbc7d3`/`be443ad10`, mirrored in the port by the task-3 commit |
| **Plus** | `5ae5260c8` (try-catch-finally in `CheckImplicitReturn`, `CppDefectsFound.md` item 12), cherry-picked 2026-08-13 as `9b5025f89`, mirrored in the port by `2253b7331` — **from `private/export-D115669841`, not from `static_h`**; see below |
| **Upstream `static_h` HEAD at time of writing** | `2d3e9018b` (2026-08-13) |
| **Commits between fork point and upstream HEAD** | 147 (105 of them predate the local `static_h` ref at `5dfe740ad`) |

The port's C++ tree is **not equal to any single upstream commit**: it is the
fork point plus sixteen cherry-picks. The 132 other commits in
`60b5c73db..origin/static_h` are unported, but all but three are irrelevant to
the front end (VM, GC, debugger, JSI, build). See the backlog below.

### The 11 cherry-picked defect fixes

Found by this port's differential testing (`CppDefectsFound.md` items 1–11),
fixed upstream 2026-08-08, cherry-picked here 2026-08-10 (plan:
`plans/2026-08-10-cpp-defect-fixes-propagation.md`).

| Upstream (landed in `static_h`) | In-tree cherry-pick | Landed content vs ours |
|---|---|---|
| `504becabe` promoter `using` | `e4408f849` | identical |
| `efb4594e2` anon `export default async` | `4a0fe2bfd` | identical |
| `7d155fb21` semDump on parser-resolved ASTs | `179fb8ca3` | identical |
| `21fa90ff9` `$SHBuiltin.#privateName()` | `416aafcd2` | identical |
| `dee8c5ce0` class-expr scope parenting in field inits | `48d221fb2`, corrected 2026-08-13 | identical |
| `4aa3006f8` export module-mode wording | `4193b558a` | differs (cosmetic wrap only) |
| `304c1533c` JSONParser recursion limit | `0b8bbd1fc`, corrected 2026-08-13 | identical |
| `87677f148` stable-sort buffered diagnostics | `7805e2103` | identical |
| `88ca314ed` promoter dead code | `ffcdbdd52` | identical |
| `91f1222dd` JSX member-expression attr names | `51035e8c2` | identical |
| `8d786acbe` match binding-pattern crash | `bfeeb404f` | identical |

Comparison method: `git show <commit> -- lib include | git patch-id --stable`,
which ignores line-number drift, so "identical" means the source change really
is the same.

**The two behavioral divergences were corrected on 2026-08-13** (plan task 1).
Upstream had revised both fixes before landing them, so the port had been
mirroring a variant upstream no longer has:

1. **`304c1533c` (JSON recursion limit) — was behavioral, now fixed.**
   Upstream landed **4× the `JSParserImpl` limits off Windows** (a JSON
   nesting level costs far less stack): `HERMES_LIMIT_STACK_DEPTH` →
   **512**, default → **4096**; Windows values unchanged. Our cherry-pick had
   used the un-scaled 128/1024. `include/hermes/Parser/JSONParser.h`,
   `lib/Parser/JSONParser.cpp` and `unittests/AST/JSONTest.cpp` are now
   byte-identical to `304c1533c`'s versions, and
   `rust/crates/parser/src/json/parser.rs` mirrors the new values
   (512 debug / 4096 release under the standing `cfg!(debug_assertions)`
   mapping). `err_deep_nesting.json` grew from 2000 to 5000 levels so it
   stays past the limit in every build profile.
2. **`dee8c5ce0` (field-init scope parenting) — turned out NOT to be
   behavioral.** Same `SaveAndRestore`; upstream declares it **after**
   `declareArguments()`, we had it **before**. Both `visit(ClassPropertyNode*)`
   and `visit(ClassPrivatePropertyNode*)` were reordered to match upstream
   exactly, and the Rust mirror with them. **The reorder changes nothing
   observable**, and the "it decides which scope the `arguments` decl lands
   in" reasoning that motivated the row was wrong: `declareArguments()`
   (`SemanticResolver.h:348-353`) puts the `Decl` in
   `argumentsFunc->getScopes().front()` — a *function*-derived scope, chosen
   inside `SemContext::funcArgumentsDecl` — and its `Binding` in the current
   *binding-table* scope, which `SaveAndRestore<LexicalScope *> curScope_`
   does not push. Neither effect reads `curScope_`. Empirically: with the
   oracle rebuilt, all 219 driver-corpus + 13 parser-entry C++ dumps
   (stdout, stderr and exit status) are byte-identical before and after.
   The reorder is kept purely for source fidelity to upstream.

---

## Ported ahead of `static_h`: `private/export-D115669841`

**`5ae5260c8` — "Handle try-catch-finally in CheckImplicitReturn".** **DONE
2026-08-13** (cherry-picked as `9b5025f89`, mirrored by `2253b7331`, upstream
sync task 4). The upstream fix for **`CppDefectsFound.md` item 12** (now
FIXED), which this port found on 2026-08-12: `try/catch/finally` inside a
function aborted the parser-entry resolver, and in Release silently ignored
the finalizer.

**It was NOT in `origin/static_h` when it was ported** — it was taken from the
export branch `private/export-D115669841` and will land in `static_h` later.
That is the one place this tree runs ahead of upstream's mainline, and it
matters for the next sync: when `5ae5260c8` (or its `static_h` rewrite, likely
under a different hash) appears in `60b5c73db..origin/static_h`, it is
**already ported** — compare with `git show <commit> -- lib include | git
patch-id --stable` against `9b5025f89` before assuming otherwise, exactly as
the 11-fix table above does, and do not re-apply it.

Rust-side consequence worth recording: this fix is what deleted
`facade_agreement.rs`'s `PARSER_ENTRY_SKIP`, and the three decisions it adds
to `CheckImplicitReturn` are observable **only** through the parser entry
point (`sema_corpus_parser`), never on the compile path — measured, see
`rust/crates/sema/tests/sema_corpus_parser/MANIFEST.md`.

Task 4 also closed the parser-entry half of the coverage gap the `04f1f53a8`
row below records for the driver corpus. Same mutation survey, run over
`sema_corpus_parser`: **14 of `check_implicit_return.rs`'s 21 decisions had
zero witnesses there** (the 18 of the task-2 survey plus the 3 this fix adds),
and after importing upstream's lit test and a copy of the authored
`implicit-return-shapes.js`, **none does**. Parser corpus **14 (6) → 16 (8)**,
driver corpus **223 (110) → 224 (111)**.

---

## Sync backlog — upstream front-end commits not yet in the port

Everything in `60b5c73db..origin/static_h` touching `lib/{Parser,Sema,AST}`,
`include/hermes/{Parser,Sema,AST}`, `SourceErrorManager`, or the front-end
tests, minus the 11 above. (`b70dd7942` touches `include/hermes/Support` but is
`sh_tryfast_fp_cvt.h` — runtime FP conversion, not a ported component.)

| Upstream | What it does | Port impact |
|---|---|---|
| ~~`04f1f53a8`~~ | Adds `-Xcompile` to the driver **and dumps `mayReachImplicitReturn`** | **DONE 2026-08-13** (`1e3806f47` + `de917f249`). The port's `CheckImplicitReturn` was byte-verified for the first time and **agreed with C++ everywhere**: 219/219 driver-corpus and 13/13 parser-entry dumps identical after mirroring `printFunction`, and zero stdout differences across all 1232 `.js` under `test/Sema`, `test/Parser` and `test/hermes`. Not vacuous — 196 of the driver corpus's 555 `Func`/`StaticBlock` lines carry `noImplicitReturn`. `-Xcompile` was cherry-picked but nothing was built on it; see the deferred `sema-parser-dump` item. **Review round:** making the flag visible also exposed that the standing gate pinned only 7 of `CheckImplicitReturn`'s 18 decisions; the authored corpus file `implicit-return-shapes.js` now pins all 18, and the driver corpus is **220 (110)**. See `sema_corpus/MANIFEST.md`'s survey table. |
| ~~`653e49c60`~~ | Handle Flow `match` in `CheckImplicitReturn` (+61 lines) | **DONE 2026-08-13** (`acf86bf51` + the Rust mirror). `check_implicit_return.rs` gained the `MatchStatement` arm, `check_termination_match_statement` and `is_irrefutable_match_pattern`. Pinned by `sema_corpus_parser/flow-match-implicit-return.js` — the parser corpus, because `90f4a3ac6` makes the driver path reject a match before it can be dumped. Six mutations of the new code, each caught by that one file; see `sema_corpus_parser/MANIFEST.md`. |
| ~~`90f4a3ac6`~~ | Reject Flow `match` when compiling: new `visit(MatchStatement/MatchExpression)` emitting "match statements/expressions are unsupported" under `compile_` | **DONE 2026-08-13** (`502bbc7d3` + the Rust mirror). `resolver/statements.rs`'s `visit_match_statement` and `resolver/expressions.rs`'s `visit_match_expression`, plus children-walk arms for the sixteen match sub-grammar kinds — before this the resolver PANICKED on any `match`. Pinned in both directions: `sema_corpus/flow-match-unsupported.js` (errors present) and `sema_corpus_parser/flow-match-implicit-return.js` (the `compile_` gate, i.e. errors absent). |
| ~~`ca6de21ce`~~ | Parser: check the parsed value of a match object property (`if (!optPattern) return false;`) | **DONE 2026-08-13** (`be443ad10`). **Nothing to change in the port:** its call site is `self.parse_match_pattern_flow()?` and `?` IS the added check, so this port never had the defect. Cited at the site and pinned by `sema_corpus/flow-match-pattern-object-{value,binding}-error.js` plus `parser/tests/upstream_defect_fixes.rs`. |
| `26872f6e9` | Moves the parser-mode semDump unit tests to lit (`test/Sema/parser-mode-*.js`) | Optional: upstream now has real files for the two shapes this port had to author (`sema_corpus_parser/{anon-export-default,with-statement}.js`); they can be replaced with upstream imports. |
| `6fbc3706d` | Backs out `#if 0` around the dead local-eval block → `if ((false))` | Dead in both forms; check the port's comments/citations (`CppDefectsFound.md` item 10b). |
| `8f9e357fd` | Reverts `#if 0` around the dead `arguments` block → `if ((false))` | Same class. |

The two divergences above are **no longer in the backlog**: both were
corrected on 2026-08-13 (plan task 1).

---

## Deferred follow-ups

Work this sync surfaces but deliberately does **not** do. Do not let these
rot — check them at the start of the next sync.

### CLOSED (2026-08-13, task 2 step 0) — import upstream's fuller `test/Sema/class-field-class-expr.js`

Done in `885a7f300`, exactly as prescribed below, and re-proved: with the
`cur_scope` switch deleted from `visit_class_private_property` only,
`sema-dump` on the imported file panics at `dump_context.rs:251` ("not all
scopes were visited, left: 2, right: 3", exit 101) and
`sema_differential_s0` reports a stdout mismatch for it; the pre-widening
55-line file exits 0 under the same mutant. Restored: 219 (109) / 13 (5).
The original entry follows unchanged, for the record.



The divergence check compares `lib include` only, so it never looked at the
lit test `dee8c5ce0` added. Upstream's version of
`test/Sema/class-field-class-expr.js` is 73 lines to our 55: besides `x` and
`static y` it also declares `#px = class {}` and `static #py = class {}`,
i.e. it is a **class expression inside a *private* field initializer**, which
nothing else anywhere exercises for **scope parenting**. (To be precise: the
`visit(ClassPrivatePropertyNode *)` `_value` branch itself *is* covered —
`sema_corpus/private-members.js`, `field-value-arguments-error.js`,
`static-blocks.js`, lit `test/Sema/private-names.js`. What has no pin on
either side is that site's half of `dee8c5ce0`.)

**This is not hypothetical.** The task-1 reviewer deleted the `cur_scope`
switch from *only* `visit_class_private_property` in the Rust port: the entire
`hermes-sema` suite — including the 219 + 13 differential — stayed green,
while upstream's 73-line input made the mutated port panic at
`dump_context.rs:251` with `not all scopes were visited, left: 2, right: 3`
(the defect-4 signature). So that half of the fix can be removed today and
nothing notices.

Importing is free: the unmutated Rust `sema-dump` output on that input is
byte-identical to our `hermesc`'s, which is byte-identical to upstream's own
CHECK lines — a pure coverage gain with no expectation churn. Not done in
task 1 only because it changes corpus *content*, and task 1's auditability
rested on provably changing no dump bytes. **Do it first in task 2.**

### OPEN — retire `tools/sema-parser-dump`

`04f1f53a8` adds **`-Xcompile`** to the stock driver, so
`hermesc -Xcompile=false -dump-sema` now produces the parser-mode (i.e.
`compile = false`) sema dump. That is exactly, and only, what the in-repo
`tools/sema-parser-dump/` oracle was written to provide (S4a §2.1) — it exists
because no upstream binary could reach `resolveASTForParser` + `semDump`.

Retiring it would delete a local-only C++ tool we otherwise maintain forever,
and would let the parser-entry corpus run against a stock upstream binary
instead of one of our own making — a strictly stronger oracle.

**Deliberately deferred (2026-08-13):** the same commit changes the sema dump
format, which reds all 232 corpus comparisons until the port matches. Doing
both at once would mean the dump-format change and an oracle swap are in
flight together, so a mismatch could not be attributed to either. Land the
dump-format port first, get the gates green, then swap the oracle as its own
change.

**Measured 2026-08-13 (task 2, information only — no action taken): the swap
is NOT free.** With `04f1f53a8` cherry-picked, `hermesc -Xcompile=false
-dump-sema` vs `sema-parser-dump` over the 13 parser-entry corpus files gives
**6 differing**, all of the predicted shape: same exit status (2), but the
driver emits an EMPTY stdout where `sema-parser-dump` emits the dump —
`error-arrow-rewrite-then-error.js` (0 vs 521 B),
`error-break-outside-loop.js` (0/100), `error-continue-outside-loop.js`
(0/356), `error-invalid-assignment-lvalue.js` (0/188),
`import-assertions-compile-false.js` (0/265), `module-imports.js` (0/530).
That is the driver's dump-suppression-on-error, and it is exactly the
behavior 8 of the 13 corpus files exist to pin. So retiring the tool would
either lose the error-path parser-entry coverage or need those files
restructured; decide that deliberately rather than assuming a drop-in swap.

**Task 3 (2026-08-13) added a 14th file and one more flag, information only.**
`flow-match-implicit-return.js` was imported into the parser-entry corpus, and
`sema-parser-dump` learned `-Xparse-flow-match`/`--Xparse-flow-match` to carry
it. On THAT file the swap *is* free: `hermesc -Xcompile=false -dump-sema
-parse-flow -Xparse-flow-match` and `sema-parser-dump -parse-flow
--Xparse-flow-match` produce byte-identical stdout (11615 B, exit 0), because
it is an error-free file and so the driver's dump-suppression never engages.
The 6 differing files above are still the blocker; this one just is not a
seventh. Retiring the tool now also means re-checking the two flags it
understands rather than one.

**When doing it, check:** that `-Xcompile=false` output is byte-identical to
`sema-parser-dump`'s for all 14 parser-entry corpus files (it is not — see
the measurement above; both call the same two library entries, but the driver
wraps them differently: `sema-parser-dump` dumps *unconditionally* even when
errors were reported, whereas the driver suppresses the dump on errors, and
`-Xcompile=false` also refuses `-commonjs`); that the harness's
per-file `// FLAGS:` mechanism can carry `-Xcompile=false`; and that
`tools/CMakeLists.txt`, the corpus MANIFESTs, `CONTRIBUTING.md` and
`CppDefectsFound.md` items 2–3 (whose repros invoke the tool by name) are all
updated together.

## Updating this file

When a sync lands: move the ported rows out of the backlog, update "Ported
through", and — if the port's C++ tree is brought to a plain upstream commit
rather than fork-point-plus-cherry-picks — say so, because that is the state
this document exists to record. Re-run the divergence check
(`git patch-id --stable` over `lib include`) for anything cherry-picked ahead
of upstream landing it; it caught two real regressions here.
