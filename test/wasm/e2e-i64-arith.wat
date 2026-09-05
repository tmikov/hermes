;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for i64 arithmetic operations (G.3).
;; Tests i64 comparisons (which return i32) and a start function that
;; exercises i64 add/sub/mul without crashing.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-i64-arith-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; i64.eqz: returns 1 if the i64 value is zero.
  (func $eqz_zero (export "eqz_zero") (result i32)
    i64.const 0
    i64.eqz)

  (func $eqz_one (export "eqz_one") (result i32)
    i64.const 1
    i64.eqz)

  (func $eqz_hi (export "eqz_hi") (result i32)
    i64.const 4294967296  ;; 0x100000000: lo=0, hi=1
    i64.eqz)

  ;; i64.eq: returns 1 if two i64 values are equal.
  (func $eq_same (export "eq_same") (result i32)
    i64.const 42
    i64.const 42
    i64.eq)

  (func $eq_diff (export "eq_diff") (result i32)
    i64.const 42
    i64.const 43
    i64.eq)

  ;; i64.lt_s: signed less than.
  (func $lt_s_true (export "lt_s_true") (result i32)
    i64.const -1
    i64.const 1
    i64.lt_s)

  (func $lt_s_false (export "lt_s_false") (result i32)
    i64.const 1
    i64.const -1
    i64.lt_s)

  ;; i64.lt_u: unsigned less than (unsigned -1 = max value).
  (func $lt_u_true (export "lt_u_true") (result i32)
    i64.const 1
    i64.const -1
    i64.lt_u)

  (func $lt_u_false (export "lt_u_false") (result i32)
    i64.const -1
    i64.const 1
    i64.lt_u)

  ;; Test: i64.eqz after i64.add(100, 200) - should be 0 (not zero).
  (func $add_not_zero (export "add_not_zero") (result i32)
    i64.const 100
    i64.const 200
    i64.add
    i64.eqz)

  ;; Test: i64.eq after i64.add(100, 200) == 300.
  (func $add_eq_300 (export "add_eq_300") (result i32)
    i64.const 100
    i64.const 200
    i64.add
    i64.const 300
    i64.eq)

  ;; Test: i64.sub(500, 200) == 300.
  (func $sub_eq (export "sub_eq") (result i32)
    i64.const 500
    i64.const 200
    i64.sub
    i64.const 300
    i64.eq)

  ;; Test: i64.mul(6, 7) == 42.
  (func $mul_eq (export "mul_eq") (result i32)
    i64.const 6
    i64.const 7
    i64.mul
    i64.const 42
    i64.eq)

  ;; Test: i64.and(0xFF00, 0x0FFF) == 0x0F00.
  (func $and_eq (export "and_eq") (result i32)
    i64.const 0xFF00
    i64.const 0x0FFF
    i64.and
    i64.const 0x0F00
    i64.eq)

  ;; Test: i64.or(0xFF00, 0x00FF) == 0xFFFF.
  (func $or_eq (export "or_eq") (result i32)
    i64.const 0xFF00
    i64.const 0x00FF
    i64.or
    i64.const 0xFFFF
    i64.eq)

  ;; Test: i64.xor(0xFF, 0x0F) == 0xF0.
  (func $xor_eq (export "xor_eq") (result i32)
    i64.const 0xFF
    i64.const 0x0F
    i64.xor
    i64.const 0xF0
    i64.eq)

  ;; Test: i64.shl(1, 32) == 0x100000000.
  (func $shl_eq (export "shl_eq") (result i32)
    i64.const 1
    i64.const 32
    i64.shl
    i64.const 4294967296
    i64.eq)

  ;; Test: i64.clz on a small number: clz(1) == 63.
  (func $clz_one (export "clz_one") (result i32)
    i64.const 1
    i64.clz
    i64.const 63
    i64.eq)

  ;; Test: i64.ctz on a power of 2: ctz(0x100000000) == 32.
  (func $ctz_hi (export "ctz_hi") (result i32)
    i64.const 4294967296
    i64.ctz
    i64.const 32
    i64.eq)

  ;; Test: i64.popcnt(0xFF) == 8.
  (func $popcnt_byte (export "popcnt_byte") (result i32)
    i64.const 0xFF
    i64.popcnt
    i64.const 8
    i64.eq)

  ;; Start function: exercise i64 arithmetic to verify no crash.
  (func $start_test
    i64.const 100
    i64.const 200
    i64.add
    drop)

  (start $start_test)
)

;; CHECK: eqz_zero: 1
;; CHECK-NEXT: eqz_one: 0
;; CHECK-NEXT: eqz_hi: 0
;; CHECK-NEXT: eq_same: 1
;; CHECK-NEXT: eq_diff: 0
;; CHECK-NEXT: lt_s_true: 1
;; CHECK-NEXT: lt_s_false: 0
;; CHECK-NEXT: lt_u_true: 1
;; CHECK-NEXT: lt_u_false: 0
;; CHECK-NEXT: add_not_zero: 0
;; CHECK-NEXT: add_eq_300: 1
;; CHECK-NEXT: sub_eq: 1
;; CHECK-NEXT: mul_eq: 1
;; CHECK-NEXT: and_eq: 1
;; CHECK-NEXT: or_eq: 1
;; CHECK-NEXT: xor_eq: 1
;; CHECK-NEXT: shl_eq: 1
;; CHECK-NEXT: clz_one: 1
;; CHECK-NEXT: ctz_hi: 1
;; CHECK-NEXT: popcnt_byte: 1
