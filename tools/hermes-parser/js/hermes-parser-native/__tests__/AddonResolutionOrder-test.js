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

// These are computed the same way HermesParserAddon.js computes them, so
// this test can assert on *which* of the two real on-disk binaries actually
// got loaded, by identity, rather than inferring it from behavior (the two
// builds are functionally identical, so parse() output can't distinguish
// them -- only require.cache can).
const devBuildPath = path.join(
  __dirname,
  '..', // hermes-parser-native
  '..', // js
  '..', // hermes-parser
  '..', // tools
  '..', // repo root
  'cmake-build-debug',
  'tools',
  'hermes-parser-native',
  'hermes-parser.node',
);
const prebuiltPath = path.join(
  __dirname,
  '..',
  'prebuilds',
  `${process.platform}-${process.arch}`,
  'hermes-parser.node',
);

describe('addon resolution order', () => {
  const originalOverride = process.env.HERMES_PARSER_NATIVE_ADDON;

  afterEach(() => {
    if (originalOverride != null) {
      process.env.HERMES_PARSER_NATIVE_ADDON = originalOverride;
    } else {
      delete process.env.HERMES_PARSER_NATIVE_ADDON;
    }
    jest.resetModules();
  });

  test('the in-repo dev build and the packaged prebuild both exist on disk', () => {
    // If this precondition doesn't hold, the precedence test below would
    // pass vacuously (there'd only be one candidate to "win"), which would
    // make it a test that can't fail. Assert it explicitly instead of
    // silently skipping, so a missing binary shows up as a loud failure
    // here rather than a silently-weakened guarantee below.
    expect(fs.existsSync(devBuildPath)).toBe(true);
    expect(fs.existsSync(prebuiltPath)).toBe(true);
  });

  test('the dev build takes precedence over the packaged prebuild when both exist', () => {
    jest.resetModules();
    delete process.env.HERMES_PARSER_NATIVE_ADDON;

    const loadAddon = require('../src/HermesParserAddon');
    const addon = loadAddon();

    // Prove identity via require.cache rather than inferring resolution
    // from whether parsing merely succeeds: both binaries are built from
    // the same source and would parse identically.
    const loadedNodeModules = Object.keys(require.cache).filter(p =>
      p.endsWith('.node'),
    );
    expect(loadedNodeModules).toContain(devBuildPath);
    expect(loadedNodeModules).not.toContain(prebuiltPath);

    // And confirm it's a real, working addon, not a stub.
    const result = addon.parse('var x = 1;', {});
    expect(result.buffer.byteLength).toBeGreaterThan(0);
  });
});
