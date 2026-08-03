/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

// Compares the native (Node-API) parser against the WASM reference parser on
// throughput. See the header comment further down for the methodology and
// ../../../../.superpowers/sdd/2026-08-03-hermes-parser-native/task-12-report.md
// for the write-up of a specific run's results.
//
// This is a plain Node script, not a jest test (hence __benchmarks__, which
// jest.config.js's testMatch does not pick up): timing needs an unpatched,
// un-instrumented require of both `dist` builds, which is what an actual
// consumer of either published package would load.
//
// Usage:
//   HERMES_PARSER_NATIVE_ADDON=$PWD/cmake-build-release/tools/hermes-parser-native/hermes-parser.node \
//     node tools/hermes-parser/js/hermes-parser-native/__benchmarks__/parse-bench.js
//
// HERMES_PARSER_NATIVE_ADDON is mandatory here (unlike in normal use of the
// package): the addon's default resolution order falls back to
// cmake-build-debug, an unoptimized build that would understate native
// performance by a large, misleading margin. Forcing the caller to name a
// binary keeps that mistake from happening silently, and the resolved path,
// size, and mtime are printed below so every run's numbers carry their own
// provenance.
//
// Optional env vars:
//   BENCH_FIRST=native|wasm  Which engine's warmup runs first (default
//                            native). Warming up second means starting from
//                            a hotter process (JIT tier-up already paid for
//                            by the other engine's warmup, OS file cache
//                            already primed). Run the script once with each
//                            value and compare; if the ratio moves, ordering
//                            matters and both numbers should be reported.

const fs = require('fs');
const path = require('path');
const os = require('os');
const {execFileSync, spawnSync} = require('child_process');

// GC pauses that happen to land inside one engine's timed round and not the
// other's are a source of noise this benchmark can actually eliminate:
// forcing a collection between rounds (available only with --expose-gc)
// keeps heap state comparable at the start of every round instead of
// leaving it to chance. Re-exec once with the flag added rather than asking
// every caller to remember it.
if (typeof global.gc !== 'function') {
  const result = spawnSync(
    process.execPath,
    ['--expose-gc', __filename, ...process.argv.slice(2)],
    {stdio: 'inherit', env: process.env},
  );
  process.exit(result.status == null ? 1 : result.status);
}

const REPO_ROOT = path.resolve(__dirname, '..', '..', '..', '..', '..');

// ---------------------------------------------------------------------------
// Binary provenance
// ---------------------------------------------------------------------------

const addonOverride = process.env.HERMES_PARSER_NATIVE_ADDON;
if (addonOverride == null || addonOverride === '') {
  console.error(
    'HERMES_PARSER_NATIVE_ADDON is not set. Refusing to run: without it, ' +
      'the addon loader falls back to cmake-build-debug (unoptimized) and ' +
      'the resulting numbers would badly understate native performance. ' +
      'Set it to the .node binary you want measured, e.g.\n' +
      '  HERMES_PARSER_NATIVE_ADDON=$PWD/cmake-build-release/tools/' +
      'hermes-parser-native/hermes-parser.node',
  );
  process.exit(1);
}
const resolvedAddonPath = path.resolve(addonOverride);
const addonStat = fs.statSync(resolvedAddonPath); // throws if missing

let gitCommit = '(unknown)';
try {
  gitCommit = execFileSync('git', ['rev-parse', '--short', 'HEAD'], {
    cwd: REPO_ROOT,
    encoding: 'utf8',
  }).trim();
} catch (e) {
  // Not fatal; this is just provenance decoration.
}

console.log('=== Environment ===');
console.log(`native addon binary : ${resolvedAddonPath}`);
console.log(
  `  size ${addonStat.size} bytes, mtime ${addonStat.mtime.toISOString()}`,
);
console.log(
  `node                 : ${process.version} (V8 ${process.versions.v8})`,
);
console.log(`platform             : ${process.platform}/${process.arch}`);
console.log(
  `cpu                  : ${os.cpus()[0].model} x${os.cpus().length}`,
);
console.log(`git commit           : ${gitCommit}`);
console.log(
  `BENCH_FIRST          : ${process.env.BENCH_FIRST || 'native (default)'}`,
);
console.log('');

// ---------------------------------------------------------------------------
// Parsers under test
// ---------------------------------------------------------------------------
//
// Both sides are loaded from their built `dist` output, not from `src`: that
// is what an actual consumer of either published package runs, and it is
// the only form in which the wasm reference package ships at all.

const NATIVE_DIST = path.join(
  REPO_ROOT,
  'tools/hermes-parser/js/hermes-parser-native/dist',
);
const WASM_DIST = path.join(
  REPO_ROOT,
  'tools/hermes-parser/js/hermes-parser/dist',
);

const {parse: parseNative} = require(path.join(NATIVE_DIST, 'HermesParser'));
const {parse: parseWasm} = require(path.join(WASM_DIST, 'HermesParser'));

// Instruments a HermesParserDeserializer class's `deserialize` method in
// place, so time spent in it can be attributed separately from time spent
// getting from source text to a serialized buffer (native call + Node-API
// marshaling on the native side; wasm call + heap copies on the wasm side).
// Patching the prototype used by `parse()` above (both modules are
// singletons in the require cache) means this has zero effect on behavior
// and only adds two hrtime reads around the unmodified original method.
function instrumentDeserializer(distDir) {
  const mod = require(path.join(distDir, 'HermesParserDeserializer'));
  const proto = mod.default.prototype;
  const original = proto.deserialize;
  let accumulatedNs = 0n;
  proto.deserialize = function (...args) {
    const t0 = process.hrtime.bigint();
    const result = original.apply(this, args);
    accumulatedNs += process.hrtime.bigint() - t0;
    return result;
  };
  return {
    reset() {
      accumulatedNs = 0n;
    },
    ms() {
      return Number(accumulatedNs) / 1e6;
    },
  };
}

const nativeDeserializeTimer = instrumentDeserializer(NATIVE_DIST);
const wasmDeserializeTimer = instrumentDeserializer(WASM_DIST);

// ---------------------------------------------------------------------------
// Corpus: the same file set Differential-test.js's "bulk corpus" test
// already proved both parsers agree on byte-for-byte AST output for, plus
// this package's own sources. Reusing a corpus whose correctness is already
// established means a parse failure here is a benchmark bug, not a silent
// timing artifact from one engine choking on a file the other handles fine.
// ---------------------------------------------------------------------------

const CORPUS_ROOTS = [
  'tools/hermes-parser/js/hermes-parser-native/src',
  'tools/hermes-parser/js/hermes-parser/src',
  'tools/hermes-parser/js/hermes-transform/src',
  'tools/hermes-parser/js/babel-plugin-syntax-hermes-parser/src',
  'tools/hermes-parser/js/flow-api-translator/src',
  'tools/hermes-parser/js/hermes-eslint/src',
  'tools/hermes-parser/js/hermes-estree/src',
].map(root => path.join(REPO_ROOT, root));

function loadCorpus() {
  const sources = [];
  for (const root of CORPUS_ROOTS) {
    const walk = dir => {
      for (const entry of fs.readdirSync(dir, {withFileTypes: true})) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
          walk(full);
        } else if (entry.name.endsWith('.js')) {
          sources.push(fs.readFileSync(full, 'utf8'));
        }
      }
    };
    walk(root);
  }
  return sources;
}

// A second, synthetic corpus that targets the string-interning claim
// directly: the fork's serializer/deserializer decode each unique string
// once and cache it, while the wasm reference's deserializer decodes UTF-8
// on every reference to an identifier. `identifierRepeated` references the
// same long identifier name on both sides of every line (one distinct
// identifier, 2*lineCount references total). `identifierUnique` is the
// control: an `L`/`R`-tagged pair of identifiers per line, so every single
// occurrence in the whole file — left side and right side, this line and
// every other line — is textually distinct from every other occurrence.
// (An earlier version of this control reused the same per-line identifier
// on both sides of `=`, which still gave native's deduper a same-line
// repeat to collapse; see the fix note at the bottom of
// task-12-report.md.) Line count and, to within a couple of characters per
// identifier, byte size are held fixed between the two corpora; only
// whether the underlying strings repeat anywhere varies. If interning is
// doing anything, native's deserialize time should close the gap (or open
// one) between these two far more than wasm's does.
function genIdentifierCorpus(lineCount) {
  const base = 'someModuleLevelCounterVariableUsedRepeatedly';
  const repeatedLines = [];
  const uniqueLines = [];
  for (let i = 0; i < lineCount; i++) {
    repeatedLines.push(`${base} = ${base} + 1;`);
    uniqueLines.push(`${base}L${i} = ${base}R${i} + 1;`);
  }
  return {
    identifierRepeated: repeatedLines.join('\n') + '\n',
    identifierUnique: uniqueLines.join('\n') + '\n',
  };
}

const corpus = loadCorpus();
const corpusBytes = corpus.reduce((n, s) => n + Buffer.byteLength(s), 0);
const identifierCorpus = genIdentifierCorpus(20000);

console.log('=== Corpus ===');
console.log(
  `bulk corpus          : ${corpus.length} files, ${corpusBytes} bytes`,
);
console.log(
  `identifierRepeated   : ${Buffer.byteLength(identifierCorpus.identifierRepeated)} bytes`,
);
console.log(
  `identifierUnique     : ${Buffer.byteLength(identifierCorpus.identifierUnique)} bytes`,
);
console.log('');

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

function stats(samples) {
  const n = samples.length;
  const mean = samples.reduce((a, b) => a + b, 0) / n;
  const variance =
    n > 1 ? samples.reduce((a, b) => a + (b - mean) ** 2, 0) / (n - 1) : 0;
  const stddev = Math.sqrt(variance);
  const sorted = [...samples].sort((a, b) => a - b);
  return {
    n,
    mean,
    stddev,
    se: stddev / Math.sqrt(n),
    median: sorted[Math.floor(n / 2)],
    min: sorted[0],
    max: sorted[n - 1],
    cv: mean === 0 ? 0 : stddev / mean,
  };
}

// Welch-style z-score for "are these two means actually different, given
// their spread". |z| > 3 is a strong signal the difference isn't noise;
// small |z| means the measured gap could plausibly be run-to-run variance.
function zScore(a, b) {
  const se = Math.sqrt(a.se ** 2 + b.se ** 2);
  return se === 0 ? Infinity : (a.mean - b.mean) / se;
}

function fmt(x, digits = 2) {
  return x.toFixed(digits);
}

function reportPair(label, a, b, unit) {
  console.log(
    `  ${label.padEnd(22)} a=${fmt(a.mean)}${unit} (sd=${fmt(a.stddev)}, cv=${fmt(a.cv * 100, 1)}%)  ` +
      `b=${fmt(b.mean)}${unit} (sd=${fmt(b.stddev)}, cv=${fmt(b.cv * 100, 1)}%)  ` +
      `ratio(b/a)=${fmt(b.mean / a.mean, 3)}  z=${fmt(zScore(a, b), 1)}` +
      (Math.abs(zScore(a, b)) < 3 ? '  [WITHIN NOISE]' : ''),
  );
}

// ---------------------------------------------------------------------------
// Bench runner
// ---------------------------------------------------------------------------
//
// Two engines are measured together, round by round, alternating which one
// goes first each round. This spreads any monotonic drift over the process
// lifetime (thermal throttling, allocator growth, background load) evenly
// over both, rather than letting one engine's whole measurement window sit
// at a systematically different point in that drift than the other's. It is
// a different, complementary mitigation from BENCH_FIRST: BENCH_FIRST
// controls which engine pays the *first-ever* warmup/JIT-tier-up cost in
// the process; the per-round alternation here controls ordering *within*
// the already-warmed-up measurement phase.
function benchTwo({
  corpusSources,
  passesPerRound,
  warmupPasses,
  rounds,
  engines,
}) {
  // Touches one property of each returned AST and folds it into a running
  // checksum, instead of discarding the result outright. An unused return
  // value from an otherwise side-effect-free-looking JS function (the
  // deserializer) is exactly the shape a sufficiently aggressive optimizer
  // could in principle treat as dead code; reading a property that the
  // final console.log below depends on forces every property that AST
  // construction sets to actually be computed. It also doubles as a sanity
  // check: the two checksums are printed and, since both parsers agree on
  // AST shape for this corpus (Differential-test.js), should be equal.
  function onePass(engine) {
    let checksum = 0;
    for (const source of corpusSources) {
      const ast = engine.parseFn(source, {});
      checksum += ast.type.length + (ast.body ? ast.body.length : 0);
    }
    engine.checksum = (engine.checksum || 0) + checksum;
  }

  const order =
    (process.env.BENCH_FIRST || 'native') === 'wasm'
      ? [...engines].reverse()
      : engines;

  for (const engine of order) {
    for (let w = 0; w < warmupPasses; w++) {
      onePass(engine);
    }
  }

  for (const engine of engines) {
    engine.totalSamples = [];
    engine.deserializeSamples = [];
    engine.parseOnlySamples = [];
    engine.checksum = 0; // Discard warmup's contribution; count only what's measured.
  }

  for (let r = 0; r < rounds; r++) {
    const roundOrder = r % 2 === 0 ? engines : [...engines].reverse();
    for (const engine of roundOrder) {
      global.gc();
      engine.deserializeTimer.reset();
      const t0 = process.hrtime.bigint();
      for (let p = 0; p < passesPerRound; p++) {
        onePass(engine);
      }
      const totalMs = Number(process.hrtime.bigint() - t0) / 1e6;
      const deserializeMs = engine.deserializeTimer.ms();
      engine.totalSamples.push(totalMs);
      engine.deserializeSamples.push(deserializeMs);
      engine.parseOnlySamples.push(totalMs - deserializeMs);
    }
  }
}

function runBenchmark(
  name,
  corpusSources,
  {passesPerRound, warmupPasses, rounds},
) {
  console.log(`=== ${name} ===`);
  const totalBytes = corpusSources.reduce(
    (n, s) => n + Buffer.byteLength(s),
    0,
  );

  const engines = [
    {
      name: 'native',
      parseFn: parseNative,
      deserializeTimer: nativeDeserializeTimer,
    },
    {name: 'wasm', parseFn: parseWasm, deserializeTimer: wasmDeserializeTimer},
  ];

  benchTwo({corpusSources, passesPerRound, warmupPasses, rounds, engines});

  const [native, wasm] = engines;
  if (native.checksum !== wasm.checksum) {
    // Both parsers are already known (Differential-test.js) to produce
    // identical ASTs for this corpus, so a mismatch here means the two
    // engines parsed a different number of rounds/files, which would also
    // invalidate the timing comparison above. Fail loudly instead of
    // reporting a ratio computed over unequal work.
    throw new Error(
      `checksum mismatch: native=${native.checksum} wasm=${wasm.checksum}. ` +
        'The two engines did not parse the same work; timings above are not comparable.',
    );
  }
  const nativeTotal = stats(native.totalSamples);
  const wasmTotal = stats(wasm.totalSamples);
  const nativeDeserialize = stats(native.deserializeSamples);
  const wasmDeserialize = stats(wasm.deserializeSamples);
  const nativeParseOnly = stats(native.parseOnlySamples);
  const wasmParseOnly = stats(wasm.parseOnlySamples);

  console.log(
    `  ${rounds} rounds x ${passesPerRound} pass(es)/round, ${warmupPasses} warmup passes, ` +
      `${totalBytes} bytes/pass`,
  );
  reportPair('total (parse+deser)', nativeTotal, wasmTotal, 'ms');
  reportPair('deserialize-only', nativeDeserialize, wasmDeserialize, 'ms');
  reportPair(
    'parse-only (native/wasm side)',
    nativeParseOnly,
    wasmParseOnly,
    'ms',
  );

  const mbPerSec = ms =>
    (totalBytes * passesPerRound) / 1024 / 1024 / (ms / 1000);
  console.log(
    `  throughput: native ${fmt(mbPerSec(nativeTotal.mean))} MB/s, ` +
      `wasm ${fmt(mbPerSec(wasmTotal.mean))} MB/s, ` +
      `native is ${fmt(mbPerSec(nativeTotal.mean) / mbPerSec(wasmTotal.mean), 3)}x wasm throughput`,
  );
  console.log('');

  return {
    nativeTotal,
    wasmTotal,
    nativeDeserialize,
    wasmDeserialize,
    nativeParseOnly,
    wasmParseOnly,
  };
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------

// Round count: 40, not the original 15. Bumped after this fix's re-measurement
// ran on a shared, contended machine (other unrelated processes pushed load
// average past 8 on a 16-core box) and 15 rounds was no longer enough to keep
// the shared-machine jitter from swamping the signal — see the fix note at
// the bottom of task-12-report.md. More rounds shrinks standard error
// (∝ 1/sqrt(n)) without being able to bias the ratio in either direction, so
// it is a precision fix, not a tuned-for-a-favorable-number change.
const ROUNDS = 40;

runBenchmark('Bulk corpus (179 real files, mixed JS/Flow/JSX)', corpus, {
  passesPerRound: 3,
  warmupPasses: 5,
  rounds: ROUNDS,
});

runBenchmark(
  'Identifier-heavy, repeated (string-interning target)',
  [identifierCorpus.identifierRepeated],
  {passesPerRound: 5, warmupPasses: 5, rounds: ROUNDS},
);

runBenchmark(
  'Identifier-heavy, unique (interning control)',
  [identifierCorpus.identifierUnique],
  {passesPerRound: 5, warmupPasses: 5, rounds: ROUNDS},
);

console.log('Done.');
