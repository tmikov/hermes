;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for f32 comparison operations (E.3).
;; Same IR pattern as f64/i32 comparisons -- boolean result converted
;; to i32 via BinaryOrInst(cmp, 0).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: f32.eq(a, b) — BinaryStrictlyEqualInst + BinaryOrInst
  ;; The first function is checked exhaustively including param loading.
  (func $f32_eq (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.eq)
;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK: %BB0:
;; CHECK:   %[[L0_0:.*]] = AllocStackInst (:any)
;; CHECK:   %[[P0_0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:              StoreStackInst %[[P0_0]]: any, %[[L0_0]]: any
;; CHECK:   %[[L1_0:.*]] = AllocStackInst (:any)
;; CHECK:   %[[P1_0:.*]] = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:              StoreStackInst %[[P1_0]]: any, %[[L1_0]]: any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any) %[[L0_0]]: any
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any) %[[L1_0]]: any
;; CHECK-NEXT: %[[CMP:.*]] = BinaryStrictlyEqualInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[OR:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[OR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 1: f32.ne(a, b) — BinaryStrictlyNotEqualInst + BinaryOrInst
  (func $f32_ne (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.ne)
;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CMP:.*]] = BinaryStrictlyNotEqualInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[OR:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[OR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 2: f32.lt(a, b) — BinaryLessThanInst + BinaryOrInst
  (func $f32_lt (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.lt)
;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CMP:.*]] = BinaryLessThanInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[OR:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[OR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 3: f32.gt(a, b) — BinaryGreaterThanInst + BinaryOrInst
  (func $f32_gt (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.gt)
;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CMP:.*]] = BinaryGreaterThanInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[OR:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[OR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 4: f32.le(a, b) — BinaryLessThanOrEqualInst + BinaryOrInst
  (func $f32_le (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.le)
;; CHECK-LABEL: function wasm_func_4(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CMP:.*]] = BinaryLessThanOrEqualInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[OR:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[OR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 5: f32.ge(a, b) — BinaryGreaterThanOrEqualInst + BinaryOrInst
  (func $f32_ge (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.ge)
)
;; CHECK-LABEL: function wasm_func_5(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CMP:.*]] = BinaryGreaterThanOrEqualInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[OR:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[OR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end
