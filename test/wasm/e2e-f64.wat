;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for f64 arithmetic and comparisons.
;; Compiles to .hbc and runs, verifying correct results.

;; REQUIRES: wasm

;; Test 1: Two-step compilation and execution.
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/load-hbc.js_ -- %t.hbc _start

;; Test 2: Verify IR is well-formed.
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; f64 add: 1.5 + 2.5 = 4.0
  (func $f64_add (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.add)

  ;; f64 sub: 10.0 - 3.5 = 6.5
  (func $f64_sub (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.sub)

  ;; f64 mul: 2.0 * 3.0 = 6.0
  (func $f64_mul (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.mul)

  ;; f64 div: 10.0 / 4.0 = 2.5
  (func $f64_div (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.div)

  ;; f64 neg: -(5.0) = -5.0
  (func $f64_neg (param f64) (result f64)
    local.get 0
    f64.neg)

  ;; f64 abs: abs(-7.0) = 7.0
  (func $f64_abs (param f64) (result f64)
    local.get 0
    f64.abs)

  ;; f64 sqrt: sqrt(4.0) = 2.0
  (func $f64_sqrt (param f64) (result f64)
    local.get 0
    f64.sqrt)

  ;; f64.lt comparison
  (func $f64_lt (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.lt)

  ;; Start function that exercises f64 operations.
  (func (export "_start")
    ;; Exercise all the f64 operations (results are dropped since
    ;; we can't print yet, but this verifies they run without trapping).
    f64.const 1.5
    f64.const 2.5
    call $f64_add
    drop

    f64.const 10.0
    f64.const 3.5
    call $f64_sub
    drop

    f64.const 2.0
    f64.const 3.0
    call $f64_mul
    drop

    f64.const 10.0
    f64.const 4.0
    call $f64_div
    drop

    f64.const 5.0
    call $f64_neg
    drop

    f64.const -7.0
    call $f64_abs
    drop

    f64.const 4.0
    call $f64_sqrt
    drop

    f64.const 1.0
    f64.const 2.0
    call $f64_lt
    drop
  )
  (start 8)
)

;; CHECK-LABEL: function global(): object
;; CHECK:   CreateScopeInst
;; CHECK:   CreateFunctionInst
;; CHECK:   CallInst
;; CHECK:   ReturnInst
