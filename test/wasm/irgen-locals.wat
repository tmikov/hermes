;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: function with declared locals and local.set/get.
;; Verifies correct data flow through local variables.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (func (param i32 i32) (result i32)
    (local i32)
    local.get 0
    local.set 2
    local.get 2))

;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK-NEXT: %BB0:
;; Scope instructions followed by params allocated and initialized.
;; CHECK:   %[[L0:.*]] = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %[[P0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %[[P0]]: any, %[[L0]]: any
;; CHECK-NEXT:   %[[L1:.*]] = AllocStackInst (:any) $local_1: any
;; CHECK-NEXT:   %[[P1:.*]] = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:        StoreStackInst %[[P1]]: any, %[[L1]]: any
;; Declared local initialized to zero.
;; CHECK-NEXT:   %[[L2:.*]] = AllocStackInst (:any) $local_2: any
;; CHECK-NEXT:        StoreStackInst 0: number, %[[L2]]: any
;; local.get 0, local.set 2, local.get 2.
;; CHECK-NEXT:   %[[V0:.*]] = LoadStackInst (:any) %[[L0]]: any
;; CHECK-NEXT:         StoreStackInst %[[V0]]: any, %[[L2]]: any
;; CHECK-NEXT:   %[[V2:.*]] = LoadStackInst (:any) %[[L2]]: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %[[PHI:.*]] = PhiInst (:any) %[[V2]]: any, %BB0
;; CHECK-NEXT:         ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end
