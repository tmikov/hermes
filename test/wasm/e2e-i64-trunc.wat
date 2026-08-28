;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for i64 truncation operations (G.4b).
;; Uses i64.trunc_* then i32.wrap_i64 to extract lo32 for i32 verification,
;; and i64 comparisons (i64.eq) to test full 64-bit results.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-i64-trunc-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; i64.trunc_f64_s(42.7) -> 42, wrap -> 42
  (func $trunc_s_lo (export "trunc_s_lo") (result i32)
    f64.const 42.7
    i64.trunc_f64_s
    i32.wrap_i64)

  ;; i64.trunc_f64_s(-100.9) -> -100, wrap -> lo32 of -100
  ;; -100 as int64: 0xFFFFFFFFFFFFFF9C, lo32 = 0xFFFFFF9C = -100 as int32
  (func $trunc_s_neg (export "trunc_s_neg") (result i32)
    f64.const -100.9
    i64.trunc_f64_s
    i32.wrap_i64)

  ;; i64.trunc_f64_u(4294967296.5) -> 4294967296 (0x100000000)
  ;; lo=0, hi=1. Wrap -> 0
  (func $trunc_u_hi (export "trunc_u_hi") (result i32)
    f64.const 4294967296.5
    i64.trunc_f64_u
    i32.wrap_i64)

  ;; Verify the full i64 result of trunc_f64_u(4294967296.5)
  ;; == 4294967296 using i64.eq
  (func $trunc_u_full (export "trunc_u_full") (result i32)
    f64.const 4294967296.5
    i64.trunc_f64_u
    i64.const 4294967296
    i64.eq)

  ;; i64.trunc_f64_s(0.0) -> 0
  (func $trunc_s_zero (export "trunc_s_zero") (result i32)
    f64.const 0.0
    i64.trunc_f64_s
    i64.const 0
    i64.eq)

  ;; i64.trunc_sat_f64_s(NaN) -> 0
  (func $sat_s_nan (export "sat_s_nan") (result i32)
    f64.const nan
    i64.trunc_sat_f64_s
    i64.const 0
    i64.eq)

  ;; i64.trunc_sat_f64_u(-1.0) -> 0 (clamped)
  (func $sat_u_neg (export "sat_u_neg") (result i32)
    f64.const -1.0
    i64.trunc_sat_f64_u
    i64.const 0
    i64.eq)

  ;; i64.trunc_sat_f64_s(1e20) -> INT64_MAX (0x7FFFFFFFFFFFFFFF)
  ;; Verify with i64.eq against INT64_MAX
  (func $sat_s_overflow (export "sat_s_overflow") (result i32)
    f64.const 1e20
    i64.trunc_sat_f64_s
    i64.const 9223372036854775807
    i64.eq)

  ;; i64.trunc_sat_f64_u: large but in range value
  ;; trunc_sat_u(1e10) = 10000000000. Verify via i64.eq.
  (func $sat_u_large (export "sat_u_large") (result i32)
    f64.const 1e10
    i64.trunc_sat_f64_u
    i64.const 10000000000
    i64.eq)

  ;; i64.trunc_f32_s: same result as f64 variant in Phase 1
  (func $trunc_f32_s (export "trunc_f32_s") (result i32)
    f32.const 99.5
    i64.trunc_f32_s
    i64.const 99
    i64.eq)

  ;; i64.trunc_sat_f32_u(nan) -> 0
  (func $sat_f32_nan (export "sat_f32_nan") (result i32)
    f32.const nan
    i64.trunc_sat_f32_u
    i64.const 0
    i64.eq)

  ;; Start: exercise the trapping truncation (no crash).
  (func $start_test
    f64.const 42.0
    i64.trunc_f64_s
    drop)

  (start $start_test)
)

;; CHECK: trunc_s_lo: 42
;; CHECK-NEXT: trunc_s_neg: -100
;; CHECK-NEXT: trunc_u_hi: 0
;; CHECK-NEXT: trunc_u_full: 1
;; CHECK-NEXT: trunc_s_zero: 1
;; CHECK-NEXT: sat_s_nan: 1
;; CHECK-NEXT: sat_u_neg: 1
;; CHECK-NEXT: sat_s_overflow: 1
;; CHECK-NEXT: sat_u_large: 1
;; CHECK-NEXT: trunc_f32_s: 1
;; CHECK-NEXT: sat_f32_nan: 1
