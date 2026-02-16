/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// JS driver that loads and runs the Wasm module using Node.js.
//
// Usage:
//   node node-run.js bench.wasm

var fs = require("fs");
var wasmPath = process.argv[2];
var bytes = fs.readFileSync(wasmPath);

var mod = new WebAssembly.Module(bytes);
var instance = new WebAssembly.Instance(mod, {
  env: {
    print: function(value) { console.log(value); }
  }
});

console.log(instance.exports.bench(4000, 100));
