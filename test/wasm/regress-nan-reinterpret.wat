;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Regression test: NaN bit patterns must be preserved through reinterpret.
;; f64.const nan = 0x7FF8000000000000 (canonical quiet NaN)
;; f64.const -nan = 0xFFF8000000000000 (negative quiet NaN)
;; i64.reinterpret_f64 must return the exact bit pattern as BigInt.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/regress-nan-reinterpret-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; i64.reinterpret_f64(nan) should be 0x7FF8000000000000 = 9218868437227405312
  (func (export "reinterpret_nan") (result i64)
    f64.const nan
    i64.reinterpret_f64)

  ;; i64.reinterpret_f64(-nan) should be 0xFFF8000000000000 = -2251799813685248
  (func (export "reinterpret_neg_nan") (result i64)
    f64.const -nan
    i64.reinterpret_f64)
)

;; CHECK: 7ff8000000000000
;; CHECK: fff8000000000000
