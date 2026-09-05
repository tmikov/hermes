/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The Worker constructor takes raw bytes and runs them through
// evaluateJavaScript, which decides source-vs-bytecode by content. Hermes
// bytecode is trusted by construction -- it is not re-validated the way source
// is -- so that path is a trust boundary, gated on
// EnableUntrustedBytecodeFromJS.
//
// Two things are asserted here, and the second is the reason the gate is not
// simply "reject binary input":
//   1. bytecode from JS is refused when the flag is off, and
//   2. SOURCE in an ArrayBuffer still works, because it is not bytecode and
//      has nothing to do with the trust boundary.

// RUN: echo 'onmessage = function() { postMessage("ran"); };' > %t.body.js
// RUN: %hermesc -emit-binary -out %t.hbc %t.body.js
// RUN: %python %S/Inputs/hbc_to_js.py %t.hbc %t.pre.js
// RUN: cat %t.pre.js %s > %t.run.js
// RUN: %hermes %t.run.js | %FileCheck %s --match-full-lines

// `BC` is prepended by the RUN pipeline: the bytecode of the worker body above.
try {
  var w = new Worker(BC.buffer);
  // Terminate immediately. If the gate ever regresses, this branch runs, and
  // an un-terminated worker would keep the CLI event loop alive forever --
  // turning a clean CHECK mismatch into a hung test. Fail fast, not slowly.
  w.terminate();
  print("bytecode, gate off: ACCEPTED");
} catch (e) {
  print("bytecode, gate off: " + e.message);
}
// CHECK: bytecode, gate off: Cannot create Worker from Hermes bytecode (EnableUntrustedBytecodeFromJS is off)

function asciiBytes(str) {
  return Uint8Array.from(str, function (c) { return c.charCodeAt(0); });
}
var src = 'onmessage = function() { postMessage("source-ran"); };';

try {
  var w2 = new Worker(asciiBytes(src).buffer);
  w2.onmessage = function (msg) {
    print("source in a buffer, gate off: " + msg);
    w2.terminate();
  };
  w2.postMessage("go");
} catch (e) {
  print("source in a buffer, gate off: UNEXPECTEDLY REFUSED: " + e.message);
}
// CHECK-NEXT: source in a buffer, gate off: source-ran
