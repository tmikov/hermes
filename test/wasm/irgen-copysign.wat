;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s
;; REQUIRES: wasm

(module
  ;; f64.copysign
  (func $f64_copysign (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.copysign
  )
;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK:   {{.*}} = CallBuiltinInst (:any) [HermesBuiltin.wasmF64Copysign]

  ;; f32.copysign
  (func $f32_copysign (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.copysign
  )
;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK:   {{.*}} = CallBuiltinInst (:any) [HermesBuiltin.wasmF32Copysign]

  ;; f64.copysign with constants
  (func $f64_copysign_const (result f64)
    f64.const 1.0
    f64.const -1.0
    f64.copysign
  )
;; CHECK-LABEL: function wasm_func_2(): any
;; CHECK:   {{.*}} = CallBuiltinInst (:any) [HermesBuiltin.wasmF64Copysign]

  ;; f32.copysign with constants
  (func $f32_copysign_const (result f32)
    f32.const 5.0
    f32.const -3.0
    f32.copysign
  )
;; CHECK-LABEL: function wasm_func_3(): any
;; CHECK:   {{.*}} = CallBuiltinInst (:any) [HermesBuiltin.wasmF32Copysign]
)
