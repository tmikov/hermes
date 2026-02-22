;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: i64 drop correctly consumes both halves (lo32 + hi32).
;; Phase 1 represents i64 as two stack slots.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Push i32 42, push i64, drop the i64 (should consume both halves),
  ;; return the i32 42.
  (func (result i32)
    i32.const 42
    i64.const 100
    drop
    ;; Stack should now have just the i32 42.
  ))

;; The i64.const pushes 2 values (lo=100, hi=0). drop should consume both.
;; The function should return the i32 42 via the phi node.
;; CHECK-LABEL: function wasm_func_0(): number 
;; CHECK:              BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %[[PHI:.*]] = PhiInst (:number) 42: number, %BB0
;; CHECK-NEXT:                 ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
