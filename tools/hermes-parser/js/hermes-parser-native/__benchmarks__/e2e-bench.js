/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

// End-to-end throughput on a single file.
//
// Two comparisons, run in one process but kept apart on purpose:
//
//  1. native vs wasm vs @babel/parser, at two API levels -- `raw`
//     (HermesParser.parse: addon/wasm call plus the JavaScript
//     deserializer) and `estree` (index.parse: the above plus the ESTree
//     adapter, which is what `require('hermes-parser').parse` returns).
//     One native addon per process, chosen with BENCH_NATIVE.
//
//  2. non-LTO vs ThinLTO addon, sharing a single JavaScript instance and
//     swapping only the .node binary beneath it. See the long note above
//     runBinaryAB() for why this cannot be folded into (1).
//
// Why one process. Absolute throughput on this machine moves by
// several percent between harnesses and between processes for reasons
// nobody has pinned down, which is far more than the ~4% ThinLTO effect
// this is meant to resolve. So nothing here is compared across processes.
// Every round measures every engine, the starting engine rotates, and the
// headline comparisons are computed *per round* and then aggregated -- a
// paired statistic, in which any drift that affects the whole round
// (frequency scaling, background load, allocator growth) cancels instead of
// landing on whichever engine happened to be measured at the time.
//
// The native engine is bound to a specific .node binary by pointing
// HERMES_PARSER_NATIVE_ADDON at it; which binary actually got loaded is
// verified below by watching require.cache, not assumed.
//
// It also refuses to run against a stale dist/, since a set of numbers
// taken against five-day-old dist/ JavaScript is what prompted this
// harness in the first place.
//
// Deliberately no global.gc() between rounds: forcing collections there
// produced a bimodal artifact that was chased down and retracted once
// already.
//
// Usage:
//   node tools/hermes-parser/js/hermes-parser-native/__benchmarks__/e2e-bench.js
//
// Env:
//   BENCH_ROUNDS      timed rounds per level (default 50)
//   BENCH_PASSES      parses per engine per round (default 20)
//   BENCH_WARMUP      warmup parses per engine (default 200)
//   BENCH_ROTATE      integer added to the per-round rotation, so a
//                     different engine pays the first-ever JIT tier-up
//                     (default 0)
//   BENCH_NATIVE      release | lto -- which addon the native engine in
//                     comparison (1) uses (default release). Comparison (2)
//                     always measures both.
//   BENCH_INPUT       path to the file to parse (default: the 65.5 KB
//                     generated deserializer table used by earlier runs)
//   BENCH_JSON        if set, write the raw per-round samples here

const fs = require('fs');
const os = require('os');
const path = require('path');
const {execFileSync} = require('child_process');

const REPO_ROOT = path.resolve(__dirname, '..', '..', '..', '..', '..');
const JS_DIR = path.join(REPO_ROOT, 'tools/hermes-parser/js');
const NATIVE_DIST = path.join(JS_DIR, 'hermes-parser-native/dist');
const WASM_DIST = path.join(JS_DIR, 'hermes-parser/dist');

const ROUNDS = Number(process.env.BENCH_ROUNDS || 50);
const PASSES = Number(process.env.BENCH_PASSES || 20);
const WARMUP = Number(process.env.BENCH_WARMUP || 200);
const ROTATE = Number(process.env.BENCH_ROTATE || 0);

const INPUT_PATH =
  process.env.BENCH_INPUT ||
  path.join(JS_DIR, 'hermes-parser/src/HermesParserNodeDeserializers.js');
const SOURCE = fs.readFileSync(INPUT_PATH, 'utf8');
const SOURCE_BYTES = Buffer.byteLength(SOURCE);

const ADDONS = {
  'native-release': path.join(
    REPO_ROOT,
    'cmake-build-release/tools/hermes-parser-native/hermes-parser.node',
  ),
  'native-lto': path.join(
    REPO_ROOT,
    'cmake-build-lto/tools/hermes-parser-native/hermes-parser.node',
  ),
};

// ---------------------------------------------------------------------------
// Freshness of what is being measured
// ---------------------------------------------------------------------------
//
// The whole reason this re-measurement exists is that an earlier set of
// numbers was taken against a dist/ that was five days behind src/. Refuse
// to produce numbers in that state rather than print a caveat nobody reads.
const {checkDistFreshness} = require(path.join(JS_DIR, 'scripts/distManifest'));
const freshness = checkDistFreshness(path.join(JS_DIR, 'hermes-parser-native'));
if (freshness.status !== 'ok') {
  console.error(
    `Refusing to benchmark: hermes-parser-native dist/ check returned ` +
      `'${freshness.status}'.\n${freshness.message || ''}`,
  );
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Engine construction
// ---------------------------------------------------------------------------

function nodeModulesInCache() {
  return new Set(Object.keys(require.cache).filter(p => p.endsWith('.node')));
}

/**
 * Load a private instance of the native dist/ module graph bound to
 * `addonPath`, and prove the binding by observing which .node file appears
 * in require.cache when the instance first parses. dist/HermesParser.js
 * reads HERMES_PARSER_NATIVE_ADDON lazily, on the first parse, so the
 * binding is forced here rather than left to whenever the first timed pass
 * happens to run.
 */
function loadNativeEngine(name, addonPath) {
  if (!fs.existsSync(addonPath)) {
    throw new Error(`${name}: no addon at ${addonPath}`);
  }
  for (const key of Object.keys(require.cache)) {
    if (key.startsWith(NATIVE_DIST + path.sep)) {
      delete require.cache[key];
    }
  }
  process.env.HERMES_PARSER_NATIVE_ADDON = addonPath;

  const before = nodeModulesInCache();
  const raw = require(path.join(NATIVE_DIST, 'HermesParser')).parse;
  const estree = require(path.join(NATIVE_DIST, 'index.js')).parse;
  raw(SOURCE, {});
  const appeared = [...nodeModulesInCache()].filter(p => !before.has(p));

  if (appeared.length !== 1 || appeared[0] !== addonPath) {
    throw new Error(
      `${name}: expected exactly ${addonPath} to be loaded, saw ` +
        `[${appeared.join(', ')}]`,
    );
  }
  const stat = fs.statSync(addonPath);
  return {
    name,
    raw: src => raw(src, {}),
    estree: src => estree(src, {}),
    provenance: `${addonPath} (${stat.size} bytes, mtime ${stat.mtime.toISOString()})`,
  };
}

function loadWasmEngine() {
  const raw = require(path.join(WASM_DIST, 'HermesParser')).parse;
  const estree = require(path.join(WASM_DIST, 'index.js')).parse;
  return {
    name: 'wasm',
    raw: src => raw(src, {}),
    estree: src => estree(src, {}),
    provenance: WASM_DIST,
  };
}

function loadBabelEngine() {
  const babel = require(path.join(JS_DIR, 'node_modules/@babel/parser'));
  const version = require(
    path.join(JS_DIR, 'node_modules/@babel/parser/package.json'),
  ).version;
  // Same plugin set the three-way benchmark used, so the two runs stay
  // comparable. The default input happens to contain neither Flow nor JSX
  // syntax, but the plugins stay on: hermes-parser always accepts both, so
  // turning them off for Babel would be measuring a parser configured for a
  // strictly smaller language than the one it is being compared against.
  const opts = {plugins: ['flow', 'jsx'], sourceType: 'unambiguous'};
  const parse = src => babel.parse(src, opts);
  return {
    name: 'babel',
    raw: parse,
    estree: parse,
    provenance: `@babel/parser ${version} plugins=[${opts.plugins.join(',')}]`,
  };
}

// Exactly one native engine per process, selected by BENCH_NATIVE.
//
// An earlier version of this harness loaded both native addons into one
// process as two instances of the dist/ module graph, so that release and
// ThinLTO could be compared in the same paired rounds. That was wrong, and
// the control that caught it is worth recording: constructing them in the
// other order (LTO first) moved the measured LTO advantage at the raw level
// from -3.9% round time to +0.4%, and flipped the sign test from "LTO
// faster in 40/50 rounds" to 15/50. Whatever V8 does to a second instance
// of the same JavaScript -- warmer inline caches from the first instance's
// warmup, code-space locality, something else -- it is worth about as much
// as the effect being measured, so the two instances are not
// interchangeable and any comparison between them is contaminated.
//
// So the cross-engine numbers below use one native engine loaded exactly
// the way a consumer loads it, and the release-vs-LTO question is answered
// separately, further down, by a comparison that shares a single JavaScript
// instance and swaps only the .node binary underneath it.
const NATIVE_VARIANT = process.env.BENCH_NATIVE || 'release';
const NATIVE_NAME = `native-${NATIVE_VARIANT}`;
if (!Object.prototype.hasOwnProperty.call(ADDONS, NATIVE_NAME)) {
  throw new Error(`BENCH_NATIVE must be one of: release, lto`);
}

const engines = [
  loadNativeEngine(NATIVE_NAME, ADDONS[NATIVE_NAME]),
  loadWasmEngine(),
  loadBabelEngine(),
];

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

function quantile(sortedValues, q) {
  const pos = (sortedValues.length - 1) * q;
  const lo = Math.floor(pos);
  const hi = Math.ceil(pos);
  return sortedValues[lo] + (sortedValues[hi] - sortedValues[lo]) * (pos - lo);
}

function summarize(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  const mean = samples.reduce((a, b) => a + b, 0) / samples.length;
  const sd = Math.sqrt(
    samples.length > 1
      ? samples.reduce((a, b) => a + (b - mean) ** 2, 0) / (samples.length - 1)
      : 0,
  );
  return {
    n: samples.length,
    mean,
    sd,
    cv: mean === 0 ? 0 : sd / mean,
    median: quantile(sorted, 0.5),
    min: sorted[0],
    max: sorted[sorted.length - 1],
    p10: quantile(sorted, 0.1),
    p90: quantile(sorted, 0.9),
  };
}

/**
 * Paired comparison of two engines measured in the same rounds. Ratios are
 * formed round by round, so anything that slowed the whole round down drops
 * out. `wins` is a sign test: how many of the rounds the second engine was
 * faster in. With 50 rounds, 50/50 wins means indistinguishable and 50/0
 * means the effect is larger than the round-to-round noise, independent of
 * any distributional assumption.
 */
function pairedRatio(aSamples, bSamples) {
  const ratios = aSamples.map((a, i) => bSamples[i] / a);
  const s = summarize(ratios);
  const wins = ratios.filter(r => r < 1).length;
  return {...s, wins, rounds: ratios.length};
}

const fmt = (x, d = 2) => x.toFixed(d);
const mbPerSec = msPerRound =>
  (SOURCE_BYTES * PASSES) / 1024 / 1024 / (msPerRound / 1000);

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

function runLevel(level) {
  const samples = new Map(engines.map(e => [e.name, []]));
  let checksum = 0;

  for (const engine of engines) {
    for (let i = 0; i < WARMUP; i++) {
      checksum += engine[level](SOURCE).type.length;
    }
  }

  for (let r = 0; r < ROUNDS; r++) {
    // Rotate the starting engine every round: no engine sits permanently at
    // the "just after the previous engine" position, where it would inherit
    // that engine's cache and allocator state every single time.
    const offset = (r + ROTATE) % engines.length;
    for (let k = 0; k < engines.length; k++) {
      const engine = engines[(offset + k) % engines.length];
      const t0 = process.hrtime.bigint();
      for (let p = 0; p < PASSES; p++) {
        checksum += engine[level](SOURCE).type.length;
      }
      samples.get(engine.name).push(Number(process.hrtime.bigint() - t0) / 1e6);
    }
  }

  return {samples, checksum};
}

function report(level, {samples, checksum}) {
  console.log(`=== ${level} ===`);
  console.log(
    `  ${ROUNDS} rounds x ${PASSES} parses, ${WARMUP} warmup parses, ` +
      `${SOURCE_BYTES} bytes/parse (checksum ${checksum})`,
  );
  console.log(
    '  engine           MB/s median   [min .. max]        ms/round median  cv',
  );
  const stats = new Map();
  for (const engine of engines) {
    const s = summarize(samples.get(engine.name));
    stats.set(engine.name, s);
    console.log(
      `  ${engine.name.padEnd(15)} ${fmt(mbPerSec(s.median)).padStart(6)}` +
        `        [${fmt(mbPerSec(s.max))} .. ${fmt(mbPerSec(s.min))}]` +
        `${''.padEnd(6)} ${fmt(s.median).padStart(7)}` +
        `        ${fmt(s.cv * 100, 1)}%`,
    );
  }

  const pairs = [
    [NATIVE_NAME, 'wasm'],
    [NATIVE_NAME, 'babel'],
    ['wasm', 'babel'],
  ];
  console.log('  paired per-round comparisons (ratio of round times, b/a):');
  for (const [a, b] of pairs) {
    const p = pairedRatio(samples.get(a), samples.get(b));
    console.log(
      `    ${(a + ' -> ' + b).padEnd(32)} ratio median ${fmt(p.median, 4)} ` +
        `[p10 ${fmt(p.p10, 4)} .. p90 ${fmt(p.p90, 4)}]  ` +
        `b faster in ${p.wins}/${p.rounds} rounds`,
    );
  }
  console.log('');
  return stats;
}

// ---------------------------------------------------------------------------
// Binary A/B: non-LTO versus ThinLTO, one JavaScript instance
// ---------------------------------------------------------------------------
//
// The only difference between the two arms here is which .node file
// `addon.parse` lands in. Everything above the Node-API boundary -- the
// option object, the header view, the deserializer class and every inline
// cache it has accumulated -- is one shared instance used by both arms, so
// the JS side cannot contribute a systematic difference the way two
// separate module instances did.
//
// The cost of that is that this path is a re-implementation of
// dist/HermesParser.js's parse() rather than a call to it (the module
// caches its addon in a closure variable on first use, so the real one
// cannot be re-pointed). It is kept deliberately literal against that file;
// the absolute numbers it produces are consistent with the `raw` level
// above, which is the check that it has not drifted.
//
// Two arms per variant: `engine` is the Node-API call alone, where the
// binary is all there is, and `e2e` adds the JavaScript deserializer, which
// dilutes any binary difference by the fraction of wall time it accounts
// for.

const CONTAINER_MAGIC = 0x484d5052;
const CONTAINER_VERSION = 1;
const PARSE_OPTIONS = {
  detectFlow: false,
  enableExperimentalComponentSyntax: false,
  enableExperimentalFlowMatchSyntax: false,
  enableExperimentalFlowRecordSyntax: false,
  tokens: false,
  allowReturnOutsideFunction: false,
};

function runBinaryAB() {
  const Deserializer = require(
    path.join(NATIVE_DIST, 'HermesParserDeserializer'),
  ).default;
  const expectedKindHash = require(
    path.join(NATIVE_DIST, 'HermesParserKindHash'),
  ).default;

  const variants = [];
  for (const [name, addonPath] of Object.entries(ADDONS)) {
    if (!fs.existsSync(addonPath)) {
      console.log(`(skipping binary A/B: ${addonPath} is missing)`);
      return;
    }
    variants.push({name, addon: require(addonPath)});
  }

  const engineOnly = addon => {
    const result = addon.parse(SOURCE, PARSE_OPTIONS);
    if (result.error != null) {
      throw new Error(result.error);
    }
    return result;
  };
  const e2e = addon => {
    const result = engineOnly(addon);
    const header = new Uint32Array(result.buffer, 0, 12);
    if (
      header[0] !== CONTAINER_MAGIC ||
      header[1] !== CONTAINER_VERSION ||
      header[2] !== expectedKindHash
    ) {
      throw new Error('unexpected parse container header');
    }
    return new Deserializer(result.buffer, header, {}).deserialize();
  };

  for (const arm of [
    {
      label: 'engine-side only (addon.parse)',
      fn: engineOnly,
      read: r => r.buffer.byteLength,
    },
    {
      label: 'end-to-end (addon.parse + deserialize)',
      fn: e2e,
      read: a => a.type.length,
    },
  ]) {
    let checksum = 0;
    for (const variant of variants) {
      for (let i = 0; i < WARMUP; i++) {
        checksum += arm.read(arm.fn(variant.addon));
      }
    }
    const samples = new Map(variants.map(v => [v.name, []]));
    for (let r = 0; r < ROUNDS; r++) {
      const offset = (r + ROTATE) % variants.length;
      for (let k = 0; k < variants.length; k++) {
        const variant = variants[(offset + k) % variants.length];
        const t0 = process.hrtime.bigint();
        for (let p = 0; p < PASSES; p++) {
          checksum += arm.read(arm.fn(variant.addon));
        }
        samples
          .get(variant.name)
          .push(Number(process.hrtime.bigint() - t0) / 1e6);
      }
    }

    console.log(`=== binary A/B, ${arm.label} ===`);
    console.log(
      `  ${ROUNDS} rounds x ${PASSES} parses, ${WARMUP} warmup parses ` +
        `(checksum ${checksum})`,
    );
    for (const variant of variants) {
      const s = summarize(samples.get(variant.name));
      console.log(
        `  ${variant.name.padEnd(15)} ${fmt(mbPerSec(s.median)).padStart(6)} MB/s` +
          `   [${fmt(mbPerSec(s.max))} .. ${fmt(mbPerSec(s.min))}]` +
          `   ms/round ${fmt(s.median)}   cv ${fmt(s.cv * 100, 1)}%`,
      );
    }
    const p = pairedRatio(
      samples.get('native-release'),
      samples.get('native-lto'),
    );
    console.log(
      `  release -> lto  ratio median ${fmt(p.median, 4)} ` +
        `[p10 ${fmt(p.p10, 4)} .. p90 ${fmt(p.p90, 4)}]  ` +
        `lto faster in ${p.wins}/${p.rounds} rounds  ` +
        `(lto is ${fmt((1 / p.median - 1) * 100, 1)}% faster)`,
    );
    console.log('');
  }
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------

let gitCommit = '(unknown)';
try {
  gitCommit = execFileSync('git', ['rev-parse', '--short', 'HEAD'], {
    cwd: REPO_ROOT,
    encoding: 'utf8',
  }).trim();
} catch (e) {
  // Provenance decoration only.
}

console.log('=== Environment ===');
console.log(`node        : ${process.version} (V8 ${process.versions.v8})`);
console.log(`cpu         : ${os.cpus()[0].model} x${os.cpus().length}`);
console.log(
  `loadavg     : ${os
    .loadavg()
    .map(x => x.toFixed(2))
    .join(' ')}`,
);
console.log(`git commit  : ${gitCommit}`);
console.log(`input       : ${INPUT_PATH} (${SOURCE_BYTES} bytes)`);
console.log(`rotate      : ${ROTATE}`);
for (const engine of engines) {
  console.log(`${engine.name.padEnd(12)}: ${engine.provenance}`);
}
console.log('');

const results = {};
for (const level of ['raw', 'estree']) {
  const run = runLevel(level);
  report(level, run);
  results[level] = Object.fromEntries(
    engines.map(e => [e.name, run.samples.get(e.name)]),
  );
}

runBinaryAB();

if (process.env.BENCH_JSON) {
  fs.writeFileSync(
    process.env.BENCH_JSON,
    JSON.stringify(
      {
        input: INPUT_PATH,
        bytes: SOURCE_BYTES,
        rounds: ROUNDS,
        passes: PASSES,
        rotate: ROTATE,
        results,
      },
      null,
      2,
    ) + '\n',
  );
}

console.log('Done.');
