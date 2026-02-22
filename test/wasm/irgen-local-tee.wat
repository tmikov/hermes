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

;; CHECK-LABEL: function wasm_func_0(p0: number): number 
;; CHECK-NEXT: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:   %[[P0:.*]] = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:        StoreStackInst %[[P0]]: number, %[[L0]]: number
;; CHECK-NEXT:   %[[V:.*]] = LoadStackInst (:number) %[[L0]]: number
;; CHECK-NEXT:        StoreStackInst %[[V]]: number, %[[L0]]: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %[[PHI:.*]] = PhiInst (:number) %[[V]]: number, %BB0
;; CHECK-NEXT:        ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
