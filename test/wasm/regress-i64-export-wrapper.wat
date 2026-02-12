;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Regression test: i64 export wrappers use BigInt at the JS boundary.
;; Previously the export wrapper set hi32=0 for all i64 params, so negative
;; values like -5 were treated as unsigned 4294967291 instead of -5.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/regress-i64-export-wrapper-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; div_s(-5, 2) should return -2n (BigInt).
  ;; With hi32=0 bug: -5 becomes i64(4294967291), div by 2 = 2147483645.
  (func (export "test_neg_div") (param i64 i64) (result i64)
    local.get 0
    local.get 1
    i64.div_s)
)

;; CHECK: -2
