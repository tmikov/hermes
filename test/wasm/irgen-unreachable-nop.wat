;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for unreachable and nop instructions.
;; D.11: unreachable traps, nop is a no-op.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Function 0: just unreachable
  ;; unreachable calls wasmTrap helper then emits UnreachableInst.
  ;; The exit block (BB1) is unreachable with a dead ReturnInst.
  (func
    unreachable)

;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK-NEXT: %BB0:
;; CHECK:        CallBuiltinInst {{.*}}[HermesBuiltin.wasmTrap]
;; CHECK-NEXT:   UnreachableInst
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   ReturnInst undefined: undefined
;; CHECK-NEXT: function_end

  ;; Function 1: just nop
  ;; nop emits nothing - function is just entry block branching to exit block.
  (func
    nop)

;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK-NEXT: %BB0:
;; CHECK:              BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end

  ;; Function 2: nop then return value
  ;; nop should not affect the value stack or control flow.
  (func (result i32)
    nop
    i32.const 99)

;; CHECK-LABEL: function wasm_func_2(): any
;; CHECK-NEXT: %BB0:
;; CHECK:              BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %[[PHI:.*]] = PhiInst (:number) 99: number, %BB0
;; CHECK-NEXT:                 ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; Function 3: push value, then unreachable - dead code after unreachable.
  ;; The pushed constant before unreachable is live, but unreachable kills
  ;; the control flow. Code after unreachable is dead.
  (func (result i32)
    i32.const 42
    unreachable))

;; CHECK-LABEL: function wasm_func_3(): any
;; CHECK-NEXT: %BB0:
;; CHECK:        CallBuiltinInst {{.*}}[HermesBuiltin.wasmTrap]
;; CHECK-NEXT:   UnreachableInst
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %{{.*}} = PhiInst (:notype)
;; CHECK-NEXT:            ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
