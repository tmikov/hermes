/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

'use strict';

const assert = require('assert');
const path = require('path');

const addonPath = process.argv[2];
assert.ok(addonPath, 'usage: node smoke.js <path-to-hermes-parser.node>');

const addon = require(path.resolve(addonPath));
assert.strictEqual(typeof addon.parse, 'function', 'parse must be exported');

const result = addon.parse('var x = 1;', {});
assert.ok(result.buffer instanceof ArrayBuffer, 'parse must return a buffer');

console.log('smoke OK');
