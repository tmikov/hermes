;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for i32 trapping division and remainder (F.2).
;; Tests normal operation (no trap) and that the module compiles and runs.

;; REQUIRES: wasm

;; Test 1: Two-step compilation and execution.
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/load-hbc.js_ -- %t.hbc

;; Test 2: Verify IR uses CallBuiltinInst for div/rem operations.
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (func $div_s (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.div_s)

  (func $div_u (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.div_u)

  (func $rem_s (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.rem_s)

  (func $rem_u (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.rem_u)

  ;; Start function: exercise normal division/remainder operations.
  (func $start
    ;; i32.div_s(10, 3) = 3
    (call $div_s (i32.const 10) (i32.const 3))
    drop

    ;; i32.div_s(-10, 3) = -3
    (call $div_s (i32.const -10) (i32.const 3))
    drop

    ;; i32.div_u(0xFFFFFFFF, 2) = 2147483647
    (call $div_u (i32.const -1) (i32.const 2))
    drop

    ;; i32.rem_s(10, 3) = 1
    (call $rem_s (i32.const 10) (i32.const 3))
    drop

    ;; i32.rem_s(-10, 3) = -1
    (call $rem_s (i32.const -10) (i32.const 3))
    drop

    ;; i32.rem_s(INT32_MIN, -1) = 0 (not a trap!)
    (call $rem_s (i32.const -2147483648) (i32.const -1))
    drop

    ;; i32.rem_u(0xFFFFFFFF, 10) = 5
    (call $rem_u (i32.const -1) (i32.const 10))
    drop
  )
  (start $start)
)

;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32DivS]
;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32DivU]
;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32RemS]
;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32RemU]
