;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: function that returns its parameter.
;; Verifies LoadParamInst, StoreStackInst, LoadStackInst, ReturnInst.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK:        %2 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %3 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %3: any, %2: any
;; CHECK-NEXT:   %5 = LoadStackInst (:any) %2: any
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %7 = PhiInst (:any) %5: any, %BB0
;; CHECK-NEXT:        ReturnInst %7: any
;; CHECK-NEXT: function_end

(module
  (func (param i32) (result i32)
    local.get 0))
