/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck %s
// REQUIRES: wasm

// Test WebAssembly.validate() exists and works correctly.
print(typeof WebAssembly.validate);
// CHECK: function
print(WebAssembly.validate.length);
// CHECK-NEXT: 1

// --- Valid minimal module ---
// The smallest valid Wasm module: magic \0asm + version 1.
var valid = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, // magic: \0asm
  0x01, 0x00, 0x00, 0x00  // version: 1
]);
print(WebAssembly.validate(valid));
// CHECK-NEXT: true

// Also works with the underlying ArrayBuffer directly.
print(WebAssembly.validate(valid.buffer));
// CHECK-NEXT: true

// --- Invalid module: bad magic ---
var badMagic = new Uint8Array([
  0x00, 0x00, 0x00, 0x00, // wrong magic
  0x01, 0x00, 0x00, 0x00
]);
print(WebAssembly.validate(badMagic));
// CHECK-NEXT: false

// --- Invalid module: truncated ---
var truncated = new Uint8Array([0x00, 0x61, 0x73]);
print(WebAssembly.validate(truncated));
// CHECK-NEXT: false

// --- Invalid module: empty buffer ---
var empty = new Uint8Array([]);
print(WebAssembly.validate(empty));
// CHECK-NEXT: false

// --- Invalid module: bad section ---
// Valid header + invalid section type (0xFF).
var badSection = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d,
  0x01, 0x00, 0x00, 0x00,
  0xFF, 0x00 // invalid section id
]);
print(WebAssembly.validate(badSection));
// CHECK-NEXT: false

// --- Valid module with a type section and function ---
// Module with one function type () -> i32, one function, and code.
var withFunc = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, // magic
  0x01, 0x00, 0x00, 0x00, // version

  // Type section (id=1)
  0x01,                   // section id
  0x05,                   // section size (5 bytes)
  0x01,                   // count: 1 type
  0x60,                   // func type
  0x00,                   // 0 params
  0x01, 0x7f,             // 1 result: i32

  // Function section (id=3)
  0x03,                   // section id
  0x02,                   // section size
  0x01,                   // count: 1 function
  0x00,                   // type index: 0

  // Code section (id=10)
  0x0a,                   // section id
  0x06,                   // section size
  0x01,                   // count: 1 body
  0x04,                   // body size
  0x00,                   // local count: 0
  0x41, 0x2a,             // i32.const 42
  0x0b                    // end
]);
print(WebAssembly.validate(withFunc));
// CHECK-NEXT: true

// --- TypeError for non-BufferSource argument ---
try {
  WebAssembly.validate(42);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

try {
  WebAssembly.validate("hello");
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

try {
  WebAssembly.validate(undefined);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Works with different typed array views ---
// Int8Array over a valid module buffer.
var i8 = new Int8Array(valid.buffer);
print(WebAssembly.validate(i8));
// CHECK-NEXT: true

// Uint32Array over a valid module buffer.
var u32 = new Uint32Array(valid.buffer);
print(WebAssembly.validate(u32));
// CHECK-NEXT: true
