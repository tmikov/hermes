/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck %s
// REQUIRES: wasm

// --- WebAssembly.Tag constructor exists ---
print(typeof WebAssembly.Tag);
// CHECK: function
print(WebAssembly.Tag.length);
// CHECK-NEXT: 1

// --- Construct a Tag with i32 parameter ---
var tag1 = new WebAssembly.Tag({parameters: ['i32']});
print(typeof tag1);
// CHECK-NEXT: object
print(Object.prototype.toString.call(tag1));
// CHECK-NEXT: [object Tag]
print(tag1 instanceof WebAssembly.Tag);
// CHECK-NEXT: true

// --- Construct a Tag with multiple parameters ---
var tag2 = new WebAssembly.Tag({parameters: ['i32', 'f64']});
print(tag2 instanceof WebAssembly.Tag);
// CHECK-NEXT: true

// --- Construct a Tag with no parameters ---
var tag3 = new WebAssembly.Tag({parameters: []});
print(tag3 instanceof WebAssembly.Tag);
// CHECK-NEXT: true

// --- Error: calling without new ---
try {
  WebAssembly.Tag({parameters: ['i32']});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: non-object argument ---
try {
  new WebAssembly.Tag(42);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: parameters not array-like ---
try {
  new WebAssembly.Tag({parameters: 42});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: invalid parameter type ---
try {
  new WebAssembly.Tag({parameters: ['v128']});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: non-string parameter type ---
try {
  new WebAssembly.Tag({parameters: [42]});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- All four value types ---
var tagI32 = new WebAssembly.Tag({parameters: ['i32']});
var tagI64 = new WebAssembly.Tag({parameters: ['i64']});
var tagF32 = new WebAssembly.Tag({parameters: ['f32']});
var tagF64 = new WebAssembly.Tag({parameters: ['f64']});
print("all types ok");
// CHECK-NEXT: all types ok
