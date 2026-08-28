;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for i64→float conversions and reinterpret (G.4c).
;; Tests correctness of conversion results via exported functions.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-i64-conv-float-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; f64.convert_i64_s: signed i64 → f64
  ;; 42 → 42.0
  (func (export "f64_convert_i64_s_small") (result f64)
    i64.const 42
    f64.convert_i64_s)

  ;; f64.convert_i64_s with negative: -100 → -100.0
  (func (export "f64_convert_i64_s_neg") (result f64)
    i64.const -100
    f64.convert_i64_s)

  ;; f64.convert_i64_u: unsigned i64 → f64
  ;; 42 → 42.0
  (func (export "f64_convert_i64_u_small") (result f64)
    i64.const 42
    f64.convert_i64_u)

  ;; f32.convert_i64_s: signed i64 → f32
  ;; 42 → 42.0
  (func (export "f32_convert_i64_s_small") (result f32)
    i64.const 42
    f32.convert_i64_s)

  ;; f32.convert_i64_u: unsigned i64 → f32
  ;; 42 → 42.0
  (func (export "f32_convert_i64_u_small") (result f32)
    i64.const 42
    f32.convert_i64_u)

  ;; f64.reinterpret_i64: bitcast i64 bits to f64
  ;; 0x3FF0000000000000 = 1.0 as IEEE 754 double
  (func (export "f64_reinterpret_i64_one") (result f64)
    i64.const 4607182418800017408
    f64.reinterpret_i64)

  ;; f64.reinterpret_i64: bitcast 0 → 0.0
  (func (export "f64_reinterpret_i64_zero") (result f64)
    i64.const 0
    f64.reinterpret_i64)

  ;; i64.reinterpret_f64 followed by f64.reinterpret_i64: roundtrip
  ;; 3.14 → bits → 3.14
  (func (export "reinterpret_roundtrip") (result f64)
    f64.const 3.14
    i64.reinterpret_f64
    f64.reinterpret_i64)
)

;; CHECK: f64_convert_i64_s_small: 42
;; CHECK: f64_convert_i64_s_neg: -100
;; CHECK: f64_convert_i64_u_small: 42
;; CHECK: f32_convert_i64_s_small: 42
;; CHECK: f32_convert_i64_u_small: 42
;; CHECK: f64_reinterpret_i64_one: 1
;; CHECK: f64_reinterpret_i64_zero: 0
;; CHECK: reinterpret_roundtrip: 3.14
