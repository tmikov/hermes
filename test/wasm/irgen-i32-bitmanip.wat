;;  Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test i32 bit manipulation IR generation: clz, ctz, popcnt, rotl, rotr.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; i32.clz
  (func $clz (param i32) (result i32)
    local.get 0
    i32.clz
  )

  ;; i32.ctz
  (func $ctz (param i32) (result i32)
    local.get 0
    i32.ctz
  )

  ;; i32.popcnt
  (func $popcnt (param i32) (result i32)
    local.get 0
    i32.popcnt
  )

  ;; i32.rotl
  (func $rotl (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.rotl
  )

  ;; i32.rotr
  (func $rotr (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.rotr
  )
)

;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32Clz]
;; CHECK:   ReturnInst

;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32Ctz]
;; CHECK:   ReturnInst

;; CHECK-LABEL: function wasm_func_2(p0: any): any
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32Popcnt]
;; CHECK:   ReturnInst

;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32Rotl]
;; CHECK:   ReturnInst

;; CHECK-LABEL: function wasm_func_4(p0: any, p1: any): any
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32Rotr]
;; CHECK:   ReturnInst
