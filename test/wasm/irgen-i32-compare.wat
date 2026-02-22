;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for i32 comparison operations.
;; Verifies that each operation produces the correct IR pattern:
;; comparison -> boolean -> AsInt32(bool) -> i32.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: i32.eq(a, b) -> FEqualInst, then AsInt32 to convert to i32
  ;; First function checked exhaustively including param loading.
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.eq)

;; CHECK-LABEL: function wasm_func_0(p0: number, p1: number): number 
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:number) $local_0: any
;; CHECK:   %[[P0:.*]] = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:           StoreStackInst %[[P0]]: number, %[[L0]]: number
;; CHECK:   %[[L1:.*]] = AllocStackInst (:number) $local_1: any
;; CHECK:   %[[P1:.*]] = LoadParamInst (:number) %p1: number
;; CHECK-NEXT:           StoreStackInst %[[P1]]: number, %[[L1]]: number
;; CHECK:   %[[A:.*]] = LoadStackInst (:number) %[[L0]]: number
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number) %[[L1]]: number
;; CHECK-NEXT: %[[CMP:.*]] = FEqualInst (:boolean) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 1: i32.ne
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.ne)

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

  ;; func 2: i32.lt_s (signed: AsInt32 both operands first)
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.lt_s)

;; CHECK-LABEL: function wasm_func_2(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[SA:.*]] = AsInt32Inst (:number) %[[A]]: number
;; CHECK-NEXT: %[[SB:.*]] = AsInt32Inst (:number) %[[B]]: number
;; CHECK-NEXT: %[[CMP:.*]] = FLessThanInst (:boolean) %[[SA]]: number, %[[SB]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 3: i32.gt_s
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.gt_s)

;; CHECK-LABEL: function wasm_func_3(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[SA:.*]] = AsInt32Inst (:number) %[[A]]: number
;; CHECK-NEXT: %[[SB:.*]] = AsInt32Inst (:number) %[[B]]: number
;; CHECK-NEXT: %[[CMP:.*]] = FGreaterThanInst (:boolean) %[[SA]]: number, %[[SB]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 4: i32.le_s
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.le_s)

;; CHECK-LABEL: function wasm_func_4(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[SA:.*]] = AsInt32Inst (:number) %[[A]]: number
;; CHECK-NEXT: %[[SB:.*]] = AsInt32Inst (:number) %[[B]]: number
;; CHECK-NEXT: %[[CMP:.*]] = FLessThanOrEqualInst (:boolean) %[[SA]]: number, %[[SB]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 5: i32.ge_s
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.ge_s)

;; CHECK-LABEL: function wasm_func_5(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[SA:.*]] = AsInt32Inst (:number) %[[A]]: number
;; CHECK-NEXT: %[[SB:.*]] = AsInt32Inst (:number) %[[B]]: number
;; CHECK-NEXT: %[[CMP:.*]] = FGreaterThanOrEqualInst (:boolean) %[[SA]]: number, %[[SB]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 6: i32.lt_u (unsigned: AsUint32 both operands first)
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.lt_u)

;; CHECK-LABEL: function wasm_func_6(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[UA:.*]] = AsUint32Inst (:number) %[[A]]: number
;; CHECK-NEXT: %[[UB:.*]] = AsUint32Inst (:number) %[[B]]: number
;; CHECK-NEXT: %[[CMP:.*]] = FLessThanInst (:boolean) %[[UA]]: number, %[[UB]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 7: i32.gt_u
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.gt_u)

;; CHECK-LABEL: function wasm_func_7(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[UA:.*]] = AsUint32Inst (:number) %[[A]]: number
;; CHECK-NEXT: %[[UB:.*]] = AsUint32Inst (:number) %[[B]]: number
;; CHECK-NEXT: %[[CMP:.*]] = FGreaterThanInst (:boolean) %[[UA]]: number, %[[UB]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 8: i32.le_u
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.le_u)

;; CHECK-LABEL: function wasm_func_8(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[UA:.*]] = AsUint32Inst (:number) %[[A]]: number
;; CHECK-NEXT: %[[UB:.*]] = AsUint32Inst (:number) %[[B]]: number
;; CHECK-NEXT: %[[CMP:.*]] = FLessThanOrEqualInst (:boolean) %[[UA]]: number, %[[UB]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 9: i32.ge_u
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.ge_u)

;; CHECK-LABEL: function wasm_func_9(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[UA:.*]] = AsUint32Inst (:number) %[[A]]: number
;; CHECK-NEXT: %[[UB:.*]] = AsUint32Inst (:number) %[[B]]: number
;; CHECK-NEXT: %[[CMP:.*]] = FGreaterThanOrEqualInst (:boolean) %[[UA]]: number, %[[UB]]: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 10: i32.eqz (unary: compare with 0)
  (func (param i32) (result i32)
    local.get 0
    i32.eqz))

;; CHECK-LABEL: function wasm_func_10(p0: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[CMP:.*]] = FEqualInst (:boolean) %[[A]]: number, 0: number
;; CHECK-NEXT: %[[R:.*]] = AsInt32Inst (:number) %[[CMP]]: boolean
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
