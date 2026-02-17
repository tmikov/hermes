/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// End-to-end test for the interp-dispatch Wasm example.
// Tests both the AOT (.hbc) and runtime (.wasm) paths using the same
// WebAssembly API (WebAssembly.Module accepts both formats).

// REQUIRES: wasm

// RUN: %hermesc --wasm -emit-binary -out %t.hbc %S/../../examples/wasm/interp-dispatch/bench.wasm && %hermes -Xhermes-internal-test-methods %s -- %t.hbc | %FileCheck --match-full-lines %s
// RUN: %hermes -Xhermes-internal-test-methods %s -- %S/../../examples/wasm/interp-dispatch/bench.wasm | %FileCheck --match-full-lines %s

var path = hermescli.getScriptArgs()[0];
var imports = {
  env: {
    print: function(value) { print(value); }
  }
};

var bytes = hermescli.loadFile(path);
var mod = new WebAssembly.Module(bytes);
var instance = new WebAssembly.Instance(mod, imports);
var exports = instance.exports;

print(exports.bench(10, 100));
// CHECK: 9.332621544394418e+158
