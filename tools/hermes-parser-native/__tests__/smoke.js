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

let threw = null;
try {
  addon.parse('var x = 1;', {});
} catch (e) {
  threw = e;
}
assert.ok(threw, 'parse must throw while unimplemented');
assert.match(threw.message, /not implemented/);

console.log('smoke OK');
