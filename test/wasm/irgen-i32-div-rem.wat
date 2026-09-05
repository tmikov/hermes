;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test i32 trapping division and remainder operations (F.2).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; i32.div_s: signed division
  (func $div_s (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.div_s)

;; CHECK-LABEL: function wasm_func_0(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:number) [HermesBuiltin.wasmI32DivS]{{.*}}, %[[A]]: number, %[[B]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; i32.div_u: unsigned division
  (func $div_u (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.div_u)

;; CHECK-LABEL: function wasm_func_1(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:number) [HermesBuiltin.wasmI32DivU]{{.*}}, %[[A]]: number, %[[B]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; i32.rem_s: signed remainder
  (func $rem_s (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.rem_s)

;; CHECK-LABEL: function wasm_func_2(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:number) [HermesBuiltin.wasmI32RemS]{{.*}}, %[[A]]: number, %[[B]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; i32.rem_u: unsigned remainder
  (func $rem_u (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.rem_u))

;; CHECK-LABEL: function wasm_func_3(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:number) [HermesBuiltin.wasmI32RemU]{{.*}}, %[[A]]: number, %[[B]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
