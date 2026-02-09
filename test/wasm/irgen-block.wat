;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test block/end with br and br_if generate correct IR.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; Test 1: block (result i32) with br 0 → sends 42 to block's phi → function
;; exit phi → return.
;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK: %BB0:
;; CHECK:              BranchInst %BB2
;; CHECK: %BB1:
;; CHECK-NEXT:   %3 = PhiInst (:number) %5: number, %BB2
;; CHECK-NEXT:        ReturnInst %3: number
;; CHECK: %BB2:
;; CHECK-NEXT:   %5 = PhiInst (:number) 42: number, %BB0
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK:   function_end

;; Test 2: block (void) with br_if taken (condition = 1) → branches to block
;; continuation, then falls through to function exit.
;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK: %BB0:
;; CHECK:              CondBranchInst 1: number, %BB2, %BB3
;; CHECK: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK: %BB2:
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK: %BB3:
;; CHECK-NEXT:        BranchInst %BB2
;; CHECK:   function_end

;; Test 3: block (void) with br_if not taken (condition = 0), followed by
;; i32.const 99 which becomes the return value.
;; CHECK-LABEL: function wasm_func_2(): any
;; CHECK: %BB0:
;; CHECK:              CondBranchInst 0: number, %BB2, %BB3
;; CHECK: %BB1:
;; CHECK-NEXT:   %3 = PhiInst (:number) 99: number, %BB2
;; CHECK-NEXT:        ReturnInst %3: number
;; CHECK: %BB2:
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK: %BB3:
;; CHECK-NEXT:        BranchInst %BB2
;; CHECK:   function_end

;; Test 4: nested blocks — inner br 1 targets the outer block's phi with 55.
;; The inner block's continuation (i32.const 0) is unreachable dead code.
;; CHECK-LABEL: function wasm_func_3(): any
;; CHECK: %BB0:
;; CHECK:              BranchInst %BB2
;; CHECK: %BB1:
;; CHECK-NEXT:   %3 = PhiInst (:number) %5: number, %BB2
;; CHECK-NEXT:        ReturnInst %3: number
;; CHECK: %BB2:
;; CHECK-NEXT:   %5 = PhiInst (:number) 55: number, %BB0
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK:   function_end

(module
  ;; Test 1: block (result i32) with br 0 → returns 42
  (func (export "block_br_result") (result i32)
    (block (result i32)
      (i32.const 42)
      (br 0)
    )
  )

  ;; Test 2: block (void) with br_if, condition true
  (func (export "block_brif_taken")
    (block
      (i32.const 1)
      (br_if 0)
    )
  )

  ;; Test 3: block (void) with br_if not taken, then i32.const 99
  (func (export "block_brif_fallthrough") (result i32)
    (block
      (i32.const 0)
      (br_if 0)
    )
    (i32.const 99)
  )

  ;; Test 4: nested blocks with br 1 targeting outer block
  (func (export "nested_br") (result i32)
    (block (result i32)
      (block
        (i32.const 55)
        (br 1)
      )
      (i32.const 0)
    )
  )
)
