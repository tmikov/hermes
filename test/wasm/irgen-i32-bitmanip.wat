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

;; CHECK-LABEL: function wasm_func_0(p0: number): number 
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:number) $local_0: any
;; CHECK:   %[[P0:.*]] = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:           StoreStackInst %[[P0]]: number, %[[L0]]: number
;; CHECK:   %[[A:.*]] = LoadStackInst (:number) %[[L0]]: number
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:number) [HermesBuiltin.wasmI32Clz]{{.*}}, %[[A]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; i32.ctz
  (func $ctz (param i32) (result i32)
    local.get 0
    i32.ctz)

;; CHECK-LABEL: function wasm_func_1(p0: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:number) [HermesBuiltin.wasmI32Ctz]{{.*}}, %[[A]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; i32.popcnt
  (func $popcnt (param i32) (result i32)
    local.get 0
    i32.popcnt)

;; CHECK-LABEL: function wasm_func_2(p0: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:number) [HermesBuiltin.wasmI32Popcnt]{{.*}}, %[[A]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; i32.rotl
  (func $rotl (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.rotl)

;; CHECK-LABEL: function wasm_func_3(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:number) [HermesBuiltin.wasmI32Rotl]{{.*}}, %[[A]]: number, %[[B]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; i32.rotr
  (func $rotr (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.rotr))

;; CHECK-LABEL: function wasm_func_4(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:number) [HermesBuiltin.wasmI32Rotr]{{.*}}, %[[A]]: number, %[[B]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
