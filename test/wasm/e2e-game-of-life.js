/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// End-to-end test for the Game of Life Wasm example.
// Tests both the AOT (.hbc, via WebAssembly.Module.fromHermesBytecode) and
// runtime (.wasm, via WebAssembly.Module) paths.

// REQUIRES: wasm

// RUN: %hermesc --wasm -emit-binary -out %t.hbc %S/../../examples/wasm/game-of-life/life.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %s -- %t.hbc | %FileCheck --match-full-lines %s
// RUN: %hermes -Xhermes-internal-test-methods %s -- %S/../../examples/wasm/game-of-life/life.wasm | %FileCheck --match-full-lines %s

var path = hermescli.getScriptArgs()[0];
var imports = {
  env: {
    log: function(value) { print(value); }
  }
};

var bytes = hermescli.loadFile(path);
var mod = path.endsWith('.hbc')
  ? WebAssembly.Module.fromHermesBytecode(bytes)
  : new WebAssembly.Module(bytes);
var instance = new WebAssembly.Instance(mod, imports);
var exports = instance.exports;

exports.run(5);
// CHECK: 9
