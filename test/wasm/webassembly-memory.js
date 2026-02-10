/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck %s
// REQUIRES: wasm

// --- WebAssembly.Memory constructor exists ---
print(typeof WebAssembly.Memory);
// CHECK: function
print(WebAssembly.Memory.length);
// CHECK-NEXT: 1

// --- Construct with initial pages ---
var mem = new WebAssembly.Memory({initial: 1});
print(typeof mem);
// CHECK-NEXT: object
print(Object.prototype.toString.call(mem));
// CHECK-NEXT: [object Memory]
print(mem instanceof WebAssembly.Memory);
// CHECK-NEXT: true

// --- buffer property returns an ArrayBuffer ---
var buf = mem.buffer;
print(buf instanceof ArrayBuffer);
// CHECK-NEXT: true
print(buf.byteLength);
// CHECK-NEXT: 65536

// --- Create memory with 2 initial pages ---
var mem2 = new WebAssembly.Memory({initial: 2});
print(mem2.buffer.byteLength);
// CHECK-NEXT: 131072

// --- Create memory with 0 initial pages ---
var mem0 = new WebAssembly.Memory({initial: 0});
print(mem0.buffer.byteLength);
// CHECK-NEXT: 0

// --- Create memory with initial and maximum ---
var memMax = new WebAssembly.Memory({initial: 1, maximum: 4});
print(memMax.buffer.byteLength);
// CHECK-NEXT: 65536

// --- grow() returns old page count ---
var oldPages = memMax.grow(1);
print(oldPages);
// CHECK-NEXT: 1
print(memMax.buffer.byteLength);
// CHECK-NEXT: 131072

// --- grow by 0 is a no-op ---
var same = memMax.grow(0);
print(same);
// CHECK-NEXT: 2

// --- grow again ---
var old2 = memMax.grow(2);
print(old2);
// CHECK-NEXT: 2
print(memMax.buffer.byteLength);
// CHECK-NEXT: 262144

// --- Data survives grow ---
var mem3 = new WebAssembly.Memory({initial: 1, maximum: 2});
var view = new Uint8Array(mem3.buffer);
view[0] = 42;
view[100] = 99;
mem3.grow(1);
var view2 = new Uint8Array(mem3.buffer);
print(view2[0]);
// CHECK-NEXT: 42
print(view2[100]);
// CHECK-NEXT: 99

// --- Error: grow beyond maximum ---
var memLim = new WebAssembly.Memory({initial: 1, maximum: 2});
memLim.grow(1); // Now at 2 pages (the max).
try {
  memLim.grow(1); // Would exceed maximum.
} catch (e) {
  print(e instanceof RangeError);
  // CHECK-NEXT: true
}

// --- Error: calling without new ---
try {
  WebAssembly.Memory({initial: 1});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: missing initial ---
try {
  new WebAssembly.Memory({});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: non-object argument ---
try {
  new WebAssembly.Memory(42);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: initial > maximum ---
try {
  new WebAssembly.Memory({initial: 3, maximum: 2});
} catch (e) {
  print(e instanceof RangeError);
  // CHECK-NEXT: true
}

// --- Error: negative initial ---
try {
  new WebAssembly.Memory({initial: -1});
} catch (e) {
  print(e instanceof RangeError);
  // CHECK-NEXT: true
}

// --- grow is a method on the prototype ---
print(typeof WebAssembly.Memory.prototype.grow);
// CHECK-NEXT: function

// --- buffer is a getter on the prototype ---
var desc = Object.getOwnPropertyDescriptor(
    WebAssembly.Memory.prototype, 'buffer');
print(typeof desc.get);
// CHECK-NEXT: function
print(desc.set);
// CHECK-NEXT: undefined

// --- Error: buffer getter on non-Memory ---
try {
  desc.get.call({});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: grow on non-Memory ---
try {
  WebAssembly.Memory.prototype.grow.call({}, 1);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}
