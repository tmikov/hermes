;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test module with a table and element segments compiles to IR.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir %t.wasm | %FileCheck %s

;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK:   BranchInst %BB1
;; CHECK: %BB1:
;; CHECK:   PhiInst (:number) 10: number, %BB0
;; CHECK:   ReturnInst
;; CHECK:   function_end

;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK:   BranchInst %BB1
;; CHECK: %BB1:
;; CHECK:   PhiInst (:number) 20: number, %BB0
;; CHECK:   ReturnInst
;; CHECK:   function_end

;; CHECK-LABEL: function wasm_func_2(p0: any): any
;; CHECK:   AllocStackInst {{.*}} $local_0
;; CHECK:   BranchInst %BB1
;; CHECK: %BB1:
;; CHECK:   PhiInst (:undefined) undefined: undefined, %BB0
;; CHECK:   ReturnInst
;; CHECK:   function_end

(module
  ;; A function table with minimum size 3.
  (table 3 funcref)

  ;; Two simple functions to put in the table.
  (func $f0 (result i32) (i32.const 10))
  (func $f1 (result i32) (i32.const 20))

  ;; Active element segment that initializes table[0..1] with $f0 and $f1.
  (elem (i32.const 0) $f0 $f1)

  ;; A function that calls indirectly.
  (func (export "call_indirect") (param i32) (result i32)
    local.get 0
    call_indirect (result i32)
  )
)
