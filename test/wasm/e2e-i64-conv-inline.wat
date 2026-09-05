;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for i64 inline conversions (G.4a).
;; Tests i32.wrap_i64, i64.extend_i32_s/u, i64.extend8/16/32_s.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-i64-conv-inline-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; --- i32.wrap_i64 ---

  ;; wrap small positive: i64(42) -> i32(42)
  (func $wrap_small (export "wrap_small") (result i32)
    i64.const 42
    i32.wrap_i64)
;; CHECK: wrap_small: 42

  ;; wrap large: i64(0x1FFFFFFFF) -> lo = 0xFFFFFFFF = -1 as signed i32
  (func $wrap_large (export "wrap_large") (result i32)
    i64.const 0x1FFFFFFFF
    i32.wrap_i64)
;; CHECK-NEXT: wrap_large: -1

  ;; wrap zero: i64(0) -> i32(0)
  (func $wrap_zero (export "wrap_zero") (result i32)
    i64.const 0
    i32.wrap_i64)
;; CHECK-NEXT: wrap_zero: 0

  ;; wrap hi-only: i64(0x100000000) -> lo=0 -> i32(0)
  (func $wrap_hi_only (export "wrap_hi_only") (result i32)
    i64.const 0x100000000
    i32.wrap_i64)
;; CHECK-NEXT: wrap_hi_only: 0

  ;; --- i64.extend_i32_s ---

  ;; extend_s positive: i32(42) -> i64(42), eqz = 0
  (func $ext_s_pos_eqz (export "ext_s_pos_eqz") (result i32)
    i32.const 42
    i64.extend_i32_s
    i64.eqz)
;; CHECK-NEXT: ext_s_pos_eqz: 0

  ;; extend_s zero: i32(0) -> i64(0), eqz = 1
  (func $ext_s_zero_eqz (export "ext_s_zero_eqz") (result i32)
    i32.const 0
    i64.extend_i32_s
    i64.eqz)
;; CHECK-NEXT: ext_s_zero_eqz: 1

  ;; extend_s negative: i32(-1) -> i64(-1) (lo=0xFFFFFFFF, hi=0xFFFFFFFF)
  ;; Compare with i64(-1): should be equal (1)
  (func $ext_s_neg_eq (export "ext_s_neg_eq") (result i32)
    i32.const -1
    i64.extend_i32_s
    i64.const -1
    i64.eq)
;; CHECK-NEXT: ext_s_neg_eq: 1

  ;; extend_s INT32_MIN: i32(0x80000000) -> i64(0xFFFFFFFF80000000)
  ;; lo=0x80000000, hi=0xFFFFFFFF. Check via lt_s with 0.
  (func $ext_s_min_lt (export "ext_s_min_lt") (result i32)
    i32.const -2147483648
    i64.extend_i32_s
    i64.const 0
    i64.lt_s)
;; CHECK-NEXT: ext_s_min_lt: 1

  ;; --- i64.extend_i32_u ---

  ;; extend_u positive: i32(42) -> i64(42)
  (func $ext_u_pos_eqz (export "ext_u_pos_eqz") (result i32)
    i32.const 42
    i64.extend_i32_u
    i64.eqz)
;; CHECK-NEXT: ext_u_pos_eqz: 0

  ;; extend_u -1: i32(-1) = 0xFFFFFFFF -> i64(0x00000000FFFFFFFF)
  ;; This is positive as i64 (hi=0). Check it's > 0.
  (func $ext_u_neg_gt (export "ext_u_neg_gt") (result i32)
    i32.const -1
    i64.extend_i32_u
    i64.const 0
    i64.gt_s)
;; CHECK-NEXT: ext_u_neg_gt: 1

  ;; extend_u -1: should equal 4294967295
  (func $ext_u_neg_eq (export "ext_u_neg_eq") (result i32)
    i32.const -1
    i64.extend_i32_u
    i64.const 4294967295
    i64.eq)
;; CHECK-NEXT: ext_u_neg_eq: 1

  ;; --- i64.extend8_s ---

  ;; 0x7F (127) -> sign bit 0 -> stays 127 (lo=127, hi=0)
  (func $ext8s_pos (export "ext8s_pos") (result i32)
    i64.const 0x7F
    i64.extend8_s
    i64.const 127
    i64.eq)
;; CHECK-NEXT: ext8s_pos: 1

  ;; 0x80 (128) -> sign bit 1 -> -128 (lo=0xFFFFFF80, hi=0xFFFFFFFF)
  (func $ext8s_neg (export "ext8s_neg") (result i32)
    i64.const 0x80
    i64.extend8_s
    i64.const -128
    i64.eq)
;; CHECK-NEXT: ext8s_neg: 1

  ;; 0xFF (255) -> sign bit 1 -> -1 (lo=0xFFFFFFFF, hi=0xFFFFFFFF)
  (func $ext8s_ff (export "ext8s_ff") (result i32)
    i64.const 0xFF
    i64.extend8_s
    i64.const -1
    i64.eq)
;; CHECK-NEXT: ext8s_ff: 1

  ;; 0x100 -> only lowest 8 bits matter -> 0x00 -> 0
  (func $ext8s_256 (export "ext8s_256") (result i32)
    i64.const 0x100
    i64.extend8_s
    i64.eqz)
;; CHECK-NEXT: ext8s_256: 1

  ;; --- i64.extend16_s ---

  ;; 0x7FFF (32767) -> positive, stays 32767
  (func $ext16s_pos (export "ext16s_pos") (result i32)
    i64.const 0x7FFF
    i64.extend16_s
    i64.const 32767
    i64.eq)
;; CHECK-NEXT: ext16s_pos: 1

  ;; 0x8000 (32768) -> sign bit 1 -> -32768
  (func $ext16s_neg (export "ext16s_neg") (result i32)
    i64.const 0x8000
    i64.extend16_s
    i64.const -32768
    i64.eq)
;; CHECK-NEXT: ext16s_neg: 1

  ;; 0xFFFF (65535) -> -1
  (func $ext16s_ffff (export "ext16s_ffff") (result i32)
    i64.const 0xFFFF
    i64.extend16_s
    i64.const -1
    i64.eq)
;; CHECK-NEXT: ext16s_ffff: 1

  ;; --- i64.extend32_s ---

  ;; 0x7FFFFFFF -> positive, stays same
  (func $ext32s_pos (export "ext32s_pos") (result i32)
    i64.const 0x7FFFFFFF
    i64.extend32_s
    i64.const 0x7FFFFFFF
    i64.eq)
;; CHECK-NEXT: ext32s_pos: 1

  ;; 0x80000000 -> sign bit 1 -> -2147483648 (lo=0x80000000, hi=0xFFFFFFFF)
  (func $ext32s_neg (export "ext32s_neg") (result i32)
    i64.const 0x80000000
    i64.extend32_s
    i64.const -2147483648
    i64.eq)
;; CHECK-NEXT: ext32s_neg: 1

  ;; 0xFFFFFFFF -> -1
  (func $ext32s_max (export "ext32s_max") (result i32)
    i64.const 0xFFFFFFFF
    i64.extend32_s
    i64.const -1
    i64.eq)
;; CHECK-NEXT: ext32s_max: 1

  ;; 0x100000000 (hi=1, lo=0) -> extend32_s sees lo=0 -> sign bit 0 -> i64(0)
  (func $ext32s_hi (export "ext32s_hi") (result i32)
    i64.const 0x100000000
    i64.extend32_s
    i64.eqz)
;; CHECK-NEXT: ext32s_hi: 1
)
