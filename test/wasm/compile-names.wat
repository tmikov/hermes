;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test Wasm module with named functions compiles to IR.
;; Note: The name section is parsed after the code section in the Wasm binary,
;; so function names are not yet applied to IR functions. This will be
;; improved in a future step. For now, functions use wasm_func_N names.

;; REQUIRES: wasm
;; RUN: %wat2wasm --debug-names %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; $add: i32.add(a, b)
  (func $add (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
  )
;; CHECK-LABEL: function wasm_func_0(p0: number, p1: number): number 
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT: %[[P0:.*]] = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:              StoreStackInst %[[P0]]: number, %[[L0]]: number
;; CHECK:   %[[L1:.*]] = AllocStackInst (:number) $local_1: any
;; CHECK-NEXT: %[[P1:.*]] = LoadParamInst (:number) %p1: number
;; CHECK-NEXT:              StoreStackInst %[[P1]]: number, %[[L1]]: number
;; CHECK:   %[[A:.*]] = LoadStackInst (:number) %[[L0]]: number
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number) %[[L1]]: number
;; CHECK-NEXT: %[[ADD:.*]] = FAddInst (:number) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT: %[[TRUNC:.*]] = AsInt32Inst (:number) %[[ADD]]: number
;; CHECK-NEXT:                 BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[TRUNC]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; $negate: i32.sub(0, a)
  (func $negate (export "negate") (param i32) (result i32)
    i32.const 0
    local.get 0
    i32.sub
  )
;; CHECK-LABEL: function wasm_func_1(p0: number): number 
;; CHECK:   %[[V:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[SUB:.*]] = FSubtractInst (:number) 0: number, %[[V]]: number
;; CHECK-NEXT: %[[TRUNC:.*]] = AsInt32Inst (:number) %[[SUB]]: number
;; CHECK-NEXT:                 BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[TRUNC]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
)
