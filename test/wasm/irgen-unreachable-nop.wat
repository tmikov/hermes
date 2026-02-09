;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for unreachable and nop instructions.
;; D.11: unreachable traps, nop is a no-op.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; Test 1: unreachable in a void function.
;; unreachable emits UnreachableInst, then a dead block follows.
;; The exit block (BB1) is unreachable with a dead ReturnInst.
;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK-NEXT: %BB0:
;; CHECK:              UnreachableInst
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end

;; Test 2: nop in a void function (should produce no extra instructions).
;; nop emits nothing — function is just entry block branching to exit block.
;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK-NEXT: %BB0:
;; CHECK:              BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end

;; Test 3: nop followed by a constant return.
;; nop should not affect the value stack or control flow.
;; CHECK-LABEL: function wasm_func_2(): any
;; CHECK-NEXT: %BB0:
;; CHECK:              BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %3 = PhiInst (:number) 99: number, %BB0
;; CHECK-NEXT:        ReturnInst %3: number
;; CHECK-NEXT: function_end

;; Test 4: unreachable after pushing a value — dead code after unreachable.
;; The pushed constant before unreachable is live, but unreachable kills
;; the control flow. Code after unreachable is dead.
;; CHECK-LABEL: function wasm_func_3(): any
;; CHECK-NEXT: %BB0:
;; CHECK:              UnreachableInst
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %3 = PhiInst (:notype)
;; CHECK-NEXT:        ReturnInst %3: notype
;; CHECK-NEXT: function_end

(module
  ;; Function 0: just unreachable
  (func
    unreachable)

  ;; Function 1: just nop
  (func
    nop)

  ;; Function 2: nop then return value
  (func (result i32)
    nop
    i32.const 99)

  ;; Function 3: push value, then unreachable (dead code follows)
  (func (result i32)
    i32.const 42
    unreachable))
