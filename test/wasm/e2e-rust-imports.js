/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// End-to-end test against real toolchain output: a no_std Rust module built
// for wasm32-unknown-unknown that imports its memory and two host functions.
// See fixtures/README.md for how the .wasm was produced.

// REQUIRES: wasm

// RUN: %hermesc --wasm -emit-binary -out %t.hbc %S/fixtures/rust-imports.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-rust-imports-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

// CHECK: run(41) = 42
// CHECK-NEXT: greet logged: hello from rust
