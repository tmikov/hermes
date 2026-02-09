;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test i32 trapping division and remainder operations (F.2).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; i32.div_s: signed division
  (func $div_s (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.div_s
  )

  ;; i32.div_u: unsigned division
  (func $div_u (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.div_u
  )

  ;; i32.rem_s: signed remainder
  (func $rem_s (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.rem_s
  )

  ;; i32.rem_u: unsigned remainder
  (func $rem_u (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.rem_u
  )
)

;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32DivS]

;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32DivU]

;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32RemS]

;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32RemU]
