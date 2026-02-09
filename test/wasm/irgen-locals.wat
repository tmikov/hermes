;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: function with declared locals and local.set/get.
;; Verifies correct data flow through local variables.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir %t.wasm | %FileCheck %s

;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK-NEXT: %BB0:
;; Params are allocated and initialized.
;; CHECK-NEXT:   %0 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %1: any, %0: any
;; CHECK-NEXT:   %3 = AllocStackInst (:any) $local_1: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:        StoreStackInst %4: any, %3: any
;; Declared local initialized to zero.
;; CHECK-NEXT:   %6 = AllocStackInst (:any) $local_2: any
;; CHECK-NEXT:        StoreStackInst 0: number, %6: any
;; local.get 0, local.set 2, local.get 2.
;; CHECK-NEXT:   %8 = LoadStackInst (:any) %0: any
;; CHECK-NEXT:        StoreStackInst %8: any, %6: any
;; CHECK-NEXT:   %10 = LoadStackInst (:any) %6: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %12 = PhiInst (:any) %10: any, %BB0
;; CHECK-NEXT:         ReturnInst %12: any
;; CHECK-NEXT: function_end

(module
  (func (param i32 i32) (result i32)
    (local i32)
    local.get 0
    local.set 2
    local.get 2))
