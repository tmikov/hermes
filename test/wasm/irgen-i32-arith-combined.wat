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

;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any, p2: any, p3: any): any
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:any) $local_0: any
;; CHECK:   %[[P0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:           StoreStackInst %[[P0]]: any, %[[L0]]: any
;; CHECK:   %[[L1:.*]] = AllocStackInst (:any) $local_1: any
;; CHECK:   %[[P1:.*]] = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:           StoreStackInst %[[P1]]: any, %[[L1]]: any
;; CHECK:   %[[L2:.*]] = AllocStackInst (:any) $local_2: any
;; CHECK:   %[[P2:.*]] = LoadParamInst (:any) %p2: any
;; CHECK-NEXT:           StoreStackInst %[[P2]]: any, %[[L2]]: any
;; CHECK:   %[[L3:.*]] = AllocStackInst (:any) $local_3: any
;; CHECK:   %[[P3:.*]] = LoadParamInst (:any) %p3: any
;; CHECK-NEXT:           StoreStackInst %[[P3]]: any, %[[L3]]: any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any) %[[L0]]: any
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any) %[[L1]]: any
;; CHECK-NEXT: %[[ADD:.*]] = BinaryAddInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[TRUNC1:.*]] = AsInt32Inst (:number) %[[ADD]]: any
;; CHECK-NEXT: %[[C:.*]] = LoadStackInst (:any) %[[L2]]: any
;; CHECK-NEXT: %[[MUL:.*]] = CallBuiltinInst (:any) [Math.imul]{{.*}}, %[[TRUNC1]]: number, %[[C]]: any
;; CHECK-NEXT: %[[D:.*]] = LoadStackInst (:any) %[[L3]]: any
;; CHECK-NEXT: %[[SUB:.*]] = BinarySubtractInst (:any) %[[MUL]]: any, %[[D]]: any
;; CHECK-NEXT: %[[TRUNC2:.*]] = AsInt32Inst (:number) %[[SUB]]: any
;; CHECK-NEXT:                  BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[TRUNC2]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
