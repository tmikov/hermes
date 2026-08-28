/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// JS driver to run the Wasm bench module. Accepts both .wasm and .hbc files.
//
// Usage:
//   hermes -Xhermes-internal-test-methods run.js -- bench.wasm
//   hermesc --wasm -emit-binary -out bench.hbc bench.wasm
//   hermes -Xhermes-internal-test-methods run.js -- bench.hbc

var path = hermescli.getScriptArgs()[0];
var bytes = hermescli.loadFile(path);

var mod = new WebAssembly.Module(bytes);
var instance = new WebAssembly.Instance(mod, {
  env: {
    print: function(value) { print(value); }
  }
});

print(instance.exports.bench(4000, 100));
