/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck %s
// REQUIRES: wasm

// --- WebAssembly.compile and WebAssembly.instantiate exist ---
print(typeof WebAssembly.compile);
// CHECK: function
print(typeof WebAssembly.instantiate);
// CHECK-NEXT: function
print(WebAssembly.compile.length);
// CHECK-NEXT: 1
print(WebAssembly.instantiate.length);
// CHECK-NEXT: 1

// --- Minimal valid Wasm module ---
var minimal = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d,
  0x01, 0x00, 0x00, 0x00
]);

// --- Module with exported function returning 42 ---
var withExport = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d,
  0x01, 0x00, 0x00, 0x00,
  // Type section: () -> i32
  0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f,
  // Function section
  0x03, 0x02, 0x01, 0x00,
  // Export section: "answer" -> func 0
  0x07, 0x0a, 0x01,
  0x06, 0x61, 0x6e, 0x73, 0x77, 0x65, 0x72,
  0x00, 0x00,
  // Code section: i32.const 42
  0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x2a, 0x0b
]);

// --- Module with import ---
var withImport = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d,
  0x01, 0x00, 0x00, 0x00,
  // Type section: (i32, i32) -> i32
  0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
  // Import section: env.add
  0x02, 0x0b, 0x01,
  0x03, 0x65, 0x6e, 0x76,
  0x03, 0x61, 0x64, 0x64,
  0x00, 0x00,
  // Function section
  0x03, 0x02, 0x01, 0x00,
  // Export section: callAdd -> func 1
  0x07, 0x0b, 0x01,
  0x07, 0x63, 0x61, 0x6c, 0x6c, 0x41, 0x64, 0x64,
  0x00, 0x01,
  // Code section: call env.add(3, 4)
  0x0a, 0x0a, 0x01, 0x08, 0x00,
  0x41, 0x03, 0x41, 0x04, 0x10, 0x00, 0x0b
]);

// ==========================================================================
// Synchronous tests (these run before Promise callbacks)
// ==========================================================================

// --- compile returns a Promise ---
var p = WebAssembly.compile(minimal);
print(p instanceof Promise);
// CHECK-NEXT: true

// --- compile TypeError for non-buffer (thrown synchronously) ---
try {
  WebAssembly.compile(42);
} catch (e) {
  print("compile-typeerror: " + (e instanceof TypeError));
}
// CHECK-NEXT: compile-typeerror: true

// ==========================================================================
// Promise-based tests (callbacks run after all synchronous code)
// ==========================================================================

// --- compile resolves to a Module ---
WebAssembly.compile(minimal).then(function(mod) {
  print("compile-minimal: " + (mod instanceof WebAssembly.Module));
});
// CHECK: compile-minimal: true

// --- compile of module with exports ---
WebAssembly.compile(withExport).then(function(mod) {
  var descs = WebAssembly.Module.exports(mod);
  print("compile-exports: " + descs.length + " " + descs[0].name + " " + descs[0].kind);
});
// CHECK: compile-exports: 1 answer function

// --- compile rejects on invalid bytes ---
WebAssembly.compile(new Uint8Array([0, 0, 0, 0])).then(
  function() { print("compile-invalid: FAIL"); },
  function(err) {
    print("compile-invalid: " + (err instanceof WebAssembly.CompileError));
  }
);
// CHECK: compile-invalid: true

// --- instantiate(bytes) returns Promise resolving to {module, instance} ---
WebAssembly.instantiate(withExport).then(function(result) {
  print("instantiate-bytes-module: " + (result.module instanceof WebAssembly.Module));
  print("instantiate-bytes-instance: " + (result.instance instanceof WebAssembly.Instance));
  print("instantiate-bytes-answer: " + result.instance.exports.answer());
});
// CHECK: instantiate-bytes-module: true
// CHECK-NEXT: instantiate-bytes-instance: true
// CHECK-NEXT: instantiate-bytes-answer: 42

// --- instantiate(bytes, imports) with import object ---
WebAssembly.instantiate(withImport, {
  env: { add: function(a, b) { return a + b; } }
}).then(function(result) {
  print("instantiate-bytes-import: " + result.instance.exports.callAdd());
});
// CHECK: instantiate-bytes-import: 7

// --- instantiate(bytes) rejects on invalid bytes ---
WebAssembly.instantiate(new Uint8Array([0, 0, 0, 0])).then(
  function() { print("instantiate-invalid: FAIL"); },
  function(err) {
    print("instantiate-invalid: " + (err instanceof WebAssembly.CompileError));
  }
);
// CHECK: instantiate-invalid: true

// --- instantiate(module) returns Promise resolving to instance ---
var mod = new WebAssembly.Module(withExport);
WebAssembly.instantiate(mod).then(function(inst) {
  print("instantiate-mod-type: " + (inst instanceof WebAssembly.Instance));
  print("instantiate-mod-answer: " + inst.exports.answer());
});
// CHECK: instantiate-mod-type: true
// CHECK-NEXT: instantiate-mod-answer: 42

// --- instantiate(module, imports) ---
var mod2 = new WebAssembly.Module(withImport);
WebAssembly.instantiate(mod2, {
  env: { add: function(a, b) { return a + b; } }
}).then(function(inst) {
  print("instantiate-mod-import: " + inst.exports.callAdd());
});
// CHECK: instantiate-mod-import: 7

// --- instantiate(module) rejects when imports missing ---
WebAssembly.instantiate(mod2).then(
  function() { print("instantiate-missing-import: FAIL"); },
  function(err) {
    print("instantiate-missing-import: " + (err instanceof WebAssembly.LinkError));
  }
);
// CHECK: instantiate-missing-import: true
