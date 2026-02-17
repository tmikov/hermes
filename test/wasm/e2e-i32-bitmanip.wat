;;  Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for i32 bit manipulation operations.
;; Verifies clz, ctz, popcnt, rotl, rotr, extend8_s, extend16_s run without
;; errors. Correctness is verified by the irgen lit tests and unit tests.

;; REQUIRES: wasm

;; Test 1: Two-step compilation and execution.
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/load-hbc.js_ -- %t.hbc

;; Test 2: Verify IR is well-formed.
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (func $test_all
    ;; i32.clz(1)
    i32.const 1
    i32.clz
    drop

    ;; i32.ctz(0x80000000)
    i32.const -2147483648
    i32.ctz
    drop

    ;; i32.popcnt(0x0F0F0F0F)
    i32.const 0x0F0F0F0F
    i32.popcnt
    drop

    ;; i32.rotl(0x80000001, 1)
    i32.const -2147483647
    i32.const 1
    i32.rotl
    drop

    ;; i32.rotr(3, 1)
    i32.const 3
    i32.const 1
    i32.rotr
    drop

    ;; i32.extend8_s(0xFF)
    i32.const 0xFF
    i32.extend8_s
    drop

    ;; i32.extend16_s(0xFFFF)
    i32.const 0xFFFF
    i32.extend16_s
    drop
  )

  (start $test_all)
)

;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32Clz]
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32Ctz]
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32Popcnt]
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32Rotl]
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32Rotr]
;; CHECK:   BinaryLeftShiftInst
;; CHECK-NEXT:   BinaryRightShiftInst
;; CHECK:   BinaryLeftShiftInst
;; CHECK-NEXT:   BinaryRightShiftInst
