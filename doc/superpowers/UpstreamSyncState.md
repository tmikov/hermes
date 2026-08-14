# Upstream C++ sync state

**What this file is:** the record of *which upstream C++ state the Rust port
mirrors*. The port is validated by byte-for-byte differential testing against
binaries built from this repo's C++ tree, so "which upstream commit are we
bug-for-bug equal to" is a load-bearing fact, not trivia. Update it whenever
upstream fixes are ported.

---

## Ported through (as of 2026-08-14)

| | |
|---|---|
| **Base** | **`origin/static_h` @ `14112ce36`** — the port's history was reconstructed directly on top of it on 2026-08-14 |
| **Local C++ delta** | **exactly one**: a 3-line cosmetic re-wrap in `lib/Sema/SemanticResolver.cpp` (the `ExportAllDeclarationNode` error call, kept in its pre-`4aa3006f8` clang-format shape). Upstream's wording, our wrapping. |
| **Local C++ additions** | the five oracle tools `tools/{js-lexer-dump,json-parse-dump,preparse-dump,sema-parser-dump,parse-bench}/` and their rows in `tools/CMakeLists.txt`. These are ours and are maintained here; nothing upstream provides them. |
| **Everything else under `lib/`, `include/`, `test/`, `unittests/`** | byte-identical to `14112ce36` |
| **Pre-squash history** | tag `rust-history-2026-08-14` and branch `rust-presquash-backup`, both `550c5db8f` |

**The port's C++ tree is a single upstream commit again**, and that is the
headline this document now carries. Until 2026-08-14 it was "fork point
`60b5c73db` plus nineteen cherry-picks", one of which (`5ae5260c8`) had not
landed upstream — so the tree ran one commit *ahead* of `static_h`'s mainline.
Both halves of that are gone:

- Eighteen of the nineteen cherry-picks were already in `origin/static_h`.
- The nineteenth, `5ae5260c8`, **landed upstream as `594e9c6a1`**
  ("Handle try-catch-finally in CheckImplicitReturn (#2131)") — the same rule
  reached by a different structure: our version added a
  `checkTerminationFinalizer` helper, upstream inlined the finalizer half into
  `checkTerminationTryStatement`. The port keeps its helper (semantics-
  preserving) and the parser-entry gate, **17 (9)**, is the evidence the two
  C++ forms agree. **Nothing in this tree is now pending upstream landing.**

So the next sync is the ordinary procedure: fast-forward the C++ tree to a
newer `origin/static_h`, rebuild the oracle, re-run the gates, run
`citations -- remap`. There is nothing to re-apply and nothing to carry.

**How the branch was reconstructed.** The 552 chronological commits were
replaced by 11 subsystem commits (support / AST / lexer / JSON / parser core /
dialects / pre-lazy / sema / oracle tools / local C++ deltas / docs) plus a
citation re-point, based on `origin/static_h` @ `14112ce36`. Correctness was
enforced by a tree-equality invariant rather than by trusting the process:
every path the port owns is byte-identical to the pre-squash tip except the
`cpp:` citation line numbers, and every other path is byte-identical to
`origin/static_h`. Both halves were checked mechanically. The plan is
`plans/2026-08-14-squash-onto-static-h.md`.

### Superseded (pre-2026-08-14): the "fork point plus cherry-picks" record

Kept because what follows — the 11-defect-fix table, the exclusion rules, the
backlog — is the record of *how* the port reached parity, and the exclusion
rules in particular are still live policy. The state it describes is no longer
current.

| | |
|---|---|
| **Fork point from `static_h`** | **`60b5c73db`** ("Deploy 0.316.0 to xplat") |
| **Plus** | the 11 defect fixes cherry-picked 2026-08-10 (below) |
| **Plus** | `04f1f53a8` (`-Xcompile` + dump `mayReachImplicitReturn`), cherry-picked 2026-08-13 as `1e3806f47`, mirrored in the port by `de917f249` |
| **Plus** | the three Flow-`match` fixes `653e49c60`/`90f4a3ac6`/`ca6de21ce`, cherry-picked 2026-08-13 as `acf86bf51`/`502bbc7d3`/`be443ad10`, mirrored in the port by the task-3 commit |
| **Plus** | `5ae5260c8` (try-catch-finally in `CheckImplicitReturn`, `CppDefectsFound.md` item 12), cherry-picked 2026-08-13 as `9b5025f89`, mirrored in the port by `2253b7331` — was **from `private/export-D115669841`, not from `static_h`**. **Landed upstream 2026-08-14 as `594e9c6a1`; this row is history.** |
| **Plus** | `6fbc3706d` + `8f9e357fd` (back out the two `#if 0` guards around permanently-dead blocks, restoring `if ((false))`), cherry-picked 2026-08-13 as `2fde4d88c`/`de41b2056` — no Rust behavior change; the port mirrors the new nesting and its citations were re-verified |
| **Plus** | `26872f6e9` (move the parser-mode semDump unit tests to lit), cherry-picked 2026-08-13 as `c5266734b`; the port replaced its two AUTHORED parser-entry corpus files with upstream's lit files |
| **Upstream `static_h` HEAD at time of writing** | `2d3e9018b` (2026-08-13) |
| **Commits between fork point and upstream HEAD** | 147 (105 of them predate the local `static_h` ref at `5dfe740ad`) |

> **No longer true as of 2026-08-14** — see the current state above. The tree
> IS equal to a single upstream commit (`14112ce36`) apart from one cosmetic
> re-wrap, and it no longer runs ahead of upstream: `5ae5260c8` landed as
> `594e9c6a1`. The paragraph is kept because the nineteen-cherry-pick
> composition is what the tables below account for.

The port's C++ tree is **not equal to any single upstream commit**, and this is
the fact this file exists to state plainly: it is the fork point `60b5c73db`
plus **nineteen** cherry-picks — the 11 defect fixes of 2026-08-10 (two of
them corrected on 2026-08-13), then `04f1f53a8`, the three Flow-`match` fixes,
`5ae5260c8`, `6fbc3706d`, `8f9e357fd` and `26872f6e9`. Eighteen of those
nineteen are in `origin/static_h`; **`5ae5260c8` is not** — it came from
`private/export-D115669841`, so the tree runs one commit AHEAD of upstream's
mainline (see the section below, and do not re-apply it at the next sync).

### Residual gap, and the commits that touch the watched paths but are NOT ported

> **Re-based 2026-08-14.** The arithmetic below is against `2d3e9018b`; the
> tree is now on `14112ce36`, 7 commits later, and the C++ tree is upstream's
> rather than fork-point-plus-picks — so "18 ported + 13 excluded = 31" is a
> historical accounting, not a live gap. **The three exclusion rules are still
> live policy** and are the reason this section is kept: the FlowChecker and
> the typed-Flow dialect are still not ported components, and the FlowChecker
> table below is still the starting backlog for whenever they are.

Re-derive the gap with the path filter this document has always used:

```bash
git log --oneline 60b5c73db..origin/static_h -- \
  lib/Parser lib/Sema lib/AST \
  include/hermes/Parser include/hermes/Sema include/hermes/AST \
  lib/Support/SourceErrorManager.cpp \
  include/hermes/Support/SourceErrorManager.h \
  test/Parser test/Sema test/AST unittests/AST unittests/Parser
```

As of 2026-08-13 that yields **31** of the range's 147 commits. **18 are
ported** — the 11 defect fixes, plus `04f1f53a8`, the three Flow-`match`
fixes, `26872f6e9`, `6fbc3706d` and `8f9e357fd` — and the backlog table below
has no open rows. The other **13 are deliberately excluded.**

That last sentence is the correction. An earlier revision of this paragraph
said the unported remainder was "all irrelevant to the front end (VM, GC,
debugger, JSI, build)". **That was false**: these 13 land squarely inside the
watched paths, and a re-derivation that trusted the old wording would surface
them as unaccounted. They are excluded under three rules, listed with hashes
so the next sync can check them off mechanically.

**Exclusion rule 1 — the FlowChecker and the typed-Flow dialect are not
ported components.** They are a distinct C++ component (`lib/Sema/FlowChecker*`,
`lib/Sema/FlowContext.cpp`, `lib/Sema/FlowTypesDumper.cpp`,
`include/hermes/Sema/FlowContext.h`, tests under `test/Sema/flow/`) that this
port does not cover: `rust/crates/*/src/` contains no FlowChecker
implementation, and the name occurs there only in prose marking its absence
(`sema/src/lib.rs:107`, `sema/src/resolve.rs:40-41`, `sema/src/dump.rs:141`).
Ten commits, all FlowChecker-only in the watched paths:

| Upstream | What it does |
|---|---|
| `3bdfce556` | `ExactObjectType::getIndexer()` returns a reference (#2112) |
| `d88208625` | distinguish generic class vs. alias handling |
| `496ea026e` | implement object type indexers |
| `c8dab081f` | improve array inference |
| `451689831` | unify the destructuring passes |
| `7f244a2c2` | restrict equality comparison operations |
| `86777d198` | set `ClassConstructorType` as the `new.target` type |
| `28b32e9c5` | visit `throw` statements |
| `e95ea2592` | variance sigils in exact object types |
| `935629540` | restrict `delete` to `any` |

Untyped Sema is what this port mirrors; the typed dialect is a **future**
component. When it is started, this table is its starting backlog.

**Exclusion rule 2 — `transformASTForCompilation` lowerings are not a ported
component.** `02ceb114f` ("Fix named function expression immutable binding in
async generator") touches `lib/AST/AsyncGenerator.cpp` — one argument,
`nullptr` → `funcExpr->_id` — but `AsyncGenerator.cpp` is a compile-pipeline
lowering run from `transformASTForCompilation` (`lib/AST/TransformAST.cpp:26`)
on the BCGen/driver side, and it is dead by default:
`Context::enableAsyncGenerators_{false}` (`include/hermes/AST/Context.h:276`),
set only from `HBC.cpp`/`BCProviderFromSrc.cpp`/`eval.cpp`. The resolver's
*only* interaction with async generators is the "async generators are
unsupported" error (`SemanticResolver.cpp:1722-1725`), which the port already
mirrors (`resolver/functions.rs:645`, pinned by
`sema_corpus/error-async-generator.js`). Nothing in that commit reaches ported
code.

**Exclusion rule 3 — netted out by back-outs this sync already ported.**
`ed4610b8c` and `7dd72d456` are the two `#if 0` guards; `6fbc3706d` and
`8f9e357fd` back them out, and both back-outs ARE ported (backlog table
below). The pairs cancel, so the originals need no separate action — and must
not be applied, since this tree never carried them in the first place.

Arithmetic: 18 ported + 13 excluded = 31 watched-path commits; the remaining
116 of the 147 touch no watched path at all (VM, GC, debugger, JSI, build).
So the gap is exactly: *fork point + 19 cherry-picks* vs *`2d3e9018b`*,
differing only in components this port does not cover, plus the one commit
where the port leads.

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
   (`SemanticResolver.h:350-355`) puts the `Decl` in
   `argumentsFunc->getScopes().front()` — a *function*-derived scope, chosen
   inside `SemContext::funcArgumentsDecl` — and its `Binding` in the current
   *binding-table* scope, which `SaveAndRestore<LexicalScope *> curScope_`
   does not push. Neither effect reads `curScope_`. Empirically: with the
   oracle rebuilt, all 219 driver-corpus + 13 parser-entry C++ dumps
   (stdout, stderr and exit status) are byte-identical before and after.
   The reorder is kept purely for source fidelity to upstream.

---

## No longer ahead of `static_h`: `5ae5260c8` landed as `594e9c6a1`

**`5ae5260c8` — "Handle try-catch-finally in CheckImplicitReturn".** **DONE
2026-08-13** (cherry-picked as `9b5025f89`, mirrored by `2253b7331`, upstream
sync task 4). The upstream fix for **`CppDefectsFound.md` item 12** (now
FIXED), which this port found on 2026-08-12: `try/catch/finally` inside a
function aborted the parser-entry resolver, and in Release silently ignored
the finalizer.

**CLOSED 2026-08-14.** It was taken from the export branch
`private/export-D115669841` because `origin/static_h` did not have it. It has
since landed there as **`594e9c6a1`** — *"Handle try-catch-finally in
CheckImplicitReturn (#2131)"* — and `lib/Sema/CheckImplicitReturn.cpp` in this
tree is now upstream's file verbatim. **The prediction in the old wording was
right down to the detail:** it landed under a different hash and as a rewrite
rather than the same patch. Upstream inlined the finalizer half into
`checkTerminationTryStatement`; our version had added a
`checkTerminationFinalizer` helper. The restructure is semantics-preserving,
so the Rust port keeps its helper and was not changed.

Two things follow, both already done, both worth knowing at the next sync:

- **Nothing is pending, so nothing must be re-applied.** The "compare with
  `git patch-id --stable` before assuming otherwise" instruction has been
  discharged; there is no export-branch content left in this tree.
- **The citations into that file moved.** `594e9c6a1` deleted the function
  eight of them named, so `citations -- remap` declined them by name and they
  were re-pointed by hand. Five of those eight turned out to have been *wrong
  since before the rewrite* — blessed at trust-on-first-use against text that
  happened not to move. Upstream's edit is what surfaced them.

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
| ~~`26872f6e9`~~ | Moves the parser-mode semDump unit tests to lit (`test/Sema/parser-mode-*.js`) | **DONE 2026-08-13** (`c5266734b`, sync task 5). Upstream now has real files for the two shapes this port had to author, so `sema_corpus_parser/{with-statement,anon-export-default}.js` were REPLACED by verbatim imports of `test/Sema/parser-mode-{with-statement,export-default-anon}.js`. Both RUN lines are `-Xcompile=false -dump-sema`, which is what that corpus's pair already is, so no `// FLAGS:` line is needed and the copies are byte-identical to their lit twins. Corpus unchanged at **16 (8)** — one for one. Coverage re-proved by mutation, and the `WithStatement`-arm gap the re-check surfaced is recorded; see `sema_corpus_parser/MANIFEST.md`'s "Upstream sync task 5" section. **Task 6 then CLOSED that gap**: a 17th corpus file, `implicit-return-with-statement.js`, puts `with` inside a function so the implicit-return analysis actually runs over it, taking the parser corpus to **17 (9)**. |
| ~~`6fbc3706d`~~ | Backs out `#if 0` around the dead local-eval block → `if ((false))` | **DONE 2026-08-13** (`2fde4d88c`). Dead in every spelling, so **no Rust behavior change**; the port's `resolver/functions.rs` mirrors the new `if false { if … }` nesting for source fidelity. This tree never carried the `#if 0` (it was never cherry-picked), so the cherry-pick conflicted and was resolved to upstream's post-back-out text — verified line-for-line against `6fbc3706d:lib/Sema/SemanticResolver.cpp`, NOT by `patch-id` (the deltas differ because the starting states differ). Citations into the block re-verified: it is now `SemanticResolver.cpp:1960-1967`. |
| ~~`8f9e357fd`~~ | Reverts `#if 0` around the dead `arguments` block → `if ((false))` | **DONE 2026-08-13** (`de41b2056`). Same class, same resolution and same verification; the site is now `SemanticResolver.cpp:2454-2461` and `resolver/declarations.rs` already had the matching nesting. |

**The backlog is empty.** The two divergences above are also no longer in it:
both were corrected on 2026-08-13 (plan task 1).

---

## The wide sweep after the sync (2026-08-13)

Re-run at the end of sync task 5, the standing whole-`test/` differential the
roadmap describes: `hermesc -dump-sema` vs a **debug** `sema-dump`, stdout +
stderr + exit status, over every `.js` in `test/{Parser,IRGen,BCGen,Optimizer,
hermes,AST,Driver,RA}`.

```
files 1420   identical 1410   mismatch 3   panic 7
```

The population grew **1418 → 1420**: the two files upstream `ca6de21ce` added
under `test/Parser/flow/match/` entered the swept directories with task 3's
cherry-pick, and **both are byte-identical**, which is why identical went
1408 → 1410. **Zero new residuals** — every one of the ten is a pre-existing,
individually classified item:

| Residual | Files | Classification |
|---|---|---|
| `$SHBuiltin.moduleFactory` panic (`resolver/calls.rs:349`, "needs `visitModuleFactory` — S4 modules") | `test/BCGen/HBC/xmod-requires-opt.js`, `test/Optimizer/xmod-{builtins,require-cse,requires-opt,requires-opt-extension}.js`, `test/hermes/xmod-exec-require{,-bad-func}.js` | 7 — the standing S4b deferral; message checked on each, all identical |
| Regex validation | `test/AST/regexp.js` | needs `lib/Regex/`, its own future component; the port exits 0 with a dump where hermesc reports 4 `Invalid regular expression` errors |
| Notes dropped per house style | `test/Parser/es6/import-error.js` | stderr only — hermesc's extra `note: first usage of name` |
| Error-recovery gap | `test/Parser/optional-chaining-error.js` | stderr only — 4 diagnostics vs the port's 3 |

**Reconciliation with task 2's sweep.** Task 2 ran a *different* dir set
(`test/Sema` + `test/Parser` + `test/hermes`, 1232 files) and found 2 stdout
diffs + 3 crashes. Both figures are consistent: its 3 crashes are
`test/Sema/xmod-errors.js` plus the two `test/hermes` `xmod` files, and
`test/Sema` is not one of the eight directories above. Task 5 also swept
`test/Sema` separately — **242 files, 241 identical, 1 panic**
(`xmod-errors.js`, the same S4b deferral) — which pins the two new
`test/Sema/parser-mode-*.js` files as byte-identical on the driver pair too,
on top of the parser pair the corpus uses. Union of both sweeps: 1662 files,
1651 identical, 3 mismatch, 8 panic.

## Citation drift found and repaired (2026-08-13, task 5)

Worth recording because it is a recurring defect class in this repo and this
round found it had reopened. Tasks 1-4 of this sync changed eight C++ files
that the port cites by line number, and only the citations those tasks
*wrote* were correct afterwards; pre-existing ones drifted silently — e.g.
`resolver/mod.rs`'s `SemanticResolver.cpp:1762-1765` for the "'use strict'
not allowed inside function with non-simple parameter list" error had been
15 lines stale since task 3, and this task's own cherry-pick added one more.

The repair was blame-aware, not a flat shift: for every citation, the commit
that last wrote that LINE was found with `git blame`, the cited C++ file was
diffed from *that* commit to HEAD, and the numbers were mapped through the
resulting line map. A flat shift would have corrupted the citations tasks 1-4
wrote correctly (e.g. `statements.rs`'s `SemanticResolver.cpp:1615-1622` for
`visit(MatchStatementNode *)`, which is right and must not move). 436 tokens
were remapped across `SemanticResolver.{cpp,h}`, `CheckImplicitReturn.cpp`,
`SemContext.cpp`, `CompilerDriver.cpp`, `JSONParser.h`,
`JSParserImpl-flow.cpp` and `sema-parser-dump.cpp`; ~20 more were fixed by
hand where the cited line had been deleted or where the citation was already
wrong before this sync's baseline. Dated plan/spec documents under
`doc/superpowers/{plans,specs}/` were left alone, per this repo's convention
that they are historical records.

**Since 2026-08-14 none of this is done by hand:** the checker below is
checked in, and `citations -- remap` does the mapping (hash-proved rather than
blame-guessed) in one command. Run it after every C++-touching commit.

## Deferred follow-ups

Work this sync surfaces but deliberately does **not** do. Do not let these
rot — check them at the start of the next sync.

### CLOSED (2026-08-14) — a checked-in citation checker

**Built and blessed 2026-08-14**, plan
`doc/superpowers/plans/2026-08-14-citation-checker.md`. It lives in
`rust/crates/tools/src/citations/` (the `citations` binary of the unpublished
`tools` crate), with `citations.toml` (resolution config),
`citations.snapshot.json` (the blessed hashes) and the standing test
`crates/tools/tests/citations.rs` beside it in `rust/crates/tools/`.
Documentation: **`rust/crates/tools/src/citations/README.md`**; the workflow is
in `rust/CONTRIBUTING.md`.

**What it does.** Three modes:

- `check` — re-hashes every cited C++ span against the working tree and names
  the sites that moved. This is what the standing `cargo test` runs.
- `remap` — the mechanical repair the item below asked for, and better than
  blame-aware: it maps a stale citation through `git diff` from the blessed
  commit, and rewrites the digits **only when the blessed hash reproduces both
  at the base coordinates and at the destination**. Everything else is
  declined by name, with the reason. That is what keeps a C++-only edit cheap.
- `bless` — re-records the tree after a reviewed change.

**Scale, as blessed:** 3183 citation sites, 53 C++ files, 84 Rust files.
The scanner covers every form in the tree — `cpp:NNN`, `File.cpp:NNN[-MMM]`,
path-qualified, `// C++ NNN`, `NNNN in File.cpp`, comma continuations,
citations wrapped across two comment lines, and the implicit-file `:NNN`.
Resolution is explicit in `citations.toml`, never guessed from a module
header's prose (the recommendation below assumed the header convention was
parseable; it is not — headers vary, wrap, and are sometimes absent).

**Two findings worth keeping.** (1) The population was **3183**, not the
~1,600 estimated below: the estimate both double-counted (a bare-`cpp:` grep
matches the tail of a qualified citation) and missed the commonest spelling
entirely, the inline `// C++ NNN` form, 1278 sites / 42% of the total.
(2) Extending the scanner to the colon-less `NNNN in File.cpp` banner shape
immediately found **34 citations in `JSParserImpl-flow.cpp` short by exactly
3**, caused by two of *this sync's own* cherry-picks (`bfeeb404f` +1,
`be443ad10` +2), verified correct when written. They had been rotting in-tree,
unreported, because no tool could see that shape — the same failure this item
was opened about, caught mid-flight.

**Known debt, deliberately not repaired** (measured, and routed to a future
repair decision): ≥20 citations that resolve and range-check but name the
wrong lines — a **floor**, measured over 15.5% of sites by a heuristic that
excluded the commonest spelling, so the true figure is higher and unmeasured;
plus **23** `(flow.cpp:NNNN-MMMM)` sibling doc citations short by exactly 2
that `check` and `remap` **structurally cannot see** (blessed at
trust-on-first-use, span never moved since), plus ~12 assorted further
mismatches. **Drift is not wrongness:** `remap` repairs 0 of the 20 known-wrong
sites, so a repair decision cannot lean on it. See the README's "Known citation
debt" section.

**At the next sync, `remap` is part of the routine** — see "Updating this
file" at the bottom.

The original item, unchanged, follows as the record of what motivated it.

### The original OPEN item (recorded 2026-08-13, task 6) — for the record

**The problem.** The Rust sources cite C++ by line number, roughly **1,600
citation tokens**: ~367 of the qualified `File.cpp:NNN` form and ~1,270 bare
`cpp:NNN` (which resolve against the module's own "Port of `<file>`" header).
Every one of them silently rots the moment upstream moves a line. This is a
**recurring** defect class, not a one-off: it has reopened **three times
across two plans**. Task 5 of this sync alone remapped **436** tokens, and to
do that it had to rebuild blame-aware remapping tooling *as a throwaway* —
the same tooling the previous round had also written and thrown away.

**Recommended shape** (the reviewer's, unchanged):

- A checked-in tool, sited next to the survey harness at
  `rust/crates/sema/tests/implicit_return_survey/` (same precedent: a
  verification harness that used to be a scratchpad script and is now
  checked in).
- A generated snapshot mapping every citation site to a **hash of the cited
  C++ line span's text** — not the line numbers, so pure line-number drift is
  detected as staleness and pure reformatting elsewhere in the file is not.
- A standing `cargo test` that fails, **naming the stale sites**, whenever a
  cited span's text changes.
- A `--rebless` / auto-remap mode that repairs the numbers mechanically,
  blame-aware, exactly as task 5 did by hand.
- Bare `cpp:NNN` tokens resolve through each module's existing
  "Port of `<file>`" header convention, so no citation has to be rewritten to
  adopt the tool.

**Cost and payoff.** Estimated **half a day to a day** to build and bless.
Runtime sub-second (hashing spans of a handful of C++ files). Ongoing
maintenance is **one re-bless step** in any task that touches C++.

**Make the failure message point at the remap tool**, so a C++-only edit
stays cheap: the common case is "you moved C++, run `--rebless`", and a
checker that only says "stale" would tax exactly the edits that should be
free.

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

**Task 5 (2026-08-13) re-measured at 16 files: still 6 differing, unchanged.**
The two files task 5 imported from upstream (`parser-mode-with-statement.js`,
`parser-mode-export-default-anon.js`) are error-free, so the driver's
dump-suppression never engages and `hermesc -Xcompile=false -dump-sema`
matches `sema-parser-dump` byte-for-byte on both — as do the two
implicit-return files task 4 added. The blocker is unchanged and is still the
same six error-path files: **6 of 16 differ**, all `stdout 0 B` from the
driver against a full dump from the tool, same exit status (2). This item
stays **OPEN**.

**Task 6 (2026-08-13) re-measured at 17 files: still 6 differing, unchanged.**
The 17th file, `implicit-return-with-statement.js` (the `WithStatement`-arm
pin), is error-free, so the driver's dump-suppression never engages:
`hermesc -Xcompile=false -dump-sema` and `sema-parser-dump` produce
byte-identical stdout on it (1910 B, empty stderr, exit 0 both). **6 of 17
differ**, the same six error-path files.

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
`sema-parser-dump`'s for all 17 parser-entry corpus files (it is not — see
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
this document exists to record. **That state was reached on 2026-08-14**
(`14112ce36`), so the next sync starts from a plain fast-forward, not from a
cherry-pick ledger. Re-run the divergence check
(`git patch-id --stable` over `lib include`) for anything cherry-picked ahead
of upstream landing it; it caught two real regressions here.

**Every cherry-pick moves C++ lines, so the citations go with it.** After any
commit that changes the C++ tree — the same routine step as rebuilding the
oracle, and per-task, not once at the end:

```bash
cargo run --manifest-path rust/Cargo.toml -p tools --bin citations -- remap
cargo run --manifest-path rust/Cargo.toml -p tools --bin citations -- check
```

`remap` rewrites the digits of everything that merely shifted and declines,
by name, anything whose cited text changed; those are read by hand and
re-recorded with `… -- bless`. Commit
`rust/crates/tools/citations.snapshot.json` alongside the sync commit. The
citation drift documented above (436 tokens remapped by hand in the 2026-08-13
sync, plus the 34 that were invisible until 2026-08-14) is exactly what this
step exists to prevent, and skipping it puts the drift back.
