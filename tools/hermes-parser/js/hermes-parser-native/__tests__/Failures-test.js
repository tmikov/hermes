/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

const path = require('path');

const {parse} = require('../src/HermesParser');

describe('failure modes', () => {
  test('unsupported platform names the platform and the supported set', () => {
    jest.resetModules();
    const originalPlatform = process.platform;
    const originalOverride = process.env.HERMES_PARSER_NATIVE_ADDON;
    delete process.env.HERMES_PARSER_NATIVE_ADDON;
    Object.defineProperty(process, 'platform', {value: 'sunos'});

    // HermesParserAddon.js checks an in-repo development fallback *before*
    // prebuilds/<platform>-<arch>/: cmake-build-debug/tools/hermes-parser-native/
    // hermes-parser.node, computed relative to HermesParserAddon.js and
    // independent of process.platform. In this checkout that file exists
    // (it's what the test harness itself builds and runs against), so
    // simply stubbing process.platform is not enough to reach the
    // "unsupported platform" error: the loader would silently succeed via
    // that fallback instead (before ever getting to the platform-keyed
    // prebuilds/ candidate that 'sunos' is meant to defeat). Mock the
    // fallback's exact resolved path so requiring it fails the same way it
    // would in a published npm package (where the fallback path never
    // exists), forcing the loader through to the platform check and on to
    // the real error.
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
    jest.doMock(devBuildPath, () => {
      throw new Error('simulated: development fallback build not present');
    });

    try {
      const loadAddon = require('../src/HermesParserAddon');
      expect(() => loadAddon()).toThrow(/no prebuilt addon for sunos/);
      expect(() => loadAddon()).toThrow(/Supported platforms/);
    } finally {
      jest.dontMock(devBuildPath);
      Object.defineProperty(process, 'platform', {value: originalPlatform});
      if (originalOverride != null) {
        process.env.HERMES_PARSER_NATIVE_ADDON = originalOverride;
      }
      jest.resetModules();
    }
  });

  test('deeply nested input fails cleanly rather than crashing', () => {
    // Established empirically (see task-10-report.md): the parser's
    // recursionDepthCheck() guard (lib/Parser/JSParserImpl.h) trips at a
    // depth of 1024 on this platform/build, well short of a real native
    // stack overflow. Verified by temporarily raising
    // JSParserImpl::MAX_RECURSION_DEPTH to 1000000 (i.e. effectively
    // disabling the guard): with the guard disabled, this exact depth
    // (5000) segfaults the process. With the guard in place it reliably
    // throws the specific SyntaxError below, at every depth tried from
    // 1024 up to 500000. So this is not merely "throws something or
    // nothing" -- it is a specific, load-bearing error path, and the
    // assertions below pin it down instead of accepting either outcome.
    const depth = 5000;
    const source = '('.repeat(depth) + '1' + ')'.repeat(depth);

    let threw = null;
    try {
      parse(source, {});
    } catch (e) {
      threw = e;
    }

    expect(threw).toBeInstanceOf(SyntaxError);
    expect(threw.message).toMatch(
      /Too many nested expressions\/statements\/declarations/,
    );
    expect(threw.loc.line).toBeGreaterThan(0);
    expect(threw.loc.column).toBeGreaterThanOrEqual(0);
  });

  test('syntax error carries line and column', () => {
    let err = null;
    try {
      parse('function f( {\n', {});
    } catch (e) {
      err = e;
    }
    expect(err).toBeInstanceOf(SyntaxError);
    expect(err.loc.line).toBeGreaterThan(0);
    expect(err.loc.column).toBeGreaterThanOrEqual(0);
  });
});

describe('kind hash guard', () => {
  test('rejects a container whose hash does not match', () => {
    const addon = require('../src/HermesParserAddon')();
    const result = addon.parse('var x = 1;', {});
    const header = new Uint32Array(result.buffer, 0, 12);

    // Corrupt the hash and re-run the checking path by calling parse with a
    // stubbed addon that returns the mutated container.
    header[2] = header[2] ^ 0xffffffff;

    jest.resetModules();
    jest.doMock('../src/HermesParserAddon', () => () => ({
      parse: () => result,
    }));
    const {parse: parseWithStub} = require('../src/HermesParser');

    expect(() => parseWithStub('var x = 1;', {})).toThrow(
      /node-kind table mismatch/,
    );

    jest.dontMock('../src/HermesParserAddon');
    jest.resetModules();
  });
});
