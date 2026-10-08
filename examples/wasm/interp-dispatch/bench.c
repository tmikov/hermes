/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Compile with (requires Homebrew llvm + lld on macOS):
//   PATH="/opt/homebrew/opt/lld/bin:$PATH" \
//   /opt/homebrew/opt/llvm/bin/clang \
//     --target=wasm32-unknown-unknown -nostdlib -O2 \
//     -Wl,--no-entry -Wl,--export-all \
//     -o bench.wasm bench.c

__attribute__((import_module("env"), import_name("print")))
extern void print(double value);

__attribute__((export_name("bench")))
double bench(int lc, int fc) {
  double res = 0;
  while (--lc >= 0) {
    int n = fc;
    double fact = n;
    while (--n > 1)
      fact *= n;
    res += fact;
  }
  return res;
}

__attribute__((export_name("main")))
void main_entry(void) {
  // Original JS benchmark uses bench(4000000, 100).
  print(bench(4000, 100));
}
