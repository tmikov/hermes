;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for i32 comparison operations.
;; Verifies that each operation produces the correct IR pattern:
;; comparison -> boolean -> BitOr(bool, 0) -> i32.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: i32.eq(a, b) -> BinaryStrictlyEqual, then BitOr to convert to i32
  ;; First function checked exhaustively including param loading.
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.eq)

;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:any) $local_0: any
;; CHECK:   %[[P0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:           StoreStackInst %[[P0]]: any, %[[L0]]: any
;; CHECK:   %[[L1:.*]] = AllocStackInst (:any) $local_1: any
;; CHECK:   %[[P1:.*]] = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:           StoreStackInst %[[P1]]: any, %[[L1]]: any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any) %[[L0]]: any
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any) %[[L1]]: any
;; CHECK-NEXT: %[[CMP:.*]] = BinaryStrictlyEqualInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 1: i32.ne
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.ne)

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

  ;; func 2: i32.lt_s (signed: AsInt32 both operands first)
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.lt_s)

;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SA:.*]] = AsInt32Inst (:number) %[[A]]: any
;; CHECK-NEXT: %[[SB:.*]] = AsInt32Inst (:number) %[[B]]: any
;; CHECK-NEXT: %[[CMP:.*]] = BinaryLessThanInst (:any) %[[SA]]: number, %[[SB]]: number
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 3: i32.gt_s
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.gt_s)

;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SA:.*]] = AsInt32Inst (:number) %[[A]]: any
;; CHECK-NEXT: %[[SB:.*]] = AsInt32Inst (:number) %[[B]]: any
;; CHECK-NEXT: %[[CMP:.*]] = BinaryGreaterThanInst (:any) %[[SA]]: number, %[[SB]]: number
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 4: i32.le_s
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.le_s)

;; CHECK-LABEL: function wasm_func_4(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SA:.*]] = AsInt32Inst (:number) %[[A]]: any
;; CHECK-NEXT: %[[SB:.*]] = AsInt32Inst (:number) %[[B]]: any
;; CHECK-NEXT: %[[CMP:.*]] = BinaryLessThanOrEqualInst (:any) %[[SA]]: number, %[[SB]]: number
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 5: i32.ge_s
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.ge_s)

;; CHECK-LABEL: function wasm_func_5(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SA:.*]] = AsInt32Inst (:number) %[[A]]: any
;; CHECK-NEXT: %[[SB:.*]] = AsInt32Inst (:number) %[[B]]: any
;; CHECK-NEXT: %[[CMP:.*]] = BinaryGreaterThanOrEqualInst (:any) %[[SA]]: number, %[[SB]]: number
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 6: i32.lt_u (unsigned: AsUint32 both operands first)
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.lt_u)

;; CHECK-LABEL: function wasm_func_6(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[UA:.*]] = AsUint32Inst (:number) %[[A]]: any
;; CHECK-NEXT: %[[UB:.*]] = AsUint32Inst (:number) %[[B]]: any
;; CHECK-NEXT: %[[CMP:.*]] = BinaryLessThanInst (:any) %[[UA]]: number, %[[UB]]: number
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 7: i32.gt_u
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.gt_u)

;; CHECK-LABEL: function wasm_func_7(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[UA:.*]] = AsUint32Inst (:number) %[[A]]: any
;; CHECK-NEXT: %[[UB:.*]] = AsUint32Inst (:number) %[[B]]: any
;; CHECK-NEXT: %[[CMP:.*]] = BinaryGreaterThanInst (:any) %[[UA]]: number, %[[UB]]: number
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 8: i32.le_u
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.le_u)

;; CHECK-LABEL: function wasm_func_8(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[UA:.*]] = AsUint32Inst (:number) %[[A]]: any
;; CHECK-NEXT: %[[UB:.*]] = AsUint32Inst (:number) %[[B]]: any
;; CHECK-NEXT: %[[CMP:.*]] = BinaryLessThanOrEqualInst (:any) %[[UA]]: number, %[[UB]]: number
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 9: i32.ge_u
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.ge_u)

;; CHECK-LABEL: function wasm_func_9(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[UA:.*]] = AsUint32Inst (:number) %[[A]]: any
;; CHECK-NEXT: %[[UB:.*]] = AsUint32Inst (:number) %[[B]]: any
;; CHECK-NEXT: %[[CMP:.*]] = BinaryGreaterThanOrEqualInst (:any) %[[UA]]: number, %[[UB]]: number
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 10: i32.eqz (unary: compare with 0)
  (func (param i32) (result i32)
    local.get 0
    i32.eqz))

;; CHECK-LABEL: function wasm_func_10(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CMP:.*]] = BinaryStrictlyEqualInst (:any) %[[A]]: any, 0: number
;; CHECK-NEXT: %[[R:.*]] = BinaryOrInst (:any) %[[CMP]]: any, 0: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end
