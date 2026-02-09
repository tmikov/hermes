;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: function with local.tee.
;; Verifies the value is both stored to the local and returned.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir %t.wasm | %FileCheck %s

;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %1: any, %0: any
;; CHECK-NEXT:   %3 = LoadStackInst (:any) %0: any
;; CHECK-NEXT:        StoreStackInst %3: any, %0: any
;; CHECK-NEXT:        ReturnInst %3: any
;; CHECK-NEXT: function_end

(module
  (func (param i32) (result i32)
    local.get 0
    local.tee 0))
