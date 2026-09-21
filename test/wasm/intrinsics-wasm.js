/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s
// REQUIRES: wasm

// The WebAssembly half of HermesInternal.intrinsics. It is filled in at the
// end of initGlobalObject rather than in createHermesInternalObject, because
// createWebAssemblyObject has not run by then; the holder is left extensible
// until that point and sealed as initGlobalObject's last act.
//
// test/hermes/hermes-internal-intrinsics.js covers the ECMAScript half and
// the holder's own lockdown. This file covers the sub-holder, and in
// particular that it stores the CONSTRUCTORS and not the namespace object:
// WebAssembly.Memory is an ordinary writable property, so a reference to the
// namespace would give script a way to swap the constructor out from under a
// module.

'use strict';

var NAMES = [
  'Module',
  'Instance',
  'Memory',
  'Table',
  'Global',
  'Tag',
  'Exception',
  'CompileError',
  'LinkError',
  'RuntimeError',
];

function attrs(obj, name) {
  var d = Object.getOwnPropertyDescriptor(obj, name);
  if (!d) return 'missing';
  return 'w=' + d.writable + ',e=' + d.enumerable + ',c=' + d.configurable;
}

var I = HermesInternal.intrinsics;

print('sub-holder: ' + attrs(I, 'WebAssembly'));
// CHECK: sub-holder: w=false,e=false,c=false

var W = I.WebAssembly;

print('sub-holder extensible: ' + Object.isExtensible(W));
// CHECK-NEXT: sub-holder extensible: false

var bad = [];
for (var i = 0; i < NAMES.length; ++i) {
  var n = NAMES[i];
  if (attrs(W, n) !== 'w=false,e=false,c=false')
    bad.push(n + ':' + attrs(W, n));
  else if (W[n] !== WebAssembly[n])
    bad.push(n + ':not-pristine');
}
print('checked ' + NAMES.length + ', bad: [' + bad.join(',') + ']');
// CHECK-NEXT: checked 10, bad: []

var extra = Object.getOwnPropertyNames(W).filter(function(n) {
  return NAMES.indexOf(n) < 0;
});
print('unexpected: [' + extra.join(',') + ']');
// CHECK-NEXT: unexpected: []

try {
  W.Memory = function() {};
  print('replace entry: no throw');
} catch (e) {
  print('replace entry: ' + e.name);
}
// CHECK-NEXT: replace entry: TypeError

// Capture the genuine namespace object before anything is tampered with.
var realNamespace = WebAssembly;
var pristineMemory = W.Memory;
var pristineTable = W.Table;

// Replacing the whole namespace binding leaves the sub-holder alone. The
// second line proves the replacement took, so the first cannot pass by
// accident.
globalThis.WebAssembly = {};
print('survives namespace replacement: ' + (W.Memory === pristineMemory));
// CHECK-NEXT: survives namespace replacement: true
print('namespace binding was replaced: ' + (globalThis.WebAssembly !== realNamespace));
// CHECK-NEXT: namespace binding was replaced: true

// So does overwriting a constructor ON the genuine namespace object. This is
// the case that a stashed reference to the namespace object would NOT have
// survived, since WebAssembly.Table is an ordinary writable property.
realNamespace.Table = function() {};
print('survives ctor overwrite: ' + (W.Table === pristineTable));
// CHECK-NEXT: survives ctor overwrite: true
print('ctor was overwritten: ' + (realNamespace.Table !== pristineTable));
// CHECK-NEXT: ctor was overwritten: true
