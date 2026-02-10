/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck %s
// REQUIRES: wasm

// Test WebAssembly namespace object exists.
print(typeof WebAssembly);
// CHECK: object
print(Object.prototype.toString.call(WebAssembly));
// CHECK-NEXT: [object WebAssembly]

// --- WebAssembly.CompileError ---
print(typeof WebAssembly.CompileError);
// CHECK-NEXT: function

var ce = new WebAssembly.CompileError("compile failed");
print(ce.message);
// CHECK-NEXT: compile failed
print(ce.name);
// CHECK-NEXT: CompileError
print(ce instanceof Error);
// CHECK-NEXT: true
print(ce instanceof WebAssembly.CompileError);
// CHECK-NEXT: true

// CompileError also works when called without new.
var ce2 = WebAssembly.CompileError("no new");
print(ce2 instanceof WebAssembly.CompileError);
// CHECK-NEXT: true
print(ce2.message);
// CHECK-NEXT: no new

// --- WebAssembly.LinkError ---
print(typeof WebAssembly.LinkError);
// CHECK-NEXT: function

var le = new WebAssembly.LinkError("link failed");
print(le.message);
// CHECK-NEXT: link failed
print(le.name);
// CHECK-NEXT: LinkError
print(le instanceof Error);
// CHECK-NEXT: true
print(le instanceof WebAssembly.LinkError);
// CHECK-NEXT: true

// LinkError is not a CompileError.
print(le instanceof WebAssembly.CompileError);
// CHECK-NEXT: false

// --- WebAssembly.RuntimeError ---
print(typeof WebAssembly.RuntimeError);
// CHECK-NEXT: function

var re = new WebAssembly.RuntimeError("runtime failed");
print(re.message);
// CHECK-NEXT: runtime failed
print(re.name);
// CHECK-NEXT: RuntimeError
print(re instanceof Error);
// CHECK-NEXT: true
print(re instanceof WebAssembly.RuntimeError);
// CHECK-NEXT: true

// RuntimeError is not a LinkError or CompileError.
print(re instanceof WebAssembly.LinkError);
// CHECK-NEXT: false
print(re instanceof WebAssembly.CompileError);
// CHECK-NEXT: false

// --- Error message is optional ---
var ce3 = new WebAssembly.CompileError();
print("msg:[" + ce3.message + "]");
// CHECK-NEXT: msg:[]

// --- Try/catch ---
try {
  throw new WebAssembly.RuntimeError("caught");
} catch (e) {
  print(e instanceof WebAssembly.RuntimeError);
  // CHECK-NEXT: true
  print(e.message);
  // CHECK-NEXT: caught
}

// --- Stack trace ---
var ce4 = new WebAssembly.CompileError("stack test");
print(typeof ce4.stack);
// CHECK-NEXT: string

// --- Error constructors are not global ---
print(typeof CompileError);
// CHECK-NEXT: undefined
print(typeof LinkError);
// CHECK-NEXT: undefined
print(typeof RuntimeError);
// CHECK-NEXT: undefined
