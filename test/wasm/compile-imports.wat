;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test Wasm module with imports compiles to IR.
;; Imported functions get placeholder stubs that return undefined.
;; Defined functions are compiled normally.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (import "env" "log" (func $log (param i32)))
  (import "env" "g" (global i32))
  (memory 1)

  ;; First defined function (index 1); global.get is unsupported so
  ;; falls through to undefined return.
  (func (export "main") (result i32)
    global.get 0
  )
;; Imported function placeholder.
;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK: %BB0:
;; CHECK-NEXT: ReturnInst undefined: undefined
;; CHECK-NEXT: function_end

;; First defined function.
;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK: %BB0:
;; CHECK:   BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %{{.*}} = PhiInst (:undefined) undefined: undefined, %BB0
;; CHECK-NEXT:           ReturnInst
;; CHECK-NEXT: function_end

  ;; Second defined function (index 2); uses i32.add on the param.
  (func $helper (param i32) (result i32)
    local.get 0
    i32.const 1
    i32.add
  )
;; CHECK-LABEL: function wasm_func_2(p0: any): any
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT: %[[P0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:              StoreStackInst %[[P0]]: any, %[[L0]]: any
;; CHECK:   %[[V:.*]] = LoadStackInst (:any) %[[L0]]: any
;; CHECK-NEXT: %[[ADD:.*]] = BinaryAddInst (:any) %[[V]]: any, 1: number
;; CHECK-NEXT: %[[TRUNC:.*]] = AsInt32Inst (:number) %[[ADD]]: any
;; CHECK-NEXT:                 BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[TRUNC]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
)
