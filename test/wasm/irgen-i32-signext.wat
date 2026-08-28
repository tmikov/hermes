;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test i32.extend8_s and i32.extend16_s IR generation.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; i32.extend8_s: sign-extend from 8 bits
  ;; First function checked exhaustively including param loading.
  (func $extend8 (param i32) (result i32)
    local.get 0
    i32.extend8_s)

;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:any) $local_0: any
;; CHECK:   %[[P0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:           StoreStackInst %[[P0]]: any, %[[L0]]: any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any) %[[L0]]: any
;; CHECK-NEXT: %[[SHL:.*]] = BinaryLeftShiftInst (:any) %[[A]]: any, 24: number
;; CHECK-NEXT: %[[SHR:.*]] = BinaryRightShiftInst (:any) %[[SHL]]: any, 24: number
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[SHR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; i32.extend16_s: sign-extend from 16 bits
  (func $extend16 (param i32) (result i32)
    local.get 0
    i32.extend16_s))

;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SHL:.*]] = BinaryLeftShiftInst (:any) %[[A]]: any, 16: number
;; CHECK-NEXT: %[[SHR:.*]] = BinaryRightShiftInst (:any) %[[SHL]]: any, 16: number
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[SHR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end
