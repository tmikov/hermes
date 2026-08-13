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
| **Upstream `static_h` HEAD at time of writing** | `2d3e9018b` (2026-08-13) |
| **Commits between fork point and upstream HEAD** | 147 (105 of them predate the local `static_h` ref at `5dfe740ad`) |

The port's C++ tree is **not equal to any single upstream commit**: it is the
fork point plus eleven cherry-picks. The 136 other commits in
`60b5c73db..origin/static_h` are unported, but all but seven are irrelevant to
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
| `dee8c5ce0` class-expr scope parenting in field inits | `48d221fb2` | **DIFFERS (behavioral)** |
| `4aa3006f8` export module-mode wording | `4193b558a` | differs (cosmetic wrap only) |
| `304c1533c` JSONParser recursion limit | `0b8bbd1fc` | **DIFFERS (behavioral)** |
| `87677f148` stable-sort buffered diagnostics | `7805e2103` | identical |
| `88ca314ed` promoter dead code | `ffcdbdd52` | identical |
| `91f1222dd` JSX member-expression attr names | `51035e8c2` | identical |
| `8d786acbe` match binding-pattern crash | `bfeeb404f` | identical |

Comparison method: `git show <commit> -- lib include | git patch-id --stable`,
which ignores line-number drift, so "identical" means the source change really
is the same.

**The two behavioral divergences matter** — upstream revised these fixes before
landing them, so the port currently mirrors a variant that upstream no longer
has:

1. **`304c1533c` (JSON recursion limit).** Upstream landed **4× the
   `JSParserImpl` limits off Windows** (a JSON nesting level costs far less
   stack): `HERMES_LIMIT_STACK_DEPTH` → **512**, default → **4096**; Windows
   values unchanged. Our cherry-pick used the un-scaled 128/1024. The Rust
   port mirrors ours.
2. **`dee8c5ce0` (field-init scope parenting).** Same `SaveAndRestore`, but
   upstream places it **after** `declareArguments()`, we placed it **before**.
   `declareArguments()` inserts `arguments` into the *current* binding-table
   scope, so the placement decides whether that decl lands in the class scope
   (upstream) or the initializer function's body scope (ours). The Rust port
   mirrors ours.

---

## Pending: `private/export-D115669841`

**`5ae5260c8` — "Handle try-catch-finally in CheckImplicitReturn".** Not yet in
`origin/static_h`; will land. This is the upstream fix for
**`CppDefectsFound.md` item 12** (OPEN), which this port found on 2026-08-12:
`try/catch/finally` inside a function aborts the parser-entry resolver, and in
Release silently ignores the finalizer.

---

## Sync backlog — upstream front-end commits not yet in the port

Everything in `60b5c73db..origin/static_h` touching `lib/{Parser,Sema,AST}`,
`include/hermes/{Parser,Sema,AST}`, `SourceErrorManager`, or the front-end
tests, minus the 11 above. (`b70dd7942` touches `include/hermes/Support` but is
`sh_tryfast_fp_cvt.h` — runtime FP conversion, not a ported component.)

| Upstream | What it does | Port impact |
|---|---|---|
| `04f1f53a8` | Adds `-Xcompile` to the driver **and dumps `mayReachImplicitReturn`** | **Largest.** Every `Func`/`StaticBlock` dump line gains ` mayReachImplicitReturn` or ` noImplicitReturn`, so all 219 driver-corpus + 13 parser-entry comparisons mismatch until `dump_context.rs` matches. The port already *computes* the flag but never dumped it, so this byte-verifies the port's `CheckImplicitReturn` for the first time. `-Xcompile=false` also gives the stock driver a parser-mode dump, which may retire the in-repo `tools/sema-parser-dump` oracle. |
| `653e49c60` | Handle Flow `match` in `CheckImplicitReturn` (+61 lines) | Port `check_implicit_return.rs`. |
| `90f4a3ac6` | Reject Flow `match` when compiling: new `visit(MatchStatement/MatchExpression)` emitting "match statements/expressions are unsupported" under `compile_` | Port to the resolver; parser mode still resolves. |
| `ca6de21ce` | Parser: check the parsed value of a match object property (`if (!optPattern) return false;`) | Port to the flow-match parser — same class as `8d786acbe`. |
| `26872f6e9` | Moves the parser-mode semDump unit tests to lit (`test/Sema/parser-mode-*.js`) | Optional: upstream now has real files for the two shapes this port had to author (`sema_corpus_parser/{anon-export-default,with-statement}.js`); they can be replaced with upstream imports. |
| `6fbc3706d` | Backs out `#if 0` around the dead local-eval block → `if ((false))` | Dead in both forms; check the port's comments/citations (`CppDefectsFound.md` item 10b). |
| `8f9e357fd` | Reverts `#if 0` around the dead `arguments` block → `if ((false))` | Same class. |

Plus the two behavioral divergences above, which are *regressions against
upstream* rather than new work.

---

## Deferred follow-ups

Work this sync surfaces but deliberately does **not** do. Do not let these
rot — check them at the start of the next sync.

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

**When doing it, check:** that `-Xcompile=false` output is byte-identical to
`sema-parser-dump`'s for all 13 parser-entry corpus files (it should be — both
call the same two library entries, but the driver wraps them differently:
`sema-parser-dump` dumps *unconditionally* even when errors were reported,
whereas the driver historically suppresses the dump on errors, and
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
