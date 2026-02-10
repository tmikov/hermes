/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck %s
// REQUIRES: wasm

// --- WebAssembly.Global constructor exists ---
print(typeof WebAssembly.Global);
// CHECK: function
print(WebAssembly.Global.length);
// CHECK-NEXT: 1

// --- Construct an i32 mutable global ---
var g1 = new WebAssembly.Global({value: "i32", mutable: true}, 42);
print(typeof g1);
// CHECK-NEXT: object
print(Object.prototype.toString.call(g1));
// CHECK-NEXT: [object Global]
print(g1 instanceof WebAssembly.Global);
// CHECK-NEXT: true

// --- value getter returns the initial value ---
print(g1.value);
// CHECK-NEXT: 42

// --- value setter modifies a mutable global ---
g1.value = 100;
print(g1.value);
// CHECK-NEXT: 100

// --- valueOf returns the same as value getter ---
print(g1.valueOf());
// CHECK-NEXT: 100

// --- i32 truncation ---
g1.value = 2147483648; // 2^31 wraps to -2^31
print(g1.value);
// CHECK-NEXT: -2147483648

// --- Construct an immutable i32 global ---
var g2 = new WebAssembly.Global({value: "i32"}, 7);
print(g2.value);
// CHECK-NEXT: 7

// --- Error: setting an immutable global ---
try {
  g2.value = 99;
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}
// Value should be unchanged.
print(g2.value);
// CHECK-NEXT: 7

// --- f64 global ---
var gf64 = new WebAssembly.Global({value: "f64", mutable: true}, 3.14);
print(gf64.value);
// CHECK-NEXT: 3.14
gf64.value = -0.5;
print(gf64.value);
// CHECK-NEXT: -0.5

// --- f32 global (value is narrowed to float precision) ---
var gf32 = new WebAssembly.Global({value: "f32"}, 1.1);
print(gf32.value);
// CHECK-NEXT: 1.100000023841858

// --- i64 global (stores as double, Phase 1 limitation) ---
var gi64 = new WebAssembly.Global({value: "i64", mutable: true}, 99);
print(gi64.value);
// CHECK-NEXT: 99

// --- Default initial value is 0 ---
var gDef = new WebAssembly.Global({value: "i32"});
print(gDef.value);
// CHECK-NEXT: 0

// --- mutable defaults to false ---
var gImm = new WebAssembly.Global({value: "f64"}, 1.5);
try {
  gImm.value = 2.5;
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}
print(gImm.value);
// CHECK-NEXT: 1.5

// --- Error: calling without new ---
try {
  WebAssembly.Global({value: "i32"}, 0);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: non-object descriptor ---
try {
  new WebAssembly.Global(42);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: missing value property ---
try {
  new WebAssembly.Global({mutable: true});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: invalid value type ---
try {
  new WebAssembly.Global({value: "v128"});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- value is a getter/setter on the prototype ---
var desc = Object.getOwnPropertyDescriptor(
    WebAssembly.Global.prototype, 'value');
print(typeof desc.get);
// CHECK-NEXT: function
print(typeof desc.set);
// CHECK-NEXT: function

// --- Error: value getter on non-Global ---
try {
  desc.get.call({});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: value setter on non-Global ---
try {
  desc.set.call({}, 5);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- valueOf method exists ---
print(typeof WebAssembly.Global.prototype.valueOf);
// CHECK-NEXT: function
