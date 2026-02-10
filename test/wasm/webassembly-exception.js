/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck %s
// REQUIRES: wasm

// --- WebAssembly.Exception constructor exists ---
print(typeof WebAssembly.Exception);
// CHECK: function
print(WebAssembly.Exception.length);
// CHECK-NEXT: 2

// --- Construct an Exception with a Tag ---
var tag1 = new WebAssembly.Tag({parameters: ['i32']});
var exc1 = new WebAssembly.Exception(tag1, [42]);
print(typeof exc1);
// CHECK-NEXT: object
print(Object.prototype.toString.call(exc1));
// CHECK-NEXT: [object Exception]
print(exc1 instanceof WebAssembly.Exception);
// CHECK-NEXT: true

// --- is() method: identity check ---
print(exc1.is(tag1));
// CHECK-NEXT: true

// --- is() returns false for different tag ---
var tag2 = new WebAssembly.Tag({parameters: ['i32']});
print(exc1.is(tag2));
// CHECK-NEXT: false

// --- getArg() extracts payload ---
print(exc1.getArg(tag1, 0));
// CHECK-NEXT: 42

// --- Multi-parameter exception ---
var tagMulti = new WebAssembly.Tag({parameters: ['i32', 'f64']});
var excMulti = new WebAssembly.Exception(tagMulti, [100, 3.14]);
print(excMulti.is(tagMulti));
// CHECK-NEXT: true
print(excMulti.getArg(tagMulti, 0));
// CHECK-NEXT: 100
print(excMulti.getArg(tagMulti, 1));
// CHECK-NEXT: 3.14

// --- i32 truncation in Exception payload ---
var excTrunc = new WebAssembly.Exception(tag1, [2147483648]);
print(excTrunc.getArg(tag1, 0));
// CHECK-NEXT: -2147483648

// --- f32 narrowing in Exception payload ---
var tagF32 = new WebAssembly.Tag({parameters: ['f32']});
var excF32 = new WebAssembly.Exception(tagF32, [1.1]);
print(excF32.getArg(tagF32, 0));
// CHECK-NEXT: 1.100000023841858

// --- Empty parameters tag ---
var tagEmpty = new WebAssembly.Tag({parameters: []});
var excEmpty = new WebAssembly.Exception(tagEmpty, []);
print(excEmpty.is(tagEmpty));
// CHECK-NEXT: true

// --- Error: getArg with wrong tag ---
try {
  excMulti.getArg(tag1, 0);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: getArg with out-of-range index ---
try {
  exc1.getArg(tag1, 1);
} catch (e) {
  print(e instanceof RangeError);
  // CHECK-NEXT: true
}

// --- Error: getArg with negative index ---
try {
  exc1.getArg(tag1, -1);
} catch (e) {
  print(e instanceof RangeError);
  // CHECK-NEXT: true
}

// --- Error: calling without new ---
try {
  WebAssembly.Exception(tag1, [42]);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: first arg not a Tag ---
try {
  new WebAssembly.Exception({}, [42]);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: second arg not array-like ---
try {
  new WebAssembly.Exception(tag1, 42);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: is() with non-Tag argument ---
try {
  exc1.is({});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: is() on non-Exception ---
try {
  WebAssembly.Exception.prototype.is.call({}, tag1);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: getArg() on non-Exception ---
try {
  WebAssembly.Exception.prototype.getArg.call({}, tag1, 0);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Prototype methods exist ---
print(typeof WebAssembly.Exception.prototype.is);
// CHECK-NEXT: function
print(typeof WebAssembly.Exception.prototype.getArg);
// CHECK-NEXT: function
