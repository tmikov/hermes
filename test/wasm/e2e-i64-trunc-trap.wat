;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test that i64 trapping truncations produce errors for NaN and out-of-range.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-i64-trunc-trap-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; Trapping: NaN should throw "invalid conversion to integer"
  (func $trunc_nan (export "trunc_nan") (result i64)
    f64.const nan
    i64.trunc_f64_s)

  ;; Trapping: overflow should throw "integer overflow"
  (func $trunc_overflow (export "trunc_overflow") (result i64)
    f64.const 1e20
    i64.trunc_f64_s)

  ;; Trapping unsigned: negative should throw "integer overflow"
  (func $trunc_u_neg (export "trunc_u_neg") (result i64)
    f64.const -1.0
    i64.trunc_f64_u)
)

;; CHECK: trunc_nan: invalid conversion to integer
;; CHECK-NEXT: trunc_overflow: integer overflow
;; CHECK-NEXT: trunc_u_neg: integer overflow
