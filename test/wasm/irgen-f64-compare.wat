;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for f64 comparison operations.
;; Verifies that each comparison produces a compare + AsInt32Inst (bool->i32).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: f64.eq -> FEqualInst + AsInt32Inst (boolean to i32)
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.eq)

;; CHECK-LABEL: function wasm_func_0(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[CMP:.*]] = FEqualInst (:boolean) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 1: f64.ne
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.ne)

;; CHECK-LABEL: function wasm_func_1(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[CMP:.*]] = FNotEqualInst (:boolean) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 2: f64.lt
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.lt)

;; CHECK-LABEL: function wasm_func_2(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[CMP:.*]] = FLessThanInst (:boolean) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 3: f64.gt
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.gt)

;; CHECK-LABEL: function wasm_func_3(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[CMP:.*]] = FGreaterThanInst (:boolean) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 4: f64.le
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.le)

;; CHECK-LABEL: function wasm_func_4(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[CMP:.*]] = FLessThanOrEqualInst (:boolean) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 5: f64.ge
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.ge))

;; CHECK-LABEL: function wasm_func_5(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[CMP:.*]] = FGreaterThanOrEqualInst (:boolean) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
