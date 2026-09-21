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

;; CHECK-LABEL: function wasm_func_0(p0: number): number 
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:number) $local_0: any
;; CHECK:   %[[P0:.*]] = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:           StoreStackInst %[[P0]]: number, %[[L0]]: number
;; CHECK:   %[[A:.*]] = LoadStackInst (:number) %[[L0]]: number
;; CHECK-NEXT: %[[SHL:.*]] = BinaryLeftShiftInst (:number) %[[A]]: number, 24: number
;; CHECK-NEXT: %[[SHR:.*]] = BinaryRightShiftInst (:number) %[[SHL]]: number, 24: number
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[SHR]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; i32.extend16_s: sign-extend from 16 bits
  (func $extend16 (param i32) (result i32)
    local.get 0
    i32.extend16_s))

;; CHECK-LABEL: function wasm_func_1(p0: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[SHL:.*]] = BinaryLeftShiftInst (:number) %[[A]]: number, 16: number
;; CHECK-NEXT: %[[SHR:.*]] = BinaryRightShiftInst (:number) %[[SHL]]: number, 16: number
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[SHR]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
