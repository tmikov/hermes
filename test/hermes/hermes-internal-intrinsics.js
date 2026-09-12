/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -enable-hermes-internal=true %s | %FileCheck --match-full-lines %s
// RUN: %hermes -enable-hermes-internal=false %s | %FileCheck --match-full-lines %s
// RUN: %shermes -exec %s -Wx,-enable-hermes-internal=true | %FileCheck --match-full-lines %s
// RUN: %shermes -exec %s -Wx,-enable-hermes-internal=false | %FileCheck --match-full-lines %s

// HermesInternal.intrinsics holds pristine constructors so that engine-
// generated code can allocate without going through a global that script can
// replace. The whole approach rests on script being unable to reach into the
// holder, so that is what this test pins: the property is non-writable and
// non-configurable, the holder is non-extensible, and every entry is
// non-writable and non-configurable.
//
// Both -enable-hermes-internal settings are run because the holder is defined
// above the flag check in createHermesInternalObject and so is unconditional.
// A generated Wasm module reads it whatever the embedder set that flag to.

'use strict';

var NAMES = [
  'Array',
  'ArrayBuffer',
  'Uint8Array',
  'Int8Array',
  'Uint8ClampedArray',
  'Uint16Array',
  'Int16Array',
  'Uint32Array',
  'Int32Array',
  'Float16Array',
  'Float32Array',
  'Float64Array',
  'BigUint64Array',
  'BigInt64Array',
];

function attrs(obj, name) {
  var d = Object.getOwnPropertyDescriptor(obj, name);
  if (!d) return 'missing';
  return 'w=' + d.writable + ',e=' + d.enumerable + ',c=' + d.configurable;
}

print('holder: ' + attrs(HermesInternal, 'intrinsics'));
// CHECK: holder: w=false,e=false,c=false

var I = HermesInternal.intrinsics;

print('holder extensible: ' + Object.isExtensible(I));
// CHECK-NEXT: holder extensible: false

print('HermesInternal extensible: ' + Object.isExtensible(HermesInternal));
// CHECK-NEXT: HermesInternal extensible: false

// Every entry must be present, locked down, and the same object the global
// currently names -- nothing has replaced a global yet at this point.
var bad = [];
for (var i = 0; i < NAMES.length; ++i) {
  var n = NAMES[i];
  if (attrs(I, n) !== 'w=false,e=false,c=false')
    bad.push(n + ':' + attrs(I, n));
  else if (I[n] !== globalThis[n])
    bad.push(n + ':not-pristine');
}
print('checked ' + NAMES.length + ', bad: [' + bad.join(',') + ']');
// CHECK-NEXT: checked 14, bad: []

// 'WebAssembly' is the one other entry the holder may carry; it is added at
// the end of initGlobalObject and covered by test/wasm/intrinsics-wasm.js.
var extra = Object.getOwnPropertyNames(I).filter(function(n) {
  return NAMES.indexOf(n) < 0 && n !== 'WebAssembly';
});
print('unexpected: [' + extra.join(',') + ']');
// CHECK-NEXT: unexpected: []

// Strict mode, so each of these throws rather than failing silently.
try {
  HermesInternal.intrinsics = {};
  print('replace holder: no throw');
} catch (e) {
  print('replace holder: ' + e.name);
}
// CHECK-NEXT: replace holder: TypeError

try {
  I.Float64Array = function() {};
  print('replace entry: no throw');
} catch (e) {
  print('replace entry: ' + e.name);
}
// CHECK-NEXT: replace entry: TypeError

try {
  I.somethingNew = 1;
  print('add entry: no throw');
} catch (e) {
  print('add entry: ' + e.name);
}
// CHECK-NEXT: add entry: TypeError

try {
  delete I.Float64Array;
  print('delete entry: no throw');
} catch (e) {
  print('delete entry: ' + e.name);
}
// CHECK-NEXT: delete entry: TypeError

// The point of the whole facility: replacing the global leaves the holder's
// copy alone. The second line proves the replacement actually took, so a
// no-op assignment cannot make the first line pass by accident.
var pristine = I.Float64Array;
globalThis.Float64Array = function() {
  return {};
};
print('holder survives replacement: ' + (I.Float64Array === pristine));
// CHECK-NEXT: holder survives replacement: true
print('global was replaced: ' + (globalThis.Float64Array !== pristine));
// CHECK-NEXT: global was replaced: true
