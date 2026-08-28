;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for f64 comparison operations.
;; Verifies that each comparison produces a compare + BinaryOrInst (bool->i32).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: f64.eq -> BinaryStrictlyEqualInst + BinaryOrInst (boolean to i32)
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.eq)

;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CMP:.*]] = BinaryStrictlyEqualInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 1: f64.ne
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.ne)

;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CMP:.*]] = BinaryStrictlyNotEqualInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 2: f64.lt
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.lt)

;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CMP:.*]] = BinaryLessThanInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 3: f64.gt
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.gt)

;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CMP:.*]] = BinaryGreaterThanInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 4: f64.le
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.le)

;; CHECK-LABEL: function wasm_func_4(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CMP:.*]] = BinaryLessThanOrEqualInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 5: f64.ge
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.ge))

;; CHECK-LABEL: function wasm_func_5(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CMP:.*]] = BinaryGreaterThanOrEqualInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end
