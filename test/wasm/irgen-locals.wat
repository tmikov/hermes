;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: function with declared locals and local.set/get.
;; Verifies correct data flow through local variables.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK-NEXT: %BB0:
;; Scope instructions followed by params allocated and initialized.
;; CHECK:        %1 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %2: any, %1: any
;; CHECK-NEXT:   %4 = AllocStackInst (:any) $local_1: any
;; CHECK-NEXT:   %5 = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:        StoreStackInst %5: any, %4: any
;; Declared local initialized to zero.
;; CHECK-NEXT:   %7 = AllocStackInst (:any) $local_2: any
;; CHECK-NEXT:        StoreStackInst 0: number, %7: any
;; local.get 0, local.set 2, local.get 2.
;; CHECK-NEXT:   %9 = LoadStackInst (:any) %1: any
;; CHECK-NEXT:         StoreStackInst %9: any, %7: any
;; CHECK-NEXT:   %11 = LoadStackInst (:any) %7: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:any) %11: any, %BB0
;; CHECK-NEXT:         ReturnInst %13: any
;; CHECK-NEXT: function_end

(module
  (func (param i32 i32) (result i32)
    (local i32)
    local.get 0
    local.set 2
    local.get 2))
