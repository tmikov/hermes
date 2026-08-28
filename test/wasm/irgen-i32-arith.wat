;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for i32 arithmetic operations.
;; Verifies that each operation produces the correct IR pattern with
;; correct data flow: params are loaded, fed to the operation, and the
;; result flows through phi to return.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: i32.add(a, b) → AsInt32(BinaryAdd(a, b))
  ;; The first function is checked exhaustively including param loading.
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)
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
;; CHECK-NEXT: %[[ADD:.*]] = BinaryAddInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[TRUNC:.*]] = AsInt32Inst (:number) %[[ADD]]: any
;; CHECK-NEXT:                 BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[TRUNC]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 1: i32.sub(a, b) → AsInt32(BinarySubtract(a, b))
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.sub)
;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SUB:.*]] = BinarySubtractInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[TRUNC:.*]] = AsInt32Inst (:number) %[[SUB]]: any
;; CHECK-NEXT:                 BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[TRUNC]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 2: i32.mul(a, b) → CallBuiltinInst(Math.imul, a, b)
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.mul)
;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[MUL:.*]] = CallBuiltinInst (:any) [Math.imul]{{.*}}, %[[A]]: any, %[[B]]: any
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[MUL]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 3: i32.and(a, b)
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.and)
;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[AND:.*]] = BinaryAndInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[AND]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 4: i32.or(a, b)
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.or)
;; CHECK-LABEL: function wasm_func_4(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[OR:.*]] = BinaryOrInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[OR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 5: i32.xor(a, b)
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.xor)
;; CHECK-LABEL: function wasm_func_5(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[XOR:.*]] = BinaryXorInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[XOR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 6: i32.shl(a, b)
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.shl)
;; CHECK-LABEL: function wasm_func_6(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SHL:.*]] = BinaryLeftShiftInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[SHL]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 7: i32.shr_s(a, b)
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.shr_s)
;; CHECK-LABEL: function wasm_func_7(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SHR:.*]] = BinaryRightShiftInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[SHR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 8: i32.shr_u(a, b)
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.shr_u)
;; CHECK-LABEL: function wasm_func_8(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[USHR:.*]] = BinaryUnsignedRightShiftInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT:                BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[USHR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end
)
