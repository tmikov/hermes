/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Node.js driver — loads and runs the Wasm Game of Life module.
//
// Usage:
//   node node-run.js life.wasm

var fs = require("fs");
var wasmPath = process.argv[2];
var bytes = fs.readFileSync(wasmPath);

var mod = new WebAssembly.Module(bytes);
var instance = new WebAssembly.Instance(mod, {
  env: {
    log: function(value) { console.log(value); }
  }
});

var t0 = Date.now();
instance.exports.run(2000);
console.log("elapsed:", Date.now() - t0, "ms");
