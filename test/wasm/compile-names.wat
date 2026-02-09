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

  ;; $negate: i32.sub(0, a)
  (func $negate (export "negate") (param i32) (result i32)
    i32.const 0
    local.get 0
    i32.sub
  )
;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; CHECK:   %[[V:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SUB:.*]] = BinarySubtractInst (:any) 0: number, %[[V]]: any
;; CHECK-NEXT: %[[TRUNC:.*]] = AsInt32Inst (:number) %[[SUB]]: any
;; CHECK-NEXT:                 BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[TRUNC]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
)
