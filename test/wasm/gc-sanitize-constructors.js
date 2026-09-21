/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The WebAssembly JS-API constructors parse their descriptors by allocating a
// comparison string per candidate type name, and coerce their arguments with
// toNumber_RJS, which runs user JS. Several of them used to hold a raw
// StringPrimitive*, a raw cell pointer, or a reference to a std::vector living
// INSIDE a movable cell across those allocations. Every one of those was a
// heap-use-after-free that only a moving heap exposes.
//
// This test exists to make that class of bug fail loudly. It must run with
// -gc-sanitize-handles=1 EXPLICITLY: a HERMESVM_SANITIZE_HANDLES=ON build
// samples at 1% by default, so without the flag these paths mostly run on a
// heap that did not move and the test reports green without exercising
// anything. In a build without HERMESVM_SANITIZE_HANDLES the flag is ignored
// and this is an ordinary behavioural test of the same constructors.

// RUN: %hermes -gc-sanitize-handles=1 %s | %FileCheck --match-full-lines %s
// REQUIRES: wasm

// --- WebAssembly.Global: the descriptor type-name comparison ---
// Every arm is exercised, because the parser allocates once per candidate and
// the failing arm is whichever one runs last.
print(new WebAssembly.Global({value: 'i32'}, 1).value);
// CHECK: 1
print(new WebAssembly.Global({value: 'f32'}, 1.5).value);
// CHECK-NEXT: 1.5
print(new WebAssembly.Global({value: 'f64'}, 2.25).value);
// CHECK-NEXT: 2.25
print(new WebAssembly.Global({value: 'i64'}, 7n).value);
// CHECK-NEXT: 7

// --- WebAssembly.Table: 'anyfunc' matches on the first comparison,
// --- 'funcref' only after a second string is allocated.
print(new WebAssembly.Table({element: 'anyfunc', initial: 2}).length);
// CHECK-NEXT: 2
print(new WebAssembly.Table({element: 'funcref', initial: 3}).length);
// CHECK-NEXT: 3

// --- WebAssembly.Tag: the parameter type names go through the same parser,
// --- once per parameter. 'f64' is last, so it allocates four times first.
var tag = new WebAssembly.Tag({parameters: ['i32', 'i64', 'f32', 'f64']});
print(tag instanceof WebAssembly.Tag);
// CHECK-NEXT: true

// --- WebAssembly.Exception: the tag's parameter-type vector lives inside the
// --- movable Tag cell, and the payload loop runs user JS twice per element
// --- before indexing it. The accessors below allocate, so the cell moves
// --- mid-loop.
var payload = {
  length: 4,
  get 0() { return {valueOf: function () { return [1, 2, 3] && 11; }}; },
  get 1() { return {valueOf: function () { return {} && 22; }}; },
  get 2() { return {valueOf: function () { return 'x'.repeat(9) && 33.5; }}; },
  get 3() { return {valueOf: function () { return [4].concat([5]) && 44.5; }}; },
};
var exc = new WebAssembly.Exception(tag, payload);
print(exc.getArg(tag, 0) + ' ' + exc.getArg(tag, 1));
// CHECK-NEXT: 11 22
print(exc.getArg(tag, 2) + ' ' + exc.getArg(tag, 3));
// CHECK-NEXT: 33.5 44.5

// --- Exception.prototype.getArg coerces its index with toNumber_RJS, which
// --- runs this valueOf; the exception and the tag must survive it.
print(exc.getArg(tag, {valueOf: function () { return [0, 1, 2] && 2; }}));
// CHECK-NEXT: 33.5

// The bounds check reads the tag's parameter count AFTER that same coercion.
try {
  exc.getArg(tag, {valueOf: function () { return {a: 1} && 4; }});
  print('no throw');
} catch (e) {
  print(e.constructor.name);
}
// CHECK-NEXT: RangeError

print('done');
// CHECK-NEXT: done
