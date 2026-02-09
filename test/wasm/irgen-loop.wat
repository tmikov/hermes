;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test loop/end with br and br_if generate correct IR.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; Test 1: Simple loop that falls through immediately (no br).
;; Entry branches to loop header, header falls through to end block,
;; end block branches to function exit.
;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK: %BB0:
;; CHECK:              BranchInst %BB2
;; CHECK: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK: %BB2:
;; CHECK-NEXT:        BranchInst %BB3
;; CHECK: %BB3:
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK:   function_end

;; Test 2: Loop with unconditional br 0 (infinite loop).
;; The br 0 branches back to the loop header. The loop's end block and
;; the dead block after br are unreachable.
;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK: %BB0:
;; CHECK:              BranchInst %BB2
;; CHECK: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK: %BB2:
;; CHECK-NEXT:        BranchInst %BB2
;; CHECK:   function_end

;; Test 3: Countdown loop — decrement local until zero using br_if.
;; The CondBranchInst branches back to %BB2 (loop header) if non-zero,
;; otherwise falls through to %BB4 which goes to %BB3 (loop end) then
;; function exit.
;; CHECK-LABEL: function wasm_func_2(p0: any): any
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

;; Test 4: Loop with result type — value falls through as loop result.
;; The i32.const 99 falls through the loop end into a phi, which then
;; feeds the function exit phi.
;; CHECK-LABEL: function wasm_func_3(): any
;; CHECK: %BB0:
;; CHECK:              BranchInst %BB2
;; CHECK: %BB1:
;; CHECK-NEXT:   %3 = PhiInst (:number) %6: number, %BB3
;; CHECK-NEXT:        ReturnInst %3: number
;; CHECK: %BB2:
;; CHECK-NEXT:        BranchInst %BB3
;; CHECK: %BB3:
;; CHECK-NEXT:   %6 = PhiInst (:number) 99: number, %BB2
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK:   function_end

;; Test 5: Block containing a loop — br 1 from inside the loop exits
;; the outer block with value 77. The loop's end block and the block's
;; "i32.const 0" are unreachable dead code.
;; CHECK-LABEL: function wasm_func_4(): any
;; CHECK: %BB0:
;; CHECK:              BranchInst %BB3
;; CHECK: %BB1:
;; CHECK-NEXT:   %3 = PhiInst (:number) %5: number, %BB2
;; CHECK-NEXT:        ReturnInst %3: number
;; CHECK: %BB2:
;; CHECK-NEXT:   %5 = PhiInst (:number) 77: number, %BB3
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK: %BB3:
;; CHECK-NEXT:        BranchInst %BB2
;; CHECK:   function_end

(module
  ;; Test 1: Empty loop (falls through immediately)
  (func (export "empty_loop")
    (loop)
  )

  ;; Test 2: Infinite loop (br 0 targets loop header)
  (func (export "infinite_loop")
    (loop
      (br 0)
    )
  )

  ;; Test 3: Countdown loop
  ;; Decrements param by 1 each iteration, loops while non-zero.
  (func (export "countdown") (param i32) (result i32)
    (loop
      ;; param = param - 1
      (local.set 0
        (i32.sub (local.get 0) (i32.const 1))
      )
      ;; branch back to loop if param != 0
      (local.get 0)
      (br_if 0)
    )
    (local.get 0)
  )

  ;; Test 4: Loop with result type
  (func (export "loop_result") (result i32)
    (loop (result i32)
      (i32.const 99)
    )
  )

  ;; Test 5: Block containing a loop with br 1 to exit the block
  (func (export "loop_in_block") (result i32)
    (block (result i32)
      (loop
        (i32.const 77)
        (br 1)
      )
      (i32.const 0)
    )
  )
)
