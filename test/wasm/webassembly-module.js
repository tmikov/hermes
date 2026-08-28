/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck %s
// REQUIRES: wasm

// --- WebAssembly.Module constructor exists ---
print(typeof WebAssembly.Module);
// CHECK: function
print(WebAssembly.Module.length);
// CHECK-NEXT: 1

// --- Construct from minimal valid module ---
var minimal = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, // magic: \0asm
  0x01, 0x00, 0x00, 0x00  // version: 1
]);
var mod = new WebAssembly.Module(minimal);
print(typeof mod);
// CHECK-NEXT: object
print(Object.prototype.toString.call(mod));
// CHECK-NEXT: [object Module]
print(mod instanceof WebAssembly.Module);
// CHECK-NEXT: true

// --- Module.exports on minimal module (no exports) ---
var exps = WebAssembly.Module.exports(mod);
print(Array.isArray(exps));
// CHECK-NEXT: true
print(exps.length);
// CHECK-NEXT: 0

// --- Module.imports on minimal module (no imports) ---
var imps = WebAssembly.Module.imports(mod);
print(Array.isArray(imps));
// CHECK-NEXT: true
print(imps.length);
// CHECK-NEXT: 0

// --- Module with an exported function ---
// Module:
//   Type section: one type () -> i32
//   Function section: one function of type 0
//   Export section: one export "answer" -> function 0
//   Code section: function body returning i32.const 42
var withExport = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d,  // magic
  0x01, 0x00, 0x00, 0x00,  // version

  // Type section (id=1)
  0x01,                     // section id
  0x05,                     // section size
  0x01,                     // count: 1 type
  0x60,                     // func type
  0x00,                     // 0 params
  0x01, 0x7f,               // 1 result: i32

  // Function section (id=3)
  0x03,                     // section id
  0x02,                     // section size
  0x01,                     // count: 1 function
  0x00,                     // type index: 0

  // Export section (id=7)
  0x07,                     // section id
  0x0a,                     // section size (10 bytes)
  0x01,                     // count: 1 export
  0x06,                     // name length: 6
  0x61, 0x6e, 0x73, 0x77, 0x65, 0x72,  // "answer"
  0x00,                     // kind: function
  0x00,                     // function index: 0

  // Code section (id=10)
  0x0a,                     // section id
  0x06,                     // section size
  0x01,                     // count: 1 body
  0x04,                     // body size
  0x00,                     // local count: 0
  0x41, 0x2a,               // i32.const 42
  0x0b                      // end
]);

var modExp = new WebAssembly.Module(withExport);
var exps2 = WebAssembly.Module.exports(modExp);
print(exps2.length);
// CHECK-NEXT: 1
print(exps2[0].name);
// CHECK-NEXT: answer
print(exps2[0].kind);
// CHECK-NEXT: function

// --- Module with an exported memory ---
// Module with a memory section and an export for it.
var withMemExport = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d,  // magic
  0x01, 0x00, 0x00, 0x00,  // version

  // Memory section (id=5)
  0x05,                     // section id
  0x03,                     // section size
  0x01,                     // count: 1 memory
  0x00,                     // limits: no max
  0x01,                     // initial: 1 page

  // Export section (id=7)
  0x07,                     // section id
  0x07,                     // section size
  0x01,                     // count: 1 export
  0x03,                     // name length: 3
  0x6d, 0x65, 0x6d,        // "mem"
  0x02,                     // kind: memory
  0x00                      // memory index: 0
]);

var modMem = new WebAssembly.Module(withMemExport);
var exps3 = WebAssembly.Module.exports(modMem);
print(exps3.length);
// CHECK-NEXT: 1
print(exps3[0].name);
// CHECK-NEXT: mem
print(exps3[0].kind);
// CHECK-NEXT: memory

// --- Module with imports ---
// Module importing a function "env"."log" (type () -> void)
// and a memory "env"."memory" (1 page).
var withImports = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d,  // magic
  0x01, 0x00, 0x00, 0x00,  // version

  // Type section (id=1)
  0x01,                     // section id
  0x04,                     // section size
  0x01,                     // count: 1 type
  0x60,                     // func type
  0x00,                     // 0 params
  0x00,                     // 0 results

  // Import section (id=2)
  0x02,                     // section id
  0x19,                     // section size (25 bytes)
  0x02,                     // count: 2 imports
  // Import 0: function "env"."log" type 0
  0x03,                     // module name length: 3
  0x65, 0x6e, 0x76,        // "env"
  0x03,                     // field name length: 3
  0x6c, 0x6f, 0x67,        // "log"
  0x00,                     // kind: function
  0x00,                     // type index: 0
  // Import 1: memory "env"."memory" (1 page, no max)
  0x03,                     // module name length: 3
  0x65, 0x6e, 0x76,        // "env"
  0x06,                     // field name length: 6
  0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79,  // "memory"
  0x02,                     // kind: memory
  0x00,                     // limits: no max
  0x01                      // initial: 1 page
]);

var modImp = new WebAssembly.Module(withImports);
var imps2 = WebAssembly.Module.imports(modImp);
print(imps2.length);
// CHECK-NEXT: 2
print(imps2[0].module);
// CHECK-NEXT: env
print(imps2[0].name);
// CHECK-NEXT: log
print(imps2[0].kind);
// CHECK-NEXT: function
print(imps2[1].module);
// CHECK-NEXT: env
print(imps2[1].name);
// CHECK-NEXT: memory
print(imps2[1].kind);
// CHECK-NEXT: memory

// --- Works with ArrayBuffer directly ---
var modBuf = new WebAssembly.Module(minimal.buffer);
print(typeof modBuf);
// CHECK-NEXT: object
print(modBuf instanceof WebAssembly.Module);
// CHECK-NEXT: true

// --- Error: calling without new ---
try {
  WebAssembly.Module(minimal);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: non-BufferSource argument ---
try {
  new WebAssembly.Module(42);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

try {
  new WebAssembly.Module("hello");
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: invalid Wasm bytes → CompileError ---
try {
  new WebAssembly.Module(new Uint8Array([0x00, 0x00, 0x00, 0x00]));
} catch (e) {
  print(e instanceof WebAssembly.CompileError);
  // CHECK-NEXT: true
}

// --- Error: Module.exports with non-Module argument ---
try {
  WebAssembly.Module.exports({});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: Module.imports with non-Module argument ---
try {
  WebAssembly.Module.imports(42);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}
