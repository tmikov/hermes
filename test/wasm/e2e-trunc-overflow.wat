;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test that i32.trunc_f64_s traps on out-of-range.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && ( ! %hermes -Xhermes-internal-test-methods %S/instantiate-hbc.js_ -- %t.hbc 2>&1 ) | %FileCheck %s

(module
  (func $start
    f64.const 3e10
    i32.trunc_f64_s
    drop)

;; CHECK: integer overflow
  (start $start)
)
