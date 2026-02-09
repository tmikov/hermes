;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: function with i32.const.
;; Verifies LiteralNumber appears in the return instruction.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK-NEXT: %BB0:
;; CHECK:              BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %3 = PhiInst (:number) 42: number, %BB0
;; CHECK-NEXT:        ReturnInst %3: number
;; CHECK-NEXT: function_end

(module
  (func (result i32)
    i32.const 42))
