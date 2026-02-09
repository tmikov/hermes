;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: function with local.tee.
;; Verifies the value is both stored to the local and returned.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (func (param i32) (result i32)
    local.get 0
    local.tee 0))

;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %[[P0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %[[P0]]: any, %[[L0]]: any
;; CHECK-NEXT:   %[[V:.*]] = LoadStackInst (:any) %[[L0]]: any
;; CHECK-NEXT:        StoreStackInst %[[V]]: any, %[[L0]]: any
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %[[PHI:.*]] = PhiInst (:any) %[[V]]: any, %BB0
;; CHECK-NEXT:        ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end
