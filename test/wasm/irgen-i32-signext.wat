;;  Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test i32.extend8_s and i32.extend16_s IR generation.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; i32.extend8_s: sign-extend from 8 bits
  (func $extend8 (param i32) (result i32)
    local.get 0
    i32.extend8_s
  )

  ;; i32.extend16_s: sign-extend from 16 bits
  (func $extend16 (param i32) (result i32)
    local.get 0
    i32.extend16_s
  )
)

;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK:   BinaryLeftShiftInst (:any) %{{.*}}, 24: number
;; CHECK:   BinaryRightShiftInst (:any) %{{.*}}, 24: number
;; CHECK:   ReturnInst

;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; CHECK:   BinaryLeftShiftInst (:any) %{{.*}}, 16: number
;; CHECK:   BinaryRightShiftInst (:any) %{{.*}}, 16: number
;; CHECK:   ReturnInst
