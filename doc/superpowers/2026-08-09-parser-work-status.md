# Parser work: status, findings, and how to resume

**Date:** 2026-08-09. Recovery document for work done 2026-08-03..09.

This file is tracked in git deliberately. The working notes it summarizes live
in `.superpowers/sdd/2026-08-03-hermes-parser-native/` (37 markdown files:
a ledger, per-task briefs and reports, and the investigation write-ups), and
that directory is **gitignored** — it will not survive `git clean -fdx`.
Everything below is the part worth keeping.

## Branches

All three branch from `9aaccbe5e` (the `n-api` tip) and share no commits with
each other.

| Branch | Commits | Worktree | Intent |
| --- | --- | --- | --- |
| `sema-implicit-return-fixes` | 2 | `~/work/hermes-sema-implicit-return` | Upstream PR |
| `hermes-parser-wasm-fixes` | 2 | `~/work/hermes-parser-wasm-fixes` | Upstream PR |
| `parser-native` | 41 | `~/work/hermes-parser-native` | The fork; not upstreamable as-is |
| `parser-native-phase-timing` | 1 | — | Profiling probes, kept for future perf work |

### `sema-implicit-return-fixes`

Two genuine bugs in `lib/Sema/CheckImplicitReturn.cpp`. `resolveASTForParser`
runs with `compile_` false, so `SemanticResolver` skips transforms that the
compile path performs, and `CheckImplicitReturn` then meets node shapes it does
not handle:

- **Flow `match`** — hit the `default:` arm, which silently dropped the case
  bodies' break labels. `lbl: { match (x) { 1 => { break lbl; } } return 1; }`
  was judged must-terminate, omitting a needed implicit return.
- **`try/catch/finally`** — `SemanticResolver.cpp:794` splits these only when
  `compile_`; otherwise `checkTerminationTryStatement` asserts, and under
  `NDEBUG` takes the `_handler` branch and **ignores the finalizer entirely**.

Both abort assertions-enabled builds and return wrong results under `NDEBUG`.
**The shipped WebAssembly parser has both today.** Verified: 33/33 AST unit
tests, and both original repros exit cleanly under `hermesc`.

Caveat: the second commit is not a byte-for-byte replay of the original —
resolving a `ResolverTest.cpp` conflict pulled in later test-quality repairs
(a helper that could crash instead of failing; a tautological case).

### `hermes-parser-wasm-fixes`

1. **Build fixes.** `scripts/build.sh` required Meta-only
   `facebook/getIncludePath.sh` and exited otherwise; now defaults to the
   repository's `include/`. `HermesParser.js` assumed the `-sMODULARIZE`
   factory returns the module object, but since emscripten 4.0.12 it always
   returns a Promise even with `-sWASM_ASYNC_COMPILATION=0`, so `cwrap` was
   undefined. Uses the object passed *into* the factory instead, which is
   populated in place and complete on return given `-sSINGLE_FILE=1`.
2. **Position fix.** Removes a `std::sort` over every source position
   (1,861,256 of them on an 8.7 MB input) that existed only so a single forward
   scan need never rewind — it cost ~3x the scan it enabled. Replaced by a
   chunk-indexed line + multi-byte table built in one `O(N)` pass. Adds
   `unittests/HermesParserJS/`, the first test coverage that file has had.

Measured on wasm: **-13.5% to -13.9%** on the engine stage up to 1.3 MB,
**-19.7%** on 8.7 MB. Verified: emscripten build succeeds, 26/26 native tests,
and position data SHA-256 identical to the shipped npm artifact.

### `parser-native`

The `hermes-parser-native` fork: a Node-API addon replacing the WebAssembly
blob, plus its npm package, packaging, and optimizations. See
`doc/superpowers/specs/2026-08-03-hermes-parser-native-design.md` and
`doc/superpowers/plans/2026-08-03-hermes-parser-native.md`, both tracked.

Correctness is well established: ASTs identical to the WebAssembly reference
across 38 differential cases and 179/179 corpus files with an enforced
comparison count; all 435 of the original package's tests pass unmodified;
`check-hermes` 4102 passing.

**Not ready to publish.** No CI wiring exists, and 3 of the 4 advertised
platforms (`linux-arm64`, `darwin-x64`, `darwin-arm64`) have never been
compiled — only `linux-x64`.

## Performance findings

All figures Release/Clang, ThinLTO where noted, on
`scratchpad/corpus/typescript.js` (8.72 MB, 930,628 nodes) unless stated.
Earlier numbers in this repository's history that contradict these were
measured wrongly — see "Errors made" below.

### The headline

The Hermes C++ parser is **not** slow. Getting its output into JavaScript is.

| Stage, 65.5 KB file | ms | share |
| --- | --- | --- |
| C++ front-end (parse + sema) | 0.64 | 12% |
| C++ serialize | 0.67 | 13% |
| C++ boundary (source-in, container, copy, init/teardown) | 0.26 | 5% |
| JS deserialize + ESTree adapt | ~3.3 | 63% |
| **Babel, complete job** | **3.19** | — |

The front-end alone is ~5x faster than Babel's entire job. Serialization costs
**more than parse and sema combined**. At the public API, on large real files,
the fork is **1.7x-2.1x slower than `@babel/parser`**, and Babel's lead grows
with file size. Of typescript.js's 778 ms, **340 ms is the ESTree adapter alone**.

### Native vs WebAssembly

Roughly at parity on the engine side, which is not an artifact. The parser is
memory-bound on its AST, and wasm32's 32-bit pointers nearly halve it —
615,480 B vs ~338,800 B. Native executes 0.71x the instructions at 0.82x the
IPC with 1.5x the L2-miss traffic; the effects cancel. Matched scope, native is
1.20x-1.22x. Attribution of the remaining deficit: string interning (the fork's
own design) 37%, container + ArrayBuffer copy 35%, absence of LTO 27%.

The published `HermesParserWASM.js` was confirmed to produce ASTs identical to
a build from this source, so it is a compatible revision — an earlier open
question.

### Optimizations applied to the fork

- Position sort removed: engine stage **-18% to -24%**.
- Container written straight into the result ArrayBuffer (one copy removed,
  plus a ~460 KB zero-fill): **+2.2%**.
- `computeKindHash()` cached — it was re-hashing 295 node kinds *per parse*.
- **ThinLTO: +4.6% engine, +3.7% end-to-end**, and the binary shrinks
  2.48 MB -> 1.14 MB. It is a *build recipe* (`-DCMAKE_INTERPROCEDURAL_OPTIMIZATION=ON`
  at configure time), not a CMake change, because it must apply to
  `hermesParser`/`hermesAST`/`hermesSema` too.

### Package size

Measured, and the design spec's original assumption was wrong. A Release
stripped `linux-x64` addon is 2.01 MB (780 KB gzipped), so four bundled
platforms are ~8.0 MB unpacked / ~3.1 MB packed against the wasm package's
423.7 KB — about **7x larger**. Per-platform `optionalDependencies` would be
~780 KB per user. Bundled was kept deliberately after measuring; switching is
packaging-only, since the loader already resolves prebuilds by platform.

## Errors made, so they are not repeated

Four times, a measurement compared unequal things and was reported as though
equal. Each was caught by review or by the owner, not by the author.

1. Engine-side vs whole-pipeline, reported as parser performance.
2. An interning "control" corpus that repeated each identifier twice per line,
   so the no-interning baseline already got a 2x benefit.
3. `dist/HermesParser.parse` (raw, unadapted AST) compared against Babel's
   complete AST. This is the one that produced "the fork beats Babel"; at the
   real public entry point Babel wins.
4. End-to-end numbers measured against a `dist/` build five days stale.

Also: the plan itself contained four tests that could not fail (a bias check
any buffer satisfied, an `Object.is` on primitive strings, an unenforced corpus
skip, a vacuously-passing platform test) and a 5x buffer over-read. Implementers
and reviewers caught all of them.

**Standing rules that came out of this:** pin binaries explicitly, never rely
on loader resolution order; interleave A/B in one process; report ranges, not
points; no `global.gc()` between rounds (it creates a bimodal artifact); and
compare like with like — state which layer a number describes.

## Environment

- **emsdk 6.0.6** at `~/3rd/emsdk`. Source `emsdk_env.sh`; do not modify shell
  profiles. `emcmake cmake -B <dir> -G Ninja -DCMAKE_BUILD_TYPE=Release` then
  build target `hermes-parser-wasm`. No host tools needed.
- Build directories are gitignored and regenerable. `prebuilds/` and `dist/`
  in the npm package are gitignored build artifacts produced by
  `scripts/build-native.sh` (needs `HERMES_PARSER_ALLOW_MISSING_PREBUILDS=1`
  when only one platform exists). A `DistFreshness-test.js` fails if `dist/`
  drifts from `src/`, using content hashes rather than mtimes.
- Large test corpus (typescript.js, three.module.js, lodash.js, rxjs.umd.js)
  was downloaded from unpkg into a scratch directory and is **not** preserved;
  re-download if needed.

## Open items

- Three of four advertised platforms never compiled; no CI. Blocks publishing.
- The ESTree adapter is the dominant remaining cost (43% of the pipeline).
  Native construction of ESTree objects via Node-API would delete both the
  adapter and the deserializer — the design spec rejected it on a theory that
  now has a ~3.3 ms budget to beat, which is far more attractive than when the
  choice was made.
- `serializeSourcePositions`'s sibling costs remain: the source is read twice
  (`napi_get_value_string_utf8` called once for length), and the serializer
  still writes the program buffer before copying it out.
- The fork's `HermesToESTreeAdapter.js` etc. are byte-identical copies of the
  original package's; a `ForkDrift-test.js` enforces that.
