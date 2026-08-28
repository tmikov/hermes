;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for return and drop instructions.
;; D.5: Explicit return, implicit return (fallthrough), and drop.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Function 0: explicit return
  ;; The explicit return jumps directly to ReturnInst; the exit block (BB1)
  ;; is unreachable with an empty PhiInst.
  (func (result i32)
    i32.const 42
    return)

;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK-NEXT: %BB0:
;; CHECK:              ReturnInst 42: number
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %{{.*}} = PhiInst (:notype)
;; CHECK-NEXT:            ReturnInst undefined: undefined
;; CHECK-NEXT: function_end

  ;; Function 1: implicit return (fallthrough)
  (func (result i32)
    i32.const 42)

;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK-NEXT: %BB0:
;; CHECK:              BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %[[PHI:.*]] = PhiInst (:number) 42: number, %BB0
;; CHECK-NEXT:                 ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; Function 2: void function with drop
  (func
    i32.const 42
    drop))

;; CHECK-LABEL: function wasm_func_2(): any
;; CHECK-NEXT: %BB0:
;; CHECK:              BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
