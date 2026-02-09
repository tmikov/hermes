;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test i32 bit manipulation IR generation: clz, ctz, popcnt, rotl, rotr.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; i32.clz — first function checked exhaustively including param loading.
  (func $clz (param i32) (result i32)
    local.get 0
    i32.clz)

;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:any) $local_0: any
;; CHECK:   %[[P0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:           StoreStackInst %[[P0]]: any, %[[L0]]: any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any) %[[L0]]: any
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmI32Clz]{{.*}}, %[[A]]: any
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; i32.ctz
  (func $ctz (param i32) (result i32)
    local.get 0
    i32.ctz)

;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmI32Ctz]{{.*}}, %[[A]]: any
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; i32.popcnt
  (func $popcnt (param i32) (result i32)
    local.get 0
    i32.popcnt)

;; CHECK-LABEL: function wasm_func_2(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmI32Popcnt]{{.*}}, %[[A]]: any
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; i32.rotl
  (func $rotl (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.rotl)

;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmI32Rotl]{{.*}}, %[[A]]: any, %[[B]]: any
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; i32.rotr
  (func $rotr (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.rotr))

;; CHECK-LABEL: function wasm_func_4(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmI32Rotr]{{.*}}, %[[A]]: any, %[[B]]: any
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end
