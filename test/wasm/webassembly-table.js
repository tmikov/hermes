/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck %s
// REQUIRES: wasm

// --- WebAssembly.Table constructor exists ---
print(typeof WebAssembly.Table);
// CHECK: function
print(WebAssembly.Table.length);
// CHECK-NEXT: 1

// --- Construct with initial size ---
var tbl = new WebAssembly.Table({element: "anyfunc", initial: 3});
print(typeof tbl);
// CHECK-NEXT: object
print(Object.prototype.toString.call(tbl));
// CHECK-NEXT: [object Table]
print(tbl instanceof WebAssembly.Table);
// CHECK-NEXT: true

// --- length getter returns the table size ---
print(tbl.length);
// CHECK-NEXT: 3

// --- get returns null for uninitialized entries ---
print(tbl.get(0));
// CHECK-NEXT: null
print(tbl.get(1));
// CHECK-NEXT: null
print(tbl.get(2));
// CHECK-NEXT: null

// --- set and get a function ---
function myFunc() { return 42; }
tbl.set(1, myFunc);
print(tbl.get(1) === myFunc);
// CHECK-NEXT: true
print(tbl.get(0));
// CHECK-NEXT: null

// --- set to null clears the entry ---
tbl.set(1, null);
print(tbl.get(1));
// CHECK-NEXT: null

// --- Construct with "funcref" element type ---
var tbl2 = new WebAssembly.Table({element: "funcref", initial: 2});
print(tbl2.length);
// CHECK-NEXT: 2

// --- grow returns old size ---
var tblGrow = new WebAssembly.Table({element: "anyfunc", initial: 2, maximum: 5});
print(tblGrow.length);
// CHECK-NEXT: 2
var oldLen = tblGrow.grow(2);
print(oldLen);
// CHECK-NEXT: 2
print(tblGrow.length);
// CHECK-NEXT: 4

// --- New entries after grow are null ---
print(tblGrow.get(2));
// CHECK-NEXT: null
print(tblGrow.get(3));
// CHECK-NEXT: null

// --- Old entries survive grow ---
function fn1() {}
var tblSurvive = new WebAssembly.Table({element: "anyfunc", initial: 2, maximum: 4});
tblSurvive.set(0, fn1);
tblSurvive.grow(1);
print(tblSurvive.get(0) === fn1);
// CHECK-NEXT: true

// --- grow by 0 is a no-op ---
var sz = tblSurvive.grow(0);
print(sz);
// CHECK-NEXT: 3

// --- Construct with 0 initial ---
var tbl0 = new WebAssembly.Table({element: "anyfunc", initial: 0});
print(tbl0.length);
// CHECK-NEXT: 0

// --- Error: grow beyond maximum ---
var tblMax = new WebAssembly.Table({element: "anyfunc", initial: 1, maximum: 2});
tblMax.grow(1);
try {
  tblMax.grow(1);
} catch (e) {
  print(e instanceof RangeError);
  // CHECK-NEXT: true
}

// --- Error: calling without new ---
try {
  WebAssembly.Table({element: "anyfunc", initial: 1});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: missing initial ---
try {
  new WebAssembly.Table({element: "anyfunc"});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: missing element ---
try {
  new WebAssembly.Table({initial: 1});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: invalid element type ---
try {
  new WebAssembly.Table({element: "externref", initial: 1});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: non-object argument ---
try {
  new WebAssembly.Table(42);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: initial > maximum ---
try {
  new WebAssembly.Table({element: "anyfunc", initial: 5, maximum: 2});
} catch (e) {
  print(e instanceof RangeError);
  // CHECK-NEXT: true
}

// --- Error: get out of bounds ---
try {
  tbl.get(100);
} catch (e) {
  print(e instanceof RangeError);
  // CHECK-NEXT: true
}

// --- Error: set out of bounds ---
try {
  tbl.set(100, null);
} catch (e) {
  print(e instanceof RangeError);
  // CHECK-NEXT: true
}

// --- Error: set non-function value ---
try {
  tbl.set(0, 42);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- length is a getter on the prototype ---
var desc = Object.getOwnPropertyDescriptor(
    WebAssembly.Table.prototype, 'length');
print(typeof desc.get);
// CHECK-NEXT: function
print(desc.set);
// CHECK-NEXT: undefined

// --- Error: length getter on non-Table ---
try {
  desc.get.call({});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: get on non-Table ---
try {
  WebAssembly.Table.prototype.get.call({}, 0);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: set on non-Table ---
try {
  WebAssembly.Table.prototype.set.call({}, 0, null);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: grow on non-Table ---
try {
  WebAssembly.Table.prototype.grow.call({}, 1);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}
