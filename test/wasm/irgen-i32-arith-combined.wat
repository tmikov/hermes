;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: combined i32 arithmetic: (a + b) * c - d
;; Verifies correct chaining of operations.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; (a + b) * c - d
  (func (param i32 i32 i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
    local.get 2
    i32.mul
    local.get 3
    i32.sub))

;; CHECK-LABEL: function wasm_func_0(p0: number, p1: number, p2: number, p3: number): number 
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:number) $local_0: any
;; CHECK:   %[[P0:.*]] = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:           StoreStackInst %[[P0]]: number, %[[L0]]: number
;; CHECK:   %[[L1:.*]] = AllocStackInst (:number) $local_1: any
;; CHECK:   %[[P1:.*]] = LoadParamInst (:number) %p1: number
;; CHECK-NEXT:           StoreStackInst %[[P1]]: number, %[[L1]]: number
;; CHECK:   %[[L2:.*]] = AllocStackInst (:number) $local_2: any
;; CHECK:   %[[P2:.*]] = LoadParamInst (:number) %p2: number
;; CHECK-NEXT:           StoreStackInst %[[P2]]: number, %[[L2]]: number
;; CHECK:   %[[L3:.*]] = AllocStackInst (:number) $local_3: any
;; CHECK:   %[[P3:.*]] = LoadParamInst (:number) %p3: number
;; CHECK-NEXT:           StoreStackInst %[[P3]]: number, %[[L3]]: number
;; CHECK:   %[[A:.*]] = LoadStackInst (:number) %[[L0]]: number
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number) %[[L1]]: number
;; CHECK-NEXT: %[[ADD:.*]] = FAddInst (:number) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[TRUNC1:.*]] = AsInt32Inst (:number) %[[ADD]]: number
;; CHECK-NEXT: %[[C:.*]] = LoadStackInst (:number) %[[L2]]: number
;; CHECK-NEXT: %[[MUL:.*]] = CallBuiltinInst (:number) [Math.imul]{{.*}}, %[[TRUNC1]]: number, %[[C]]: number
;; CHECK-NEXT: %[[D:.*]] = LoadStackInst (:number) %[[L3]]: number
;; CHECK-NEXT: %[[SUB:.*]] = FSubtractInst (:number) %[[MUL]]: number, %[[D]]: number
;; CHECK-NEXT: %[[TRUNC2:.*]] = AsInt32Inst (:number) %[[SUB]]: number
;; CHECK-NEXT:                  BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[TRUNC2]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
