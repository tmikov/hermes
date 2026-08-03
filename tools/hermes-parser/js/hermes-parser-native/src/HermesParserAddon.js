/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @flow strict-local
 * @format
 */

'use strict';

const path = require('path');

const SUPPORTED = [
  'linux-x64',
  'linux-arm64',
  'darwin-x64',
  'darwin-arm64',
];

/**
 * Locate and load the prebuilt addon for the running platform.
 *
 * The path can be overridden with HERMES_PARSER_NATIVE_ADDON, which the
 * in-tree test setup uses to point at a freshly built binary.
 */
function loadAddon() {
  const override = process.env.HERMES_PARSER_NATIVE_ADDON;
  if (override != null && override !== '') {
    /* $FlowFixMe[unsupported-syntax] dynamic require by design */
    return require(path.resolve(override));
  }

  const target = `${process.platform}-${process.arch}`;
  if (!SUPPORTED.includes(target)) {
    throw new Error(
      `hermes-parser-native: no prebuilt addon for ${target}. ` +
        `Supported platforms: ${SUPPORTED.join(', ')}.`,
    );
  }

  const addonPath = path.join(
    __dirname,
    '..',
    'prebuilds',
    target,
    'hermes-parser.node',
  );

  try {
    /* $FlowFixMe[unsupported-syntax] dynamic require by design */
    return require(addonPath);
  } catch (e) {
    throw new Error(
      `hermes-parser-native: failed to load the prebuilt addon for ${target} ` +
        `at ${addonPath}: ${e.message}`,
    );
  }
}

module.exports = loadAddon;
