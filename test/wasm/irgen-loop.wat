;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test loop/end with br and br_if generate correct IR.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Test 1: Empty loop (falls through immediately)
  ;; Entry branches to loop header, header falls through to end block,
  ;; end block branches to function exit.
  (func (export "empty_loop")
    (loop))

;; CHECK-LABEL: function wasm_func_0(): undefined 
;; CHECK: %BB0:
;; CHECK:              BranchInst %BB2
;; CHECK: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK: %BB2:
;; CHECK-NEXT:        BranchInst %BB3
;; CHECK: %BB3:
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK:   function_end

  ;; Test 2: Infinite loop (br 0 targets loop header)
  ;; The br 0 branches back to the loop header. The loop's end block and
  ;; the dead block after br are unreachable.
  (func (export "infinite_loop")
    (loop
      (br 0)))

;; CHECK-LABEL: function wasm_func_1(): undefined 
;; CHECK: %BB0:
;; CHECK:              BranchInst %BB2
;; CHECK: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK: %BB2:
;; CHECK-NEXT:        BranchInst %BB2
;; CHECK:   function_end

  ;; Test 3: Countdown loop
  ;; Decrements param by 1 each iteration, loops while non-zero.
  (func (export "countdown") (param i32) (result i32)
    (loop
      ;; param = param - 1
      (local.set 0
        (i32.sub (local.get 0) (i32.const 1)))
      ;; branch back to loop if param != 0
      (local.get 0)
      (br_if 0))
    (local.get 0))

;; CHECK-LABEL: function wasm_func_2(p0: number): number 
;; CHECK: %BB0:
;; CHECK:        BranchInst %BB2
;; CHECK: %BB2:
;; CHECK:        StoreStackInst
;; CHECK:        CondBranchInst {{.*}}, %BB2, %BB4
;; CHECK: %BB3:
;; CHECK:        BranchInst %BB1
;; CHECK: %BB4:
;; CHECK-NEXT:        BranchInst %BB3
;; CHECK:   function_end

  ;; Test 4: Loop with result type - value falls through as loop result.
  ;; The i32.const 99 falls through the loop end into a phi, which then
  ;; feeds the function exit phi.
  (func (export "loop_result") (result i32)
    (loop (result i32)
      (i32.const 99)))

;; CHECK-LABEL: function wasm_func_3(): number 
;; CHECK: %BB0:
;; CHECK:              BranchInst %BB2
;; CHECK: %BB1:
;; CHECK-NEXT:   %[[EXIT_PHI:.*]] = PhiInst (:number) %[[LOOP_PHI:.*]]: number, %BB3
;; CHECK-NEXT:                      ReturnInst %[[EXIT_PHI]]: number
;; CHECK: %BB2:
;; CHECK-NEXT:        BranchInst %BB3
;; CHECK: %BB3:
;; CHECK-NEXT:   %[[LOOP_PHI]] = PhiInst (:number) 99: number, %BB2
;; CHECK-NEXT:                   BranchInst %BB1
;; CHECK:   function_end

  ;; Test 5: Block containing a loop - br 1 from inside the loop exits
  ;; the outer block with value 77. The loop's end block and the block's
  ;; "i32.const 0" are unreachable dead code.
  (func (export "loop_in_block") (result i32)
    (block (result i32)
      (loop
        (i32.const 77)
        (br 1))
      (i32.const 0))))

;; CHECK-LABEL: function wasm_func_4(): number 
;; CHECK: %BB0:
;; CHECK:              BranchInst %BB3
;; CHECK: %BB1:
;; CHECK-NEXT:   %[[EXIT_PHI:.*]] = PhiInst (:number) %[[BLOCK_PHI:.*]]: number, %BB2
;; CHECK-NEXT:                      ReturnInst %[[EXIT_PHI]]: number
;; CHECK: %BB2:
;; CHECK-NEXT:   %[[BLOCK_PHI]] = PhiInst (:number) 77: number, %BB3
;; CHECK-NEXT:                    BranchInst %BB1
;; CHECK: %BB3:
;; CHECK-NEXT:        BranchInst %BB2
;; CHECK:   function_end
