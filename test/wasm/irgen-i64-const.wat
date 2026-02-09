;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: i64 constant split into lo32/hi32 pair.
;; Phase 1 represents i64 as two values on the stack (lo, hi).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; i64.const 0x0000000100000002 = 4294967298
;; Split: lo32 = 2, hi32 = 1
;; The function has result type i64, endFunction() returns top of stack = hi=1.
;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK-NEXT: %BB0:
;; CHECK:              BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %3 = PhiInst (:number) 1: number, %BB0
;; CHECK-NEXT:        ReturnInst %3: number
;; CHECK-NEXT: function_end

(module
  (func (result i64)
    i64.const 4294967298))
