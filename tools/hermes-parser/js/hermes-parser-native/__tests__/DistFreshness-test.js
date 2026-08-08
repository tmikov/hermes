/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

// Guards against a stale `dist/`.
//
// `dist/` is a gitignored build output that nothing regenerates
// automatically, and it is what `main: dist/index.js` resolves to -- so the
// benchmarks, any consumer-shaped script, and a packed copy of this package
// all run whatever `dist/` happens to contain. That has silently diverged
// from `src/` twice already: a set of end-to-end benchmark numbers was taken
// against five-day-old `dist/` JavaScript, and the loader-precedence fix in
// HermesParserAddon.js sat inert in `src/` while `dist/` kept the old order.
// Both were caught by accident.
//
// build-native.sh's last step records the SHA-256 of every file under `src/`
// into `dist/build-manifest.json`. This test recomputes them. If they
// disagree, the developer who edited `src/` finds out here instead of five
// days later.
//
// Why this and not the prebuilds/ vs cmake-build-* comparison as well: see
// the note at the bottom of this file.

const fs = require('fs');
const os = require('os');
const path = require('path');

const {
  MANIFEST_NAME,
  checkDistFreshness,
  writeManifest,
} = require('../../scripts/distManifest');

const PACKAGE_DIR = path.resolve(__dirname, '..');

describe('dist freshness', () => {
  test('dist/ was built from the current src/', () => {
    const result = checkDistFreshness(PACKAGE_DIR);

    if (result.status === 'no-dist') {
      // Nothing has been built. Jest resolves this package through
      // moduleNameMapper straight to src/, so the suite is meaningful
      // without a dist/ and an unbuilt checkout is not an error: there is
      // no stale artifact to run into.
      return;
    }

    if (result.status === 'stale') {
      throw new Error(result.message);
    }

    expect(result.status).toBe('ok');
  });
});

// The checker itself, against synthetic package layouts. Without these the
// test above is a single assertion that passes on a healthy tree and has
// never been observed to fail, which is exactly the shape of a guard that
// quietly stops guarding.
describe('checkDistFreshness', () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'dist-freshness-'));
  });

  afterEach(() => {
    fs.rmSync(tmpDir, {recursive: true, force: true});
  });

  /**
   * Build a miniature package that looks like what build-native.sh
   * produces: a src/ tree, a dist/ copy of it, and a manifest.
   */
  function makePackage({withSrc = true, withDist = true, withManifest = true}) {
    const pkgDir = path.join(tmpDir, 'pkg');
    const srcDir = path.join(pkgDir, 'src');
    fs.mkdirSync(path.join(srcDir, 'nested'), {recursive: true});
    fs.writeFileSync(path.join(srcDir, 'index.js'), 'module.exports = 1;\n');
    fs.writeFileSync(
      path.join(srcDir, 'nested', 'a.js'),
      'module.exports = 2;\n',
    );

    if (withDist) {
      const distDir = path.join(pkgDir, 'dist');
      fs.cpSync(srcDir, distDir, {recursive: true});
      if (withManifest) {
        writeManifest(pkgDir);
      }
    }
    if (!withSrc) {
      fs.rmSync(srcDir, {recursive: true});
    }
    return pkgDir;
  }

  test('a freshly built package is ok', () => {
    expect(checkDistFreshness(makePackage({})).status).toBe('ok');
  });

  test('a published package with no src/ is not flagged', () => {
    // This is the case that must never fire: `files` in package.json ships
    // dist/ and prebuilds/ but not src/, so a consumer's node_modules copy
    // has nothing to compare against and dist/ is authoritative.
    const pkgDir = makePackage({withSrc: false});
    expect(fs.existsSync(path.join(pkgDir, 'src'))).toBe(false);
    expect(fs.existsSync(path.join(pkgDir, 'dist'))).toBe(true);
    expect(checkDistFreshness(pkgDir).status).toBe('published');
  });

  test('a checkout with no dist/ is not flagged', () => {
    expect(checkDistFreshness(makePackage({withDist: false})).status).toBe(
      'no-dist',
    );
  });

  test('an edited src file is flagged, by name', () => {
    const pkgDir = makePackage({});
    fs.writeFileSync(
      path.join(pkgDir, 'src', 'nested', 'a.js'),
      'module.exports = 3;\n',
    );
    const result = checkDistFreshness(pkgDir);
    expect(result.status).toBe('stale');
    expect(result.changed).toEqual(['nested/a.js']);
    expect(result.message).toContain('src/nested/a.js');
    expect(result.message).toContain('build-native.sh');
  });

  test('touching a src file without changing it is not flagged', () => {
    // The reason this checker hashes content instead of comparing mtimes.
    // `git checkout`, `git stash pop` and build-native.sh's own `rsync -a`
    // all move mtimes around without changing what dist/ was built from; a
    // guard that fired on those would be trained away within a week.
    const pkgDir = makePackage({});
    const future = new Date(Date.now() + 60_000);
    fs.utimesSync(path.join(pkgDir, 'src', 'index.js'), future, future);
    expect(checkDistFreshness(pkgDir).status).toBe('ok');
  });

  test('a new src file is flagged', () => {
    const pkgDir = makePackage({});
    fs.writeFileSync(path.join(pkgDir, 'src', 'b.js'), 'module.exports = 4;\n');
    const result = checkDistFreshness(pkgDir);
    expect(result.status).toBe('stale');
    expect(result.added).toEqual(['b.js']);
  });

  test('a deleted src file is flagged', () => {
    const pkgDir = makePackage({});
    fs.rmSync(path.join(pkgDir, 'src', 'index.js'));
    const result = checkDistFreshness(pkgDir);
    expect(result.status).toBe('stale');
    expect(result.removed).toEqual(['index.js']);
  });

  test('a dist/ with no manifest is flagged', () => {
    // Either the build predates the manifest or it died partway through.
    // Both mean "nobody can vouch for what dist/ contains".
    const pkgDir = makePackage({withManifest: false});
    const result = checkDistFreshness(pkgDir);
    expect(result.status).toBe('stale');
    expect(result.message).toContain(MANIFEST_NAME);
  });

  test('an unparseable manifest is flagged', () => {
    const pkgDir = makePackage({});
    fs.writeFileSync(path.join(pkgDir, 'dist', MANIFEST_NAME), '{not json');
    expect(checkDistFreshness(pkgDir).status).toBe('stale');
  });
});

// On prebuilds/ vs the cmake-build-* trees: deliberately not guarded here.
//
// The equivalent staleness -- a leftover prebuilds/<platform>-<arch>/
// binary shadowing a freshly built dev addon -- was fixed structurally
// rather than by detection: HermesParserAddon.js now checks the in-repo
// cmake-build-debug addon *before* prebuilds/, so in a source checkout a
// stale prebuild is unreachable, and AddonResolutionOrder-test.js pins that
// order by asserting on which .node file lands in require.cache.
//
// A second mtime comparison, prebuilds/ against cmake-build-release, would
// also have to be wrong on purpose a fair amount of the time: prebuilds are
// legitimately produced from the ThinLTO tree, not the plain Release tree,
// and a developer who has only ever built cmake-build-debug has no
// non-stale state for it to be satisfied with. A check that fires when
// nothing is wrong gets skipped, and then the one check that matters gets
// skipped with it. Instead, build-native.sh records each packaged addon's
// SHA-256, size and mtime in dist/build-manifest.json, so "which binary is
// in this package" stays answerable without an assertion that cries wolf.
