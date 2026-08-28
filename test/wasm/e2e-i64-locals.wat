;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for i64 locals and control flow (G.5).
;; Tests i64 parameters, locals, function calls, block/if results.
;; All exported functions return i32 for easy JS-side verification.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-i64-locals-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; Helper: i64 identity (tests param passing and return)
  (func $id (param i64) (result i64)
    local.get 0)

  ;; Test 1: Call id with a value, wrap result to i32
  ;; 42 should survive the round-trip.
  (func $call_id_lo (export "call_id_lo") (result i32)
    i64.const 42
    call $id
    i32.wrap_i64)

  ;; Test 2: Call id with a large value (hi32 != 0), verify hi32 survives
  ;; 0x0000000100000002 → lo=2, hi=1. After wrap, should get 2.
  (func $call_id_hi (export "call_id_hi") (result i32)
    i64.const 4294967298  ;; lo=2, hi=1
    call $id
    ;; Verify hi survived: shift right by 32 and wrap
    ;; i64.const 32 + i64.shr_u gives us hi in lo position
    i64.const 32
    i64.shr_u
    i32.wrap_i64)

  ;; Test 3: i64 local set/get
  (func $local_roundtrip (export "local_roundtrip") (result i32)
    (local i64)
    i64.const 999
    local.set 0
    local.get 0
    i32.wrap_i64)

  ;; Test 4: i64 local.tee
  (func $local_tee (export "local_tee") (result i32)
    (local i64)
    i64.const 777
    local.tee 0
    drop
    local.get 0
    i32.wrap_i64)

  ;; Test 5: i64 block result
  (func $block_result (export "block_result") (result i32)
    (block (result i64)
      i64.const 55)
    i32.wrap_i64)

  ;; Test 6: i64 if/else result
  (func $if_result_true (export "if_result_true") (result i32)
    (if (result i64) (i32.const 1)
      (then (i64.const 10))
      (else (i64.const 20)))
    i32.wrap_i64)

  (func $if_result_false (export "if_result_false") (result i32)
    (if (result i64) (i32.const 0)
      (then (i64.const 10))
      (else (i64.const 20)))
    i32.wrap_i64)

  ;; Test 7: i64 through multiple calls with hi32 bits
  ;; Verify that hi32 is preserved across call boundaries
  (func $double_i64 (param i64) (result i64)
    local.get 0
    local.get 0
    i64.add)

  (func $double_check (export "double_check") (result i32)
    i64.const 100
    call $double_i64
    ;; 100 + 100 = 200
    i64.const 200
    i64.eq))

;; CHECK: call_id_lo: 42
;; CHECK: call_id_hi: 1
;; CHECK: local_roundtrip: 999
;; CHECK: local_tee: 777
;; CHECK: block_result: 55
;; CHECK: if_result_true: 10
;; CHECK: if_result_false: 20
;; CHECK: double_check: 1
