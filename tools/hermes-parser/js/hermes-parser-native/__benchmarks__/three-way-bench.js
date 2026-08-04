/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

// A three-way parser comparison: hermes-parser-native (Node-API addon),
// hermes-parser (WebAssembly), and @babel/parser, on the same corpora, in the
// same process, with the time broken down by phase.
//
// This is a superset of ./parse-bench.js, which compares only the two Hermes
// parsers. It is a plain Node script rather than a jest test (hence
// __benchmarks__, which jest.config.js's testMatch does not pick up): timing
// needs an unpatched, un-instrumented require of each package's `dist` build,
// which is what an actual consumer would load.
//
// Usage:
//   HERMES_PARSER_NATIVE_ADDON=$PWD/cmake-build-release/tools/hermes-parser-native/hermes-parser.node \
//     node tools/hermes-parser/js/hermes-parser-native/__benchmarks__/three-way-bench.js
//
// HERMES_PARSER_NATIVE_ADDON is mandatory: the addon's default resolution
// order falls back to cmake-build-debug, an unoptimized build that understates
// native performance by roughly 5.6x. Forcing the caller to name a binary
// keeps that mistake from happening silently, and the resolved path, size and
// mtime are printed below so every run carries its own provenance.
//
// Optional env vars:
//   BENCH_ROUNDS=<n>         Timed rounds per scenario (default 25).
//   BENCH_ROTATE=<n>         Rotate the engine start order by n before the
//                            run, on top of the per-round rotation. Running
//                            the script once per value of n and comparing
//                            shows whether *which engine warms up first*
//                            changes the answer.
//   BENCH_SCENARIOS=a,b,c    Run only the named scenarios (see SCENARIOS).
//   HERMES_PARSER_NATIVE_PHASE_TIMING=1
//                            If the addon was built with the phase-timing
//                            probes, breaks the native step down into
//                            sourceIn/contextInit/parserInit/parse/serialize/
//                            sema/container/copyOut/teardown. Adds ~9 clock
//                            reads per parse call; measure both ways to bound it.

const fs = require('fs');
const path = require('path');
const os = require('os');
const {execFileSync, spawnSync} = require('child_process');

// GC pauses that happen to land inside one engine's timed round and not
// another's are noise this benchmark can eliminate: forcing a collection
// between rounds (available only with --expose-gc) keeps heap state
// comparable at the start of every round. Re-exec once with the flag rather
// than asking every caller to remember it.
if (typeof global.gc !== 'function') {
  const result = spawnSync(
    process.execPath,
    ['--expose-gc', __filename, ...process.argv.slice(2)],
    {stdio: 'inherit', env: process.env},
  );
  process.exit(result.status == null ? 1 : result.status);
}

const REPO_ROOT = path.resolve(__dirname, '..', '..', '..', '..', '..');
const JS_ROOT = path.join(REPO_ROOT, 'tools/hermes-parser/js');
const NATIVE_DIST = path.join(JS_ROOT, 'hermes-parser-native/dist');
const WASM_DIST = path.join(JS_ROOT, 'hermes-parser/dist');
const BABEL_PARSER = path.join(JS_ROOT, 'node_modules/@babel/parser');

// ---------------------------------------------------------------------------
// Provenance
// ---------------------------------------------------------------------------

const addonOverride = process.env.HERMES_PARSER_NATIVE_ADDON;
if (addonOverride == null || addonOverride === '') {
  console.error(
    'HERMES_PARSER_NATIVE_ADDON is not set. Refusing to run: without it the ' +
      'addon loader falls back to cmake-build-debug (unoptimized) and the ' +
      'numbers would badly understate native performance.',
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
  // Provenance decoration only.
}

const babelPkgVersion = JSON.parse(
  fs.readFileSync(path.join(BABEL_PARSER, 'package.json'), 'utf8'),
).version;

console.log('=== Environment ===');
console.log(`native addon binary : ${resolvedAddonPath}`);
console.log(
  `  size ${addonStat.size} bytes, mtime ${addonStat.mtime.toISOString()}`,
);
console.log(`@babel/parser       : ${babelPkgVersion} (${BABEL_PARSER})`);
console.log(`node                : ${process.version} (V8 ${process.versions.v8})`);
console.log(`platform            : ${process.platform}/${process.arch}`);
console.log(`cpu                 : ${os.cpus()[0].model} x${os.cpus().length}`);
console.log(`loadavg             : ${os.loadavg().map(x => x.toFixed(2)).join(' ')}`);
console.log(`git commit          : ${gitCommit}`);
console.log('');

// ---------------------------------------------------------------------------
// Parsers under test
// ---------------------------------------------------------------------------
//
// Three packages, and for the two Hermes ones, three API levels:
//
//   raw     HermesParser.parse() - native/wasm call plus the JavaScript
//           deserializer. Produces the raw Hermes AST: `loc` but no `range`,
//           no `sourceType`, no docblock. This is the level ./parse-bench.js
//           measures, kept here for continuity.
//   estree  index.parse() - raw plus HermesToESTreeAdapter, a full JS walk of
//           the AST that adds `range`, `sourceType`, docblock and various
//           node fixups. This is what `require('hermes-parser').parse` gives
//           a consumer, and is the honest "end to end" figure for ESTree.
//   babel   index.parse({babel: true}) - estree plus the Flow-lowering
//           transforms and TransformESTreeToBabel. Produces a Babel `File`
//           node, i.e. the same AST shape @babel/parser produces, so it is
//           the only apples-to-apples output comparison against Babel.
//
// @babel/parser has one level; it is reported against all three.

const nativeRaw = require(path.join(NATIVE_DIST, 'HermesParser')).parse;
const wasmRaw = require(path.join(WASM_DIST, 'HermesParser')).parse;
const nativeIndex = require(path.join(NATIVE_DIST, 'index')).parse;
const wasmIndex = require(path.join(WASM_DIST, 'index')).parse;
const babelParser = require(BABEL_PARSER);

// The addon object HermesParser.js's getAddon() resolves to. `require` is
// cached, so this is the very same object, and wrapping its `parse` property
// intercepts every call the package makes.
const addonModule = require(resolvedAddonPath);

/// Wraps addonModule.parse so the time inside the Node-API call - everything
/// on the C++ side plus the N-API dispatch - can be separated from the time
/// the JavaScript deserializer and adapters spend afterwards.
function instrumentAddon() {
  const original = addonModule.parse;
  let ns = 0n;
  addonModule.parse = function (...args) {
    const t0 = process.hrtime.bigint();
    const r = original.apply(this, args);
    ns += process.hrtime.bigint() - t0;
    return r;
  };
  return {
    reset() {
      ns = 0n;
    },
    ms() {
      return Number(ns) / 1e6;
    },
  };
}

/// Instruments a HermesParserDeserializer's `deserialize` in place, so time
/// spent turning the serialized buffer into JavaScript objects can be
/// attributed separately from time spent producing that buffer. Patching the
/// prototype of the class `parse()` already uses (both dist modules are
/// singletons in the require cache) has no effect on behavior beyond two
/// hrtime reads around the unmodified original.
function instrumentDeserializer(distDir) {
  const proto = require(path.join(distDir, 'HermesParserDeserializer')).default
    .prototype;
  const original = proto.deserialize;
  let ns = 0n;
  proto.deserialize = function (...args) {
    const t0 = process.hrtime.bigint();
    const result = original.apply(this, args);
    ns += process.hrtime.bigint() - t0;
    return result;
  };
  return {
    reset() {
      ns = 0n;
    },
    ms() {
      return Number(ns) / 1e6;
    },
  };
}

/// Instruments everything index.js layers on top of HermesParser.parse: the
/// HermesToESTreeAdapter walk, the four Flow-lowering transforms and the
/// ESTree-to-Babel transform. All of these are module-level exports on
/// `__esModule` modules, so index.js's `_interopRequireWildcard` handed it
/// the very objects patched here and reads the property fresh on each call.
/// Accumulating them into one timer gives the "JavaScript adapters" column,
/// which is the only way to attribute wasm's non-deserialize time (wasm has
/// no equivalent of the addon hook without patching emscripten's cwrap).
function instrumentAdapters(distDir) {
  let ns = 0n;
  const wrap = fn =>
    function (...args) {
      const t0 = process.hrtime.bigint();
      const r = fn.apply(this, args);
      ns += process.hrtime.bigint() - t0;
      return r;
    };

  const adapter = require(path.join(distDir, 'HermesToESTreeAdapter')).default;
  adapter.prototype.transform = wrap(adapter.prototype.transform);

  for (const rel of [
    'estree/TransformEnumSyntax',
    'estree/TransformMatchSyntax',
    'estree/TransformComponentSyntax',
    'estree/TransformRecordSyntax',
    'estree/StripFlowTypesForBabel',
    'babel/TransformESTreeToBabel',
  ]) {
    const mod = require(path.join(distDir, rel));
    if (typeof mod.transformProgram !== 'function') {
      throw new Error(`${rel} has no transformProgram to instrument`);
    }
    mod.transformProgram = wrap(mod.transformProgram);
  }

  return {
    reset() {
      ns = 0n;
    },
    ms() {
      return Number(ns) / 1e6;
    },
  };
}

const addonTimer = instrumentAddon();
const nativeDeserTimer = instrumentDeserializer(NATIVE_DIST);
const wasmDeserTimer = instrumentDeserializer(WASM_DIST);
const nativeAdapterTimer = instrumentAdapters(NATIVE_DIST);
const wasmAdapterTimer = instrumentAdapters(WASM_DIST);

// Optional C++-side phase attribution, present only in an addon built with
// the phase-timing probes and only active when the env var was set at load.
const phaseNames = [
  'sourceIn',
  'contextInit',
  'parserInit',
  'parse',
  'serialize',
  'sema',
  'container',
  'copyOut',
  'teardown',
];
const phaseTimingAvailable =
  typeof addonModule.getPhaseTimings === 'function' &&
  addonModule.getPhaseTimings().enabled === true;
console.log(
  `native phase timing : ${
    phaseTimingAvailable
      ? 'ENABLED (addon reports per-phase C++ times)'
      : 'not available (addon lacks probes or env var unset)'
  }`,
);
console.log('');

// ---------------------------------------------------------------------------
// Corpora
// ---------------------------------------------------------------------------

function walkJs(dir, out) {
  for (const entry of fs.readdirSync(dir, {withFileTypes: true})) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name !== 'node_modules') {
        walkJs(full, out);
      }
    } else if (entry.name.endsWith('.js')) {
      out.push(full);
    }
  }
  return out;
}

// Corpus 1: the seven sibling package `src/` trees. Flow-typed, JSX in
// places. This is the same file set Differential-test.js's "bulk corpus" test
// proves the two Hermes parsers agree byte-for-byte on, and the corpus
// ./parse-bench.js uses, so a parse failure here is a benchmark bug rather
// than a silent timing artifact.
const FLOW_ROOTS = [
  'hermes-parser-native/src',
  'hermes-parser/src',
  'hermes-transform/src',
  'babel-plugin-syntax-hermes-parser/src',
  'flow-api-translator/src',
  'hermes-eslint/src',
  'hermes-estree/src',
].map(r => path.join(JS_ROOT, r));

// Corpus 2: real published JavaScript from node_modules. No Flow syntax, so
// all three parsers handle all of it - which the Flow corpus above does not
// allow (see the babel-compatible subset below). It is also far more varied
// in file size (14 bytes to 546 KB), which is what makes the size sweep at
// the end meaningful.
const PLAIN_ROOTS = [
  '@babel/types/lib',
  '@babel/generator/lib',
  '@babel/traverse/lib',
  '@babel/core/lib',
  'acorn/dist',
  'espree/lib',
  'lodash',
  'jest-diff/build',
  'chalk/source',
  'resolve/lib',
  'semver',
].map(r => path.join(JS_ROOT, 'node_modules', r));

function loadFiles(roots) {
  const files = [];
  for (const root of roots) {
    if (fs.existsSync(root)) {
      walkJs(root, files);
    }
  }
  return files
    .map(f => ({file: f, source: fs.readFileSync(f, 'utf8')}))
    .filter(x => x.source.trim().length > 0);
}

const flowCorpus = loadFiles(FLOW_ROOTS);
const plainCorpusAll = loadFiles(PLAIN_ROOTS);

// Babel options. Hermes always parses Flow and JSX - there is no way to turn
// either off through this API - and always decides script-vs-module itself.
// Giving Babel `sourceType: 'unambiguous'` and both plugins is therefore the
// setting that makes it do the most comparable work. `plugins: []` would be
// faster for Babel but would not be the same job.
const BABEL_OPTS = {sourceType: 'unambiguous', plugins: ['flow', 'jsx']};

/// Which of \p corpus each engine can actually parse. A parser that throws on
/// a file is not doing the work, so any file not handled by all three is
/// excluded from the shared comparison and counted here instead.
function classify(corpus) {
  const perParser = {native: 0, wasm: 0, babel: 0};
  const allThree = [];
  for (const entry of corpus) {
    let n = true;
    let w = true;
    let b = true;
    try {
      nativeIndex(entry.source, {});
    } catch (e) {
      n = false;
    }
    try {
      wasmIndex(entry.source, {});
    } catch (e) {
      w = false;
    }
    try {
      babelParser.parse(entry.source, BABEL_OPTS);
    } catch (e) {
      b = false;
    }
    if (n) perParser.native++;
    if (w) perParser.wasm++;
    if (b) perParser.babel++;
    if (n && w && b) allThree.push(entry);
  }
  return {perParser, allThree};
}

const flowClass = classify(flowCorpus);
const plainClass = classify(plainCorpusAll);

const bytesOf = c => c.reduce((n, x) => n + Buffer.byteLength(x.source), 0);

console.log('=== Corpora ===');
console.log(
  `flow (7 package src/ trees) : ${flowCorpus.length} files, ` +
    `${bytesOf(flowCorpus)} bytes`,
);
console.log(
  `  parsed by: native ${flowClass.perParser.native}, ` +
    `wasm ${flowClass.perParser.wasm}, babel ${flowClass.perParser.babel}`,
);
console.log(
  `  all-three subset          : ${flowClass.allThree.length} files, ` +
    `${bytesOf(flowClass.allThree)} bytes`,
);
console.log(
  `plain (node_modules JS)     : ${plainCorpusAll.length} files, ` +
    `${bytesOf(plainCorpusAll)} bytes`,
);
console.log(
  `  parsed by: native ${plainClass.perParser.native}, ` +
    `wasm ${plainClass.perParser.wasm}, babel ${plainClass.perParser.babel}`,
);
console.log(
  `  all-three subset          : ${plainClass.allThree.length} files, ` +
    `${bytesOf(plainClass.allThree)} bytes`,
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
    median: sorted[n >> 1],
    min: sorted[0],
    max: sorted[n - 1],
    cv: mean === 0 ? 0 : stddev / mean,
  };
}

/// Welch-style z for "are these two means actually different, given spread".
function zScore(a, b) {
  const se = Math.sqrt(a.se ** 2 + b.se ** 2);
  return se === 0 ? Infinity : (a.mean - b.mean) / se;
}

const fmt = (x, d = 2) => x.toFixed(d);

// ---------------------------------------------------------------------------
// Bench runner
// ---------------------------------------------------------------------------
//
// All engines are measured together, round by round, rotating which one goes
// first each round. This spreads monotonic drift over the process lifetime
// (thermal throttling, allocator growth, background load) evenly over every
// engine instead of letting one engine's whole measurement window sit at a
// systematically different point in that drift. BENCH_ROTATE is the
// complementary control: it changes which engine pays the first-ever warmup
// and JIT tier-up cost in the process.

const ROUNDS = Number(process.env.BENCH_ROUNDS || 25);
const ROTATE = Number(process.env.BENCH_ROTATE || 0);

function rotate(arr, k) {
  const n = arr.length;
  const m = ((k % n) + n) % n;
  return arr.slice(m).concat(arr.slice(0, m));
}

function runScenario(name, corpus, engineDefs, {passes = 1, warmup = 3} = {}) {
  const totalBytes = bytesOf(corpus);
  console.log(`=== ${name} ===`);
  console.log(
    `  ${corpus.length} files, ${totalBytes} bytes/pass, ` +
      `${ROUNDS} rounds x ${passes} pass(es), ${warmup} warmup passes, ` +
      `rotate=${ROTATE}`,
  );

  const engines = engineDefs.map(d => ({
    ...d,
    total: [],
    deser: [],
    addon: [],
    adapters: [],
    checksum: 0,
  }));

  // Touch a property of every returned AST and fold it into a checksum rather
  // than discarding the result: an unused return value from an
  // otherwise-side-effect-free-looking function is exactly what a
  // sufficiently aggressive optimizer could elide. The checksum is printed,
  // so every field AST construction sets has to actually be computed.
  function onePass(engine) {
    let checksum = 0;
    for (const entry of corpus) {
      const ast = engine.parse(entry.source);
      checksum += ast.type.length;
    }
    engine.checksum += checksum;
  }

  for (const engine of rotate(engines, ROTATE)) {
    for (let w = 0; w < warmup; w++) {
      onePass(engine);
    }
  }
  for (const engine of engines) {
    engine.checksum = 0; // Discard warmup; count only measured work.
  }

  for (let r = 0; r < ROUNDS; r++) {
    for (const engine of rotate(engines, ROTATE + r)) {
      global.gc();
      if (engine.deserTimer) engine.deserTimer.reset();
      if (engine.addonTimer) engine.addonTimer.reset();
      if (engine.adapterTimer) engine.adapterTimer.reset();
      const t0 = process.hrtime.bigint();
      for (let p = 0; p < passes; p++) {
        onePass(engine);
      }
      const totalMs = Number(process.hrtime.bigint() - t0) / 1e6;
      engine.total.push(totalMs);
      engine.deser.push(engine.deserTimer ? engine.deserTimer.ms() : NaN);
      engine.addon.push(engine.addonTimer ? engine.addonTimer.ms() : NaN);
      engine.adapters.push(
        engine.adapterTimer ? engine.adapterTimer.ms() : NaN,
      );
    }
  }

  const mbPerSec = ms => (totalBytes * passes) / 1024 / 1024 / (ms / 1000);
  const results = engines.map(e => ({
    name: e.name,
    shape: e.shape,
    total: stats(e.total),
    deser: e.deserTimer ? stats(e.deser) : null,
    addon: e.addonTimer ? stats(e.addon) : null,
    adapters: e.adapterTimer ? stats(e.adapters) : null,
  }));

  const baseline = results[0];
  console.log('');
  console.log(
    '  engine                  shape    total ms (sd,  cv)     MB/s   ' +
      'vs ' +
      baseline.name.padEnd(12) +
      ' z',
  );
  for (const r of results) {
    const z = r === baseline ? 0 : zScore(baseline.total, r.total);
    console.log(
      `  ${r.name.padEnd(22)}  ${(r.shape || '').padEnd(7)}  ` +
        `${fmt(r.total.mean).padStart(8)} (${fmt(r.total.stddev).padStart(6)}, ` +
        `${fmt(r.total.cv * 100, 1).padStart(4)}%)  ` +
        `${fmt(mbPerSec(r.total.mean)).padStart(6)}  ` +
        `${fmt(baseline.total.mean / r.total.mean, 3).padStart(6)}x       ` +
        `${fmt(z, 1).padStart(7)}` +
        (r !== baseline && Math.abs(z) < 3 ? '  [WITHIN NOISE]' : ''),
    );
  }

  const split = results.filter(r => r.deser != null);
  if (split.length > 0) {
    console.log('');
    console.log(
      '  phase split (ms/round)    total     engineStep   jsDeserialize   ' +
        'jsAdapters       jsGlue',
    );
    for (const r of split) {
      const total = r.total.mean;
      const deser = r.deser.mean;
      const adapters = r.adapters.mean;
      // The addon timer isolates native's C++ step exactly. wasm has no
      // equivalent hook without patching emscripten's cwrap, so its step is
      // obtained by subtraction and therefore absorbs the JS glue (the
      // Buffer.from + heap copy of the source, the malloc/free pair and the
      // result accessors) that native's column reports separately.
      const step = r.addon ? r.addon.mean : total - deser - adapters;
      const glue = r.addon ? total - step - deser - adapters : NaN;
      const cell = (x, label) =>
        `${(isNaN(x) ? '-' : fmt(x)).padStart(8)} ${
          isNaN(x) ? '     - ' : (fmt((x / total) * 100, 1) + '%').padStart(7)
        }`;
      console.log(
        `  ${r.name.padEnd(20)} ${fmt(total).padStart(8)}  ` +
          `${cell(step)}  ${cell(deser)}  ${cell(adapters)}  ${cell(glue)}` +
          (r.addon ? '' : '   [step by subtraction: includes glue]'),
      );
    }
  }
  console.log('');
  return results;
}

// ---------------------------------------------------------------------------
// Engine definitions
// ---------------------------------------------------------------------------

const ENGINE = {
  nativeRaw: {
    name: 'native raw',
    shape: 'hermes',
    parse: s => nativeRaw(s, {}),
    deserTimer: nativeDeserTimer,
    adapterTimer: nativeAdapterTimer,
    addonTimer,
  },
  wasmRaw: {
    name: 'wasm raw',
    shape: 'hermes',
    parse: s => wasmRaw(s, {}),
    deserTimer: wasmDeserTimer,
    adapterTimer: wasmAdapterTimer,
  },
  nativeEstree: {
    name: 'native estree',
    shape: 'estree',
    parse: s => nativeIndex(s, {}),
    deserTimer: nativeDeserTimer,
    adapterTimer: nativeAdapterTimer,
    addonTimer,
  },
  wasmEstree: {
    name: 'wasm estree',
    shape: 'estree',
    parse: s => wasmIndex(s, {}),
    deserTimer: wasmDeserTimer,
    adapterTimer: wasmAdapterTimer,
  },
  nativeBabel: {
    name: 'native babel-ast',
    shape: 'babel',
    parse: s => nativeIndex(s, {babel: true}),
    deserTimer: nativeDeserTimer,
    adapterTimer: nativeAdapterTimer,
    addonTimer,
  },
  wasmBabel: {
    name: 'wasm babel-ast',
    shape: 'babel',
    parse: s => wasmIndex(s, {babel: true}),
    deserTimer: wasmDeserTimer,
    adapterTimer: wasmAdapterTimer,
  },
  babel: {
    name: '@babel/parser',
    shape: 'babel',
    parse: s => babelParser.parse(s, BABEL_OPTS),
  },
  babelNoPlugins: {
    name: '@babel/parser (no plugins)',
    shape: 'babel',
    parse: s => babelParser.parse(s, {sourceType: 'unambiguous'}),
  },
};

// ---------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------

const SCENARIOS = {
  // The direct answer to "is native faster than wasm and Babel". Everything
  // produces a Babel-shaped File node, so the outputs are genuinely
  // comparable.
  'plain-babel-shape': () =>
    runScenario(
      'Plain JS corpus, Babel-shaped output (identical AST shape)',
      plainClass.allThree,
      [
        ENGINE.nativeBabel,
        ENGINE.wasmBabel,
        ENGINE.babel,
        ENGINE.babelNoPlugins,
      ],
    ),
  // The shapes differ here - ESTree versus Babel - so Babel's column is an
  // upper reference, not a like-for-like result.
  'plain-estree': () =>
    runScenario(
      'Plain JS corpus, ESTree output (Babel column is a different shape)',
      plainClass.allThree,
      [ENGINE.nativeEstree, ENGINE.wasmEstree, ENGINE.babel],
    ),
  // The level ./parse-bench.js measures. Least work of all, and the level at
  // which the native-vs-wasm question was originally asked.
  'plain-raw': () =>
    runScenario(
      'Plain JS corpus, raw Hermes AST (no adapters; Babel not comparable)',
      plainClass.allThree,
      [ENGINE.nativeRaw, ENGINE.wasmRaw, ENGINE.babel],
    ),
  'flow-all-raw': () =>
    runScenario(
      'Flow corpus (all 179 files), raw Hermes AST - Hermes parsers only',
      flowCorpus,
      [ENGINE.nativeRaw, ENGINE.wasmRaw],
    ),
  'flow-subset-babel-shape': () =>
    runScenario(
      'Flow corpus, Babel-parsable subset, Babel-shaped output',
      flowClass.allThree,
      [ENGINE.nativeBabel, ENGINE.wasmBabel, ENGINE.babel],
    ),
  'flow-subset-raw': () =>
    runScenario(
      'Flow corpus, Babel-parsable subset, raw Hermes AST',
      flowClass.allThree,
      [ENGINE.nativeRaw, ENGINE.wasmRaw, ENGINE.babel],
    ),
};

const requested =
  process.env.BENCH_SCENARIOS === 'none'
    ? []
    : process.env.BENCH_SCENARIOS
      ? process.env.BENCH_SCENARIOS.split(',')
      : Object.keys(SCENARIOS);

for (const key of requested) {
  const fn = SCENARIOS[key];
  if (fn == null) {
    console.error(`unknown scenario "${key}"`);
    process.exit(1);
  }
  fn();
}

// ---------------------------------------------------------------------------
// File-size sweep
// ---------------------------------------------------------------------------
//
// If a fixed per-call cost (the N-API boundary, Context construction, the
// wasm malloc/free pair, Babel's per-call setup) dominates, throughput must
// climb with file size. Buckets hold real files from the plain corpus grouped
// by size; each bucket is measured on its own so MB/s can be compared across
// them. Buckets are capped in total size so no single bucket dominates the
// run time.

function sizeSweep() {
  console.log('=== File-size sweep (plain corpus, raw/estree/babel) ===');
  const BUCKETS = [
    [0, 512],
    [512, 2048],
    [2048, 8192],
    [8192, 32768],
    [32768, 131072],
    [131072, Infinity],
  ];
  const CAP_BYTES = 400 * 1024;

  const engines = [
    {name: 'native estree', parse: s => nativeIndex(s, {})},
    {name: 'wasm estree', parse: s => wasmIndex(s, {})},
    {name: '@babel/parser', parse: s => babelParser.parse(s, BABEL_OPTS)},
  ];

  console.log(
    '  bucket            files   bytes    ' +
      engines.map(e => e.name.padStart(14)).join('') +
      '     (MB/s)',
  );

  for (const [lo, hi] of BUCKETS) {
    const all = plainClass.allThree.filter(x => {
      const n = Buffer.byteLength(x.source);
      return n >= lo && n < hi;
    });
    const files = [];
    let bytes = 0;
    for (const f of all) {
      if (bytes >= CAP_BYTES) break;
      files.push(f);
      bytes += Buffer.byteLength(f.source);
    }
    if (files.length === 0) continue;

    const perEngine = [];
    for (const e of engines) {
      for (let w = 0; w < 3; w++) {
        for (const f of files) e.parse(f.source);
      }
      const samples = [];
      for (let r = 0; r < 9; r++) {
        global.gc();
        const t0 = process.hrtime.bigint();
        for (const f of files) e.parse(f.source);
        samples.push(Number(process.hrtime.bigint() - t0) / 1e6);
      }
      const s = stats(samples);
      perEngine.push(bytes / 1024 / 1024 / (s.median / 1000));
    }
    const label = `${lo}-${hi === Infinity ? 'inf' : hi}B`;
    console.log(
      `  ${label.padEnd(16)} ${String(files.length).padStart(5)} ` +
        `${String(bytes).padStart(8)}  ` +
        perEngine.map(v => fmt(v).padStart(14)).join('') +
        `   (mean file ${Math.round(bytes / files.length)}B)`,
    );
  }
  console.log('');
}

sizeSweep();

// The bucketed sweep above uses real files, so file size and file *content*
// vary together: the smallest bucket is lodash's one-function modules and the
// largest is bundled output, and those differ in more than length. This
// second sweep holds content statistically constant - every input is built
// from the same ~340-byte declaration template with unique identifiers - and
// varies only how many templates end up in one parse() call, with the total
// bytes parsed held fixed at each point. Any throughput change across rows is
// therefore per-call fixed cost and nothing else.
function syntheticSizeSweep() {
  console.log(
    '=== Synthetic size sweep (identical content, varying file size) ===',
  );

  let counter = 0;
  const unit = () => {
    const i = counter++;
    return (
      `function helperFunction${i}(alphaParam${i}, betaParam${i}) {\n` +
      `  const gammaLocal${i} = alphaParam${i} + betaParam${i} * 2;\n` +
      `  if (gammaLocal${i} > 10) {\n` +
      `    return {kind: 'large', value: gammaLocal${i}, tag: 'unit${i}'};\n` +
      `  }\n` +
      `  return [gammaLocal${i}, alphaParam${i}, betaParam${i}].map(x => x + 1);\n` +
      `}\n`
    );
  };

  const TARGET_TOTAL = 1024 * 1024;
  const engines = [
    {name: 'native raw', parse: s => nativeRaw(s, {})},
    {name: 'native estree', parse: s => nativeIndex(s, {})},
    {name: 'wasm estree', parse: s => wasmIndex(s, {})},
    {name: 'native babel-ast', parse: s => nativeIndex(s, {babel: true})},
    {name: '@babel/parser', parse: s => babelParser.parse(s, BABEL_OPTS)},
  ];
  for (const e of engines) e.points = [];

  console.log(
    '  bytes/file    files  ' +
      engines.map(e => e.name.padStart(17)).join('') +
      '     (MB/s)',
  );

  // Dense in the small-file regime, where per-call cost is the whole story,
  // and a few large points to show where throughput plateaus.
  for (const units of [1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 64, 256, 1024]) {
    const sources = [];
    let total = 0;
    while (total < TARGET_TOTAL) {
      let src = '';
      for (let i = 0; i < units; i++) src += unit();
      sources.push(src);
      total += Buffer.byteLength(src);
    }
    const bytesPerFile = total / sources.length;
    const row = [];
    for (const e of engines) {
      for (let w = 0; w < 2; w++) for (const src of sources) e.parse(src);
      const samples = [];
      for (let r = 0; r < 11; r++) {
        global.gc();
        const t0 = process.hrtime.bigint();
        for (const src of sources) e.parse(src);
        samples.push(Number(process.hrtime.bigint() - t0) / 1e6);
      }
      const ms = stats(samples).median;
      // Microseconds spent per parse() call at this file size, which is what
      // the fixed-plus-marginal fit below is regressed on.
      e.points.push({bytes: bytesPerFile, usPerCall: (ms * 1000) / sources.length});
      row.push(total / 1024 / 1024 / (ms / 1000));
    }
    console.log(
      `  ${String(Math.round(bytesPerFile)).padStart(10)} ${String(
        sources.length,
      ).padStart(8)}  ` + row.map(v => fmt(v).padStart(17)).join(''),
    );
  }

  // Ordinary least squares of usPerCall = fixed + perByte * bytes. If a fixed
  // per-call cost is what makes small files slow, `fixed` is a direct measure
  // of it in microseconds, and `perByte` inverted is the throughput each
  // parser would reach if per-call overhead were free.
  //
  // The fit is restricted to inputs under FIT_MAX_BYTES. Above roughly 20 KB
  // every engine's throughput stops rising and then falls again - the AST for
  // a 300 KB file no longer fits in cache - so the model stops holding, and
  // including those points lets two heavily-leveraged rows drag the intercept
  // negative (which is how an earlier version of this fit reported a
  // physically impossible -104 us of fixed cost).
  const FIT_MAX_BYTES = 8192;
  console.log('');
  console.log(
    `  fit over inputs < ${FIT_MAX_BYTES} B:  usPerCall = fixed + perByte*bytes`,
  );
  console.log(
    '                                         fixed(us)   marginal MB/s   R^2',
  );
  for (const e of engines) {
    const pts = e.points.filter(pt => pt.bytes < FIT_MAX_BYTES);
    const n = pts.length;
    const mx = pts.reduce((a, p) => a + p.bytes, 0) / n;
    const my = pts.reduce((a, p) => a + p.usPerCall, 0) / n;
    const sxy = pts.reduce((a, p) => a + (p.bytes - mx) * (p.usPerCall - my), 0);
    const sxx = pts.reduce((a, p) => a + (p.bytes - mx) ** 2, 0);
    const slope = sxy / sxx;
    const intercept = my - slope * mx;
    const ssTot = pts.reduce((a, p) => a + (p.usPerCall - my) ** 2, 0);
    const ssRes = pts.reduce(
      (a, p) => a + (p.usPerCall - (intercept + slope * p.bytes)) ** 2,
      0,
    );
    const marginal = 1 / slope / 1.048576; // us/byte -> MB/s
    console.log(
      `  ${e.name.padEnd(38)} ${fmt(intercept, 1).padStart(9)}   ` +
        `${fmt(marginal).padStart(13)}   ${fmt(1 - ssRes / ssTot, 4)}`,
    );
  }
  console.log('');
}

syntheticSizeSweep();

// ---------------------------------------------------------------------------
// Native C++ phase attribution
// ---------------------------------------------------------------------------

function phaseAttribution(label, corpus, parse = s => nativeRaw(s, {})) {
  console.log(`=== Native C++ phase attribution (${label}) ===`);
  for (let w = 0; w < 3; w++) {
    for (const f of corpus) parse(f.source);
  }
  addonModule.resetPhaseTimings();
  addonTimer.reset();
  nativeDeserTimer.reset();
  nativeAdapterTimer.reset();
  const t0 = process.hrtime.bigint();
  for (let p = 0; p < 3; p++) {
    for (const f of corpus) parse(f.source);
  }
  const totalMs = Number(process.hrtime.bigint() - t0) / 1e6;
  const t = addonModule.getPhaseTimings();
  const addonMs = addonTimer.ms();
  const deserMs = nativeDeserTimer.ms();

  let sumNs = 0;
  for (const p of phaseNames) sumNs += t[p];
  console.log(`  calls ${t.calls}, JS-observed total ${fmt(totalMs)} ms`);
  console.log(
    `  of which inside addon.parse(): ${fmt(addonMs)} ms ` +
      `(${fmt((addonMs / totalMs) * 100, 1)}%)`,
  );
  console.log(
    `  of which JS deserialize:       ${fmt(deserMs)} ms ` +
      `(${fmt((deserMs / totalMs) * 100, 1)}%)`,
  );
  const adaptMs = nativeAdapterTimer.ms();
  console.log(
    `  of which JS adapters:          ${fmt(adaptMs)} ms ` +
      `(${fmt((adaptMs / totalMs) * 100, 1)}%)`,
  );
  console.log('');
  console.log('  C++ phase              ms      % of addon   % of total');
  for (const p of phaseNames) {
    const ms = t[p] / 1e6;
    console.log(
      `  ${p.padEnd(20)} ${fmt(ms).padStart(7)}  ` +
        `${fmt((ms / (sumNs / 1e6)) * 100, 1).padStart(10)}%  ` +
        `${fmt((ms / totalMs) * 100, 1).padStart(10)}%`,
    );
  }
  const napiOverhead = addonMs - sumNs / 1e6;
  console.log(
    `  ${'(N-API dispatch)'.padEnd(20)} ${fmt(napiOverhead).padStart(7)}  ` +
      `${'-'.padStart(11)}  ${fmt((napiOverhead / totalMs) * 100, 1).padStart(10)}%`,
  );
  console.log('');
}

if (phaseTimingAvailable) {
  phaseAttribution('plain corpus, raw path, 3 passes', plainClass.allThree);
  phaseAttribution('flow corpus, raw path, 3 passes', flowCorpus);
  // The index.js path additionally defaults `flow` to 'detect', which makes
  // the addon run the docblock `@flow` pragma scan inside contextInit.
  phaseAttribution('flow corpus, index.js estree path, 3 passes', flowCorpus, s =>
    nativeIndex(s, {}),
  );
  phaseAttribution(
    'flow corpus, index.js babel-ast path, 3 passes',
    flowCorpus,
    s => nativeIndex(s, {babel: true}),
  );
}

console.log('Done.');
