;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: combined i32 arithmetic: (a + b) * c - d
;; Verifies correct chaining of operations.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any, p2: any, p3: any): any
;; CHECK:   %{{[0-9]+}} = BinaryAddInst
;; CHECK:   %{{[0-9]+}} = AsInt32Inst
;; CHECK:   %{{[0-9]+}} = CallBuiltinInst
;; CHECK:   %{{[0-9]+}} = BinarySubtractInst
;; CHECK:   %{{[0-9]+}} = AsInt32Inst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

(module
  ;; (a + b) * c - d
  (func (param i32 i32 i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
    local.get 2
    i32.mul
    local.get 3
    i32.sub))
