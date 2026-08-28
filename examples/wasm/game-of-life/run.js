/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// JS driver for the Game of Life Wasm module. Accepts both .wasm and .hbc files.
//
// Usage:
//   hermes -Xhermes-internal-test-methods run.js -- life.wasm
//   hermesc --wasm -emit-binary -out life.hbc life.wasm
//   hermes -Xhermes-internal-test-methods run.js -- life.hbc

var path = hermescli.getScriptArgs()[0];
var bytes = hermescli.loadFile(path);

var mod = new WebAssembly.Module(bytes);
var instance = new WebAssembly.Instance(mod, {
  env: {
    log: function(value) { console.log(value); }
  }
});

var t0 = Date.now();
instance.exports.run(2000);
console.log("elapsed:", Date.now() - t0, "ms");
