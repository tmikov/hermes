;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for f32 arithmetic and comparisons (E.2, E.3).
;; Verifies that f32 operations compile and execute without errors.

;; REQUIRES: wasm

;; Test 1: Two-step compilation and execution via WebAssembly API.
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/load-hbc.js_ -- %t.hbc

;; Test 2: Verify IR structure.
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Simple f32 addition
  (func $f32_add (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.add
  )

  ;; f32 less-than comparison
  (func $f32_lt (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.lt
  )

  ;; f32 negation
  (func $f32_neg (param f32) (result f32)
    local.get 0
    f32.neg
  )

  ;; f32.abs
  (func $f32_abs (param f32) (result f32)
    local.get 0
    f32.abs
  )

  ;; f32.min
  (func $f32_min (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.min
  )

  ;; Start function (void): exercises f32 ops.
  ;; Computes abs(neg(min(1.5 + 2.5, 10.0))), then checks lt(result, 5.0).
  (func $start
    f32.const 1.5
    f32.const 2.5
    call $f32_add
    f32.const 10.0
    call $f32_min
    call $f32_neg
    call $f32_abs
    f32.const 5.0
    call $f32_lt
    drop
  )

  (start $start)
)

;; CHECK-LABEL: function wasm_func_0(p0: number, p1: number): number 
;; CHECK:   FAddInst

;; CHECK-LABEL: function wasm_func_1(p0: number, p1: number): number 
;; CHECK:   FLessThanInst
;; CHECK-NEXT:   AsInt32Inst

;; CHECK-LABEL: function wasm_func_2(p0: number): number 
;; CHECK:   FNegate

;; CHECK-LABEL: function wasm_func_3(p0: number): number 
;; CHECK:   CallBuiltinInst {{.*}}[Math.abs]

;; CHECK-LABEL: function wasm_func_4(p0: number, p1: number): number 
;; CHECK:   CallBuiltinInst {{.*}}[Math.min]
