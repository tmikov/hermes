;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test basic Wasm module compilation to Hermes IR.
;; Verifies that a simple two-parameter function produces the correct
;; param loading, arithmetic, and return pattern.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (memory (export "memory") 1)

  ;; A simple add function with two i32 params.
  (func (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
  )
;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT: %[[P0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:              StoreStackInst %[[P0]]: any, %[[L0]]: any
;; CHECK:   %[[L1:.*]] = AllocStackInst (:any) $local_1: any
;; CHECK-NEXT: %[[P1:.*]] = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:              StoreStackInst %[[P1]]: any, %[[L1]]: any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any) %[[L0]]: any
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any) %[[L1]]: any
;; CHECK-NEXT: %[[ADD:.*]] = BinaryAddInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[TRUNC:.*]] = AsInt32Inst (:number) %[[ADD]]: any
;; CHECK-NEXT:                 BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[TRUNC]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
)
