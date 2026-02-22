;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test if/else generates correct IR.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Test 1: if/else with result, true condition -> 42
  (func $if_true (export "if_true") (result i32)
    (if (result i32) (i32.const 1)
      (then (i32.const 42))
      (else (i32.const 99))))

;; CHECK-LABEL: function wasm_func_0(): number 
;; CHECK:         CondBranchInst 1: number, %BB2, %BB3
;; CHECK:       %BB2:
;; CHECK-NEXT:    BranchInst %BB4
;; CHECK:       %BB3:
;; CHECK-NEXT:    BranchInst %BB4
;; CHECK:       %BB4:
;; CHECK-NEXT:    %[[PHI:.*]] = PhiInst (:number) 42: number, %BB2, 99: number, %BB3

  ;; Test 2: if/else with result, false condition -> 99
  (func $if_false (export "if_false") (result i32)
    (if (result i32) (i32.const 0)
      (then (i32.const 42))
      (else (i32.const 99))))

;; CHECK-LABEL: function wasm_func_1(): number 
;; CHECK:         CondBranchInst 0: number, %BB2, %BB3
;; CHECK:       %BB2:
;; CHECK-NEXT:    BranchInst %BB4
;; CHECK:       %BB3:
;; CHECK-NEXT:    BranchInst %BB4
;; CHECK:       %BB4:
;; CHECK-NEXT:    %[[PHI:.*]] = PhiInst (:number) 42: number, %BB2, 99: number, %BB3

  ;; Test 3: if without else, void body
  (func $if_void (export "if_void") (param i32) (result i32)
    (local i32)
    (i32.const 10)
    (local.set 1)
    (if (local.get 0)
      (then
        (i32.const 20)
        (local.set 1)))
    (local.get 1))

;; CHECK-LABEL: function wasm_func_2(p0: number): number 
;; CHECK:         CondBranchInst %[[COND:.*]]: number, %BB2, %BB3
;; CHECK:       %BB2:
;; CHECK-NEXT:    StoreStackInst 20: number, %[[LOCAL:.*]]: number
;; CHECK-NEXT:    BranchInst %BB4
;; CHECK:       %BB3:
;; CHECK-NEXT:    BranchInst %BB4

  ;; Test 4: nested if/else
  (func $nested (export "nested") (param i32) (param i32) (result i32)
    (if (result i32) (local.get 0)
      (then
        (if (result i32) (local.get 1)
          (then (i32.const 1))
          (else (i32.const 2))))
      (else (i32.const 3)))))

;; CHECK-LABEL: function wasm_func_3(p0: number, p1: number): number 
;; CHECK:         CondBranchInst %[[C0:.*]]: number, %BB2, %BB3
;; CHECK:       %BB2:
;; CHECK:         CondBranchInst %[[C1:.*]]: number, %BB5, %BB6
;; CHECK:       %BB3:
;; CHECK-NEXT:    BranchInst %BB4
;; CHECK:       %BB4:
;; CHECK-NEXT:    %[[OUTER:.*]] = PhiInst (:number) %[[INNER:.*]]: number, %BB7, 3: number, %BB3
;; CHECK:       %BB5:
;; CHECK-NEXT:    BranchInst %BB7
;; CHECK:       %BB6:
;; CHECK-NEXT:    BranchInst %BB7
;; CHECK:       %BB7:
;; CHECK-NEXT:    %[[INNER]] = PhiInst (:number) 1: number, %BB5, 2: number, %BB6
