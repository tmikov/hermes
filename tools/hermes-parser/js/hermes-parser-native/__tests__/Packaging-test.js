/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

const fs = require('fs');
const path = require('path');

const pkg = require('../package.json');

describe('packaging', () => {
  test('package metadata is correct', () => {
    expect(pkg.name).toBe('hermes-parser-native');
    expect(pkg.version).toBe('0.37.0');
    expect(Object.keys(pkg.dependencies)).toEqual(['hermes-estree']);
    expect(pkg.files).toContain('dist');
    expect(pkg.files).toContain('prebuilds');
  });

  // `dist` and `prebuilds` are build outputs and are deliberately not
  // checked here; these two are checked in, so a published tarball that
  // declares them must actually contain them.
  test.each(['LICENSE', 'README.md'])('%s is present in the package', name => {
    expect(pkg.files).toContain(name);
    expect(fs.existsSync(path.resolve(__dirname, '..', name))).toBe(true);
  });

  test('no wasm blob is shipped', () => {
    const src = path.resolve(__dirname, '../src');
    const names = fs.readdirSync(src);
    expect(names).not.toContain('HermesParserWASM.js');
    expect(names).not.toContain('HermesParserWASM.js.flow');
  });
});
