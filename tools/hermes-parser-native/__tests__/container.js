/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

'use strict';

const assert = require('assert');
const path = require('path');

const addon = require(path.resolve(process.argv[2]));

// --- successful parse returns a container ---
const ok = addon.parse('var x = 1;', {});
assert.ok(ok.buffer instanceof ArrayBuffer, 'expected an ArrayBuffer');
assert.strictEqual(ok.error, undefined);

const header = new Uint32Array(ok.buffer, 0, 12);
assert.strictEqual(header[0], 0x484d5052, 'magic');
assert.strictEqual(header[1], 1, 'format version');
assert.notStrictEqual(header[2], 0, 'kind hash must be set');
assert.strictEqual(header[3], 48, 'program region starts after the header');
assert.strictEqual(header[3] % 8, 0, 'program region must be 8-byte aligned');
assert.ok(header[4] > 0, 'program region must be non-empty');

// Region bounds must all lie inside the buffer.
const total = ok.buffer.byteLength;
assert.ok(header[5] + header[6] * 20 <= total, 'positions in bounds');
assert.ok(header[9] + header[10] <= total, 'string data in bounds');

// --- syntax error returns a descriptor, not a throw ---
const bad = addon.parse('var = ;', {});
assert.strictEqual(bad.buffer, undefined);
assert.strictEqual(typeof bad.error, 'string');
assert.ok(bad.error.length > 0);
assert.strictEqual(typeof bad.line, 'number');
assert.strictEqual(typeof bad.column, 'number');

// --- the string table holds the identifier exactly once ---
const withDupes = addon.parse('var foo; foo; foo; foo;', {});
const h2 = new Uint32Array(withDupes.buffer, 0, 12);
const strCount = h2[8];
const strOffsets = new Uint32Array(withDupes.buffer, h2[7], strCount + 1);
const strData = new Uint8Array(withDupes.buffer, h2[9], h2[10]);
let fooCount = 0;
for (let i = 0; i < strCount; i++) {
  const s = Buffer.from(
    strData.subarray(strOffsets[i], strOffsets[i + 1]),
  ).toString('utf8');
  if (s === 'foo') fooCount++;
}
assert.strictEqual(fooCount, 1, 'identifier must be interned once');

console.log('container OK');
