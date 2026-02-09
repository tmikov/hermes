;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for return and drop instructions.
;; D.5: Explicit return, implicit return (fallthrough), and drop.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir %t.wasm | %FileCheck %s

;; Test 1: Explicit return with i32 result.
;; The explicit return jumps directly to ReturnInst; the exit block (BB1)
;; is unreachable with an empty PhiInst.
;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:        ReturnInst 42: number
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %1 = PhiInst (:notype)
;; CHECK-NEXT:        ReturnInst %1: notype
;; CHECK-NEXT: %BB2:
;; CHECK-NEXT: function_end

;; Test 2: Implicit return (fallthrough) with i32 result.
;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %1 = PhiInst (:number) 42: number, %BB0
;; CHECK-NEXT:        ReturnInst %1: number
;; CHECK-NEXT: function_end

;; Test 3: Void function with drop.
;; CHECK-LABEL: function wasm_func_2(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end

(module
  ;; Function 0: explicit return
  (func (result i32)
    i32.const 42
    return)

  ;; Function 1: implicit return (fallthrough)
  (func (result i32)
    i32.const 42)

  ;; Function 2: void function with drop
  (func
    i32.const 42
    drop))
