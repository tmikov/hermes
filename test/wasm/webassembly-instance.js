/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck %s
// REQUIRES: wasm

// --- Instance constructor exists ---
print(typeof WebAssembly.Instance);
// CHECK: function
print(WebAssembly.Instance.length);
// CHECK-NEXT: 1

// --- Instance @@toStringTag ---
print(Object.prototype.toString.call(WebAssembly.Instance.prototype));
// CHECK-NEXT: [object Instance]

// --- Instantiate minimal module with no imports ---
var minimal = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, // magic: \0asm
  0x01, 0x00, 0x00, 0x00  // version: 1
]);
var mod = new WebAssembly.Module(minimal);
var inst = new WebAssembly.Instance(mod);
print(typeof inst);
// CHECK-NEXT: object
print(inst instanceof WebAssembly.Instance);
// CHECK-NEXT: true

// --- Instance has frozen exports object ---
print(typeof inst.exports);
// CHECK-NEXT: object
print(Object.isFrozen(inst.exports));
// CHECK-NEXT: true

// --- Instantiate module with exported function ---
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

var mod2 = new WebAssembly.Module(withExport);
var inst2 = new WebAssembly.Instance(mod2);

// Exported function should be callable
print(typeof inst2.exports.answer);
// CHECK-NEXT: function
print(inst2.exports.answer());
// CHECK-NEXT: 42

// --- Instantiate module with function imports ---
// Module importing "env"."add" (i32, i32) -> i32
// and exporting "callAdd" which calls the import with (3, 4)
var withImport = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d,  // magic
  0x01, 0x00, 0x00, 0x00,  // version

  // Type section
  0x01,                     // section id
  0x07,                     // section size
  0x01,                     // count: 1 type
  0x60,                     // func type
  0x02, 0x7f, 0x7f,         // 2 params: i32, i32
  0x01, 0x7f,               // 1 result: i32

  // Import section
  0x02,                     // section id
  0x0b,                     // section size (11 bytes)
  0x01,                     // count: 1 import
  0x03,                     // module name length: 3
  0x65, 0x6e, 0x76,        // "env"
  0x03,                     // field name length: 3
  0x61, 0x64, 0x64,        // "add"
  0x00,                     // kind: function
  0x00,                     // type index: 0

  // Function section
  0x03,                     // section id
  0x02,                     // section size
  0x01,                     // count: 1 function
  0x00,                     // type index: 0

  // Export section
  0x07,                     // section id
  0x0b,                     // section size (11 bytes)
  0x01,                     // count: 1 export
  0x07,                     // name length: 7
  0x63, 0x61, 0x6c, 0x6c, 0x41, 0x64, 0x64,  // "callAdd"
  0x00,                     // kind: function
  0x01,                     // function index: 1 (defined func, after import)

  // Code section
  0x0a,                     // section id
  0x0a,                     // section size
  0x01,                     // count: 1 body
  0x08,                     // body size
  0x00,                     // local count: 0
  0x41, 0x03,               // i32.const 3
  0x41, 0x04,               // i32.const 4
  0x10, 0x00,               // call 0 (imported function)
  0x0b                      // end
]);

var mod3 = new WebAssembly.Module(withImport);
var inst3 = new WebAssembly.Instance(mod3, {
  env: {
    add: function(a, b) { return a + b; }
  }
});
print(inst3.exports.callAdd());
// CHECK-NEXT: 7

// --- Data segment initialization ---
// Module with memory and a data segment that writes bytes at offset 0
var withData = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d,  // magic
  0x01, 0x00, 0x00, 0x00,  // version

  // Type section
  0x01,                     // section id
  0x05,                     // section size
  0x01,                     // count: 1 type
  0x60,                     // func type
  0x00,                     // 0 params
  0x01, 0x7f,               // 1 result: i32

  // Function section
  0x03,                     // section id
  0x02,                     // section size
  0x01,                     // count: 1 function
  0x00,                     // type index: 0

  // Memory section
  0x05,                     // section id
  0x03,                     // section size
  0x01,                     // count: 1 memory
  0x00,                     // limits: no max
  0x01,                     // initial: 1 page

  // Export section
  0x07,                     // section id
  0x0d,                     // section size (13 bytes)
  0x01,                     // count: 1 export
  0x09,                     // name length: 9
  0x6c, 0x6f, 0x61, 0x64, 0x42, 0x79, 0x74, 0x65, 0x30,  // "loadByte0"
  0x00,                     // kind: function
  0x00,                     // function index: 0

  // Code section
  0x0a,                     // section id
  0x09,                     // section size
  0x01,                     // count: 1 body
  0x07,                     // body size
  0x00,                     // local count: 0
  0x41, 0x00,               // i32.const 0
  0x28, 0x02, 0x00,         // i32.load offset=0 align=4
  0x0b,                     // end

  // Data section (id=11)
  0x0b,                     // section id
  0x0a,                     // section size (10 bytes)
  0x01,                     // count: 1 segment
  0x00,                     // flags: active, memory 0
  0x41, 0x00, 0x0b,         // offset: i32.const 0, end
  0x04,                     // data size: 4 bytes
  0x2a, 0x00, 0x00, 0x00   // data: 42 in little-endian
]);

var mod4 = new WebAssembly.Module(withData);
var inst4 = new WebAssembly.Instance(mod4);
print(inst4.exports.loadByte0());
// CHECK-NEXT: 42

// --- Start function runs during instantiation ---
// Module with a start function that calls an imported function
var withStart = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d,  // magic
  0x01, 0x00, 0x00, 0x00,  // version

  // Type section
  0x01,                     // section id
  0x04,                     // section size
  0x01,                     // count: 1 type
  0x60,                     // func type
  0x00,                     // 0 params
  0x00,                     // 0 results

  // Import section
  0x02,                     // section id
  0x0f,                     // section size (15 bytes)
  0x01,                     // count: 1 import
  0x03,                     // module name length: 3
  0x65, 0x6e, 0x76,        // "env"
  0x07,                     // field name length: 7
  0x6f, 0x6e, 0x53, 0x74, 0x61, 0x72, 0x74,  // "onStart"
  0x00,                     // kind: function
  0x00,                     // type index: 0

  // Function section
  0x03,                     // section id
  0x02,                     // section size
  0x01,                     // count: 1 function
  0x00,                     // type index: 0

  // Start section (id=8)
  0x08,                     // section id
  0x01,                     // section size
  0x01,                     // start function index: 1 (defined func)

  // Code section
  0x0a,                     // section id
  0x06,                     // section size
  0x01,                     // count: 1 body
  0x04,                     // body size
  0x00,                     // local count: 0
  0x10, 0x00,               // call 0 (imported onStart)
  0x0b                      // end
]);

var startCalled = false;
var mod5 = new WebAssembly.Module(withStart);
var inst5 = new WebAssembly.Instance(mod5, {
  env: {
    onStart: function() { startCalled = true; }
  }
});
print(startCalled);
// CHECK-NEXT: true

// --- Error: calling without new ---
try {
  WebAssembly.Instance(mod);
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: first arg is not a Module ---
try {
  new WebAssembly.Instance({});
} catch (e) {
  print(e instanceof TypeError);
  // CHECK-NEXT: true
}

// --- Error: missing import object when module has imports ---
try {
  new WebAssembly.Instance(mod3);
} catch (e) {
  print(e instanceof WebAssembly.LinkError);
  // CHECK-NEXT: true
}
