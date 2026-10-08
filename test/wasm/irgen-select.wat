;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test select instruction generates correct IR.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Test 1: select with constant true condition -> 42
  ;; BB0=entry, BB1=exit, BB2=trueBlock, BB3=falseBlock, BB4=mergeBlock
  (func $select_true (export "select_true") (result i32)
    (select (i32.const 42) (i32.const 99) (i32.const 1)))

;; CHECK-LABEL: function wasm_func_0(): number 
;; CHECK:         CondBranchInst 1: number, %BB2, %BB3
;; CHECK:       %BB2:
;; CHECK-NEXT:    BranchInst %BB4
;; CHECK:       %BB3:
;; CHECK-NEXT:    BranchInst %BB4
;; CHECK:       %BB4:
;; CHECK-NEXT:    %[[PHI:.*]] = PhiInst (:number) 42: number, %BB2, 99: number, %BB3

  ;; Test 2: select with constant false condition -> 99
  (func $select_false (export "select_false") (result i32)
    (select (i32.const 42) (i32.const 99) (i32.const 0)))

;; CHECK-LABEL: function wasm_func_1(): number 
;; CHECK:         CondBranchInst 0: number, %BB2, %BB3
;; CHECK:       %BB2:
;; CHECK-NEXT:    BranchInst %BB4
;; CHECK:       %BB3:
;; CHECK-NEXT:    BranchInst %BB4
;; CHECK:       %BB4:
;; CHECK-NEXT:    %[[PHI:.*]] = PhiInst (:number) 42: number, %BB2, 99: number, %BB3

  ;; Test 3: select with parameter condition
  (func $select_param (export "select_param") (param i32) (param i32) (param i32) (result i32)
    (select (local.get 0) (local.get 1) (local.get 2))))

;; CHECK-LABEL: function wasm_func_2(p0: number, p1: number, p2: number): number 
;; CHECK:         CondBranchInst %[[COND:.*]]: number, %BB2, %BB3
;; CHECK:       %BB2:
;; CHECK-NEXT:    BranchInst %BB4
;; CHECK:       %BB3:
;; CHECK-NEXT:    BranchInst %BB4
;; CHECK:       %BB4:
;; CHECK-NEXT:    %[[PHI:.*]] = PhiInst (:number) %[[TV:.*]]: number, %BB2, %[[FV:.*]]: number, %BB3
