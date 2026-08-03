/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

/**
 * hermes-parser-native is a fork of hermes-parser: every file under `src/`,
 * `__tests__/` and `__test_utils__/` is a byte-for-byte copy of the original
 * except for a small, enumerated set. That byte-identity is the whole reason
 * the fork can be trusted: it is why the copied tests are a check on the
 * native parser rather than a check on a separately-evolved codebase.
 *
 * Nothing else enforces it. If either copy is edited the two silently drift
 * apart and the copied tests quietly stop meaning what they are supposed to
 * mean. This test pins the exact set of allowed differences.
 *
 * If it fails, either re-copy the named file from hermes-parser, or -- if
 * the difference is deliberate -- add it to ALLOWED below with a reason.
 */

const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const FORK_DIR = path.resolve(__dirname, '..');
const ORIGINAL_DIR = path.resolve(__dirname, '../../hermes-parser');

/**
 * The complete set of allowed differences, per copied directory.
 *
 * - `modified`: present in both, with different contents.
 * - `onlyInFork`: added by the fork.
 * - `onlyInOriginal`: intentionally not carried over.
 */
const ALLOWED = {
  src: {
    // Reach the parser through the addon instead of the wasm blob, and read
    // the container header rather than raw wasm memory.
    modified: ['HermesParser.js', 'HermesParserDeserializer.js'],
    // Addon loading, and the generated ESTree.def kind-hash constant.
    // Neither has an equivalent in the wasm package.
    onlyInFork: ['HermesParserAddon.js', 'HermesParserKindHash.js'],
    // The wasm blob's Flow declaration. There is no wasm blob here.
    onlyInOriginal: ['HermesParserWASM.js.flow'],
  },
  __tests__: {
    modified: [],
    // Tests of behaviour that only exists in the native package.
    onlyInFork: [
      'Differential-test.js',
      'Failures-test.js',
      'ForkDrift-test.js',
      'Native-test.js',
      'Packaging-test.js',
    ],
    onlyInOriginal: [],
  },
  __test_utils__: {
    modified: [],
    onlyInFork: [],
    onlyInOriginal: [],
  },
};

const KINDS = ['modified', 'onlyInFork', 'onlyInOriginal'];

/**
 * Map every file under `dir` to a hash of its contents, keyed by its path
 * relative to `dir` (with forward slashes, so keys read the same on any
 * platform). Returns an empty map if `dir` does not exist.
 */
function hashTree(dir) {
  const out = new Map();
  if (!fs.existsSync(dir)) {
    return out;
  }
  const walk = current => {
    for (const entry of fs.readdirSync(current, {withFileTypes: true})) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) {
        walk(full);
        continue;
      }
      const rel = path.relative(dir, full).split(path.sep).join('/');
      const hash = crypto.createHash('sha1');
      hash.update(fs.readFileSync(full));
      out.set(rel, hash.digest('hex'));
    }
  };
  walk(dir);
  return out;
}

/** Classify every file of `dirName` into the three difference kinds. */
function diffTrees(dirName) {
  const fork = hashTree(path.join(FORK_DIR, dirName));
  const original = hashTree(path.join(ORIGINAL_DIR, dirName));
  const found = {modified: [], onlyInFork: [], onlyInOriginal: []};

  for (const [rel, hash] of fork) {
    if (!original.has(rel)) {
      found.onlyInFork.push(rel);
    } else if (original.get(rel) !== hash) {
      found.modified.push(rel);
    }
  }
  for (const rel of original.keys()) {
    if (!fork.has(rel)) {
      found.onlyInOriginal.push(rel);
    }
  }

  return {found, forkSize: fork.size, originalSize: original.size};
}

describe('fork of hermes-parser has not drifted', () => {
  test.each(Object.keys(ALLOWED))('%s/ matches the original', dirName => {
    const {found, forkSize, originalSize} = diffTrees(dirName);

    // Without this, a mistyped path would walk nothing and every assertion
    // below would hold vacuously.
    expect(forkSize).toBeGreaterThan(0);
    expect(originalSize).toBeGreaterThan(0);

    // Report both directions as one comparison so a failure names every file
    // involved. `unexpected` means the file drifted; `noLongerDiffers` means
    // an entry in ALLOWED is stale and should be removed.
    const unexpected = [];
    const noLongerDiffers = [];
    for (const kind of KINDS) {
      for (const file of found[kind]) {
        if (!ALLOWED[dirName][kind].includes(file)) {
          unexpected.push(`${kind}: ${dirName}/${file}`);
        }
      }
      for (const file of ALLOWED[dirName][kind]) {
        if (!found[kind].includes(file)) {
          noLongerDiffers.push(`${kind}: ${dirName}/${file}`);
        }
      }
    }

    expect({unexpected: unexpected.sort(), noLongerDiffers}).toStrictEqual({
      unexpected: [],
      noLongerDiffers: [],
    });
  });
});
