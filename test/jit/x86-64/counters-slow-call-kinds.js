/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xjit=force -Xjit-emit-counters %s 2>&1 | %FileCheck %s
// REQUIRES: jit, jit-arch-x86-64

// Exercises the per-CellKind slow-call histogram: calls from JIT code
// that cannot take the JIT-to-JIT fast path are counted by callee
// CellKind. print() is a NativeFunction, so three calls of it from
// jitted code must produce a NativeFunction row of at least 3.
// A jitted-to-jitted call (f -> g) must NOT appear in the histogram.

function g(x) {
  return x + 1;
}

function f() {
  print(g(1));
  print(g(2));
  print(g(3));
}
f();

// CHECK: 2
// CHECK-NEXT: 3
// CHECK-NEXT: 4
// CHECK: JIT counters:
// CHECK: NumCallSlow[NativeFunction]: {{[3-9]}}
