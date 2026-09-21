;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for f32 comparison operations (E.3).
;; Same IR pattern as f64/i32 comparisons -- boolean result converted
;; to i32 via AsInt32Inst(cmp).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: f32.eq(a, b) — FEqualInst + AsInt32Inst
  ;; The first function is checked exhaustively including param loading.
  (func $f32_eq (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.eq)
;; CHECK-LABEL: function wasm_func_0(p0: number, p1: number): number 
;; CHECK: %BB0:
;; CHECK:   %[[L0_0:.*]] = AllocStackInst (:number)
;; CHECK:   %[[P0_0:.*]] = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:              StoreStackInst %[[P0_0]]: number, %[[L0_0]]: number
;; CHECK:   %[[L1_0:.*]] = AllocStackInst (:number)
;; CHECK:   %[[P1_0:.*]] = LoadParamInst (:number) %p1: number
;; CHECK-NEXT:              StoreStackInst %[[P1_0]]: number, %[[L1_0]]: number
;; CHECK:   %[[A:.*]] = LoadStackInst (:number) %[[L0_0]]: number
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number) %[[L1_0]]: number
;; CHECK-NEXT: %[[CMP:.*]] = FEqualInst (:boolean) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[OR:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[OR]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 1: f32.ne(a, b) — FNotEqualInst + AsInt32Inst
  (func $f32_ne (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.ne)
;; CHECK-LABEL: function wasm_func_1(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[CMP:.*]] = FNotEqualInst (:boolean) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[OR:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[OR]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 2: f32.lt(a, b) — FLessThanInst + AsInt32Inst
  (func $f32_lt (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.lt)
;; CHECK-LABEL: function wasm_func_2(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[CMP:.*]] = FLessThanInst (:boolean) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[OR:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[OR]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 3: f32.gt(a, b) — FGreaterThanInst + AsInt32Inst
  (func $f32_gt (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.gt)
;; CHECK-LABEL: function wasm_func_3(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[CMP:.*]] = FGreaterThanInst (:boolean) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[OR:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[OR]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 4: f32.le(a, b) — FLessThanOrEqualInst + AsInt32Inst
  (func $f32_le (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.le)
;; CHECK-LABEL: function wasm_func_4(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[CMP:.*]] = FLessThanOrEqualInst (:boolean) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[OR:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[OR]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 5: f32.ge(a, b) — FGreaterThanOrEqualInst + AsInt32Inst
  (func $f32_ge (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.ge)
)
;; CHECK-LABEL: function wasm_func_5(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[CMP:.*]] = FGreaterThanOrEqualInst (:boolean) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[OR:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[OR]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
