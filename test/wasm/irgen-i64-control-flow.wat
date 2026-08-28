;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: i64 block results and if/else with i64 (G.5).
;; Each i64 result type produces 2 PhiInst nodes (lo, hi).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Block returning i64: 2 phis in continuation block
  (func $block_i64 (result i64)
    (block (result i64)
      i64.const 100))

  ;; If/else returning i64: 2 phis in merge block
  (func $if_i64 (param i32) (result i64)
    (if (result i64) (local.get 0)
      (then (i64.const 1))
      (else (i64.const 2)))))

;; -- block_i64: inner block's continuation has 2 phis for i64 result --
;; CHECK-LABEL: function wasm_func_0(): any
;; Exit block (BB1) was created first by beginFunction:
;; CHECK:      %BB1:
;; CHECK-NEXT:   %{{.*}} = PhiInst
;; CHECK-NEXT:   %{{.*}} = PhiInst
;; CHECK-NEXT:   %{{.*}} = CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiStash]
;; CHECK:                  ReturnInst
;; Block continuation (BB2) has 2 phis for i64 result:
;; CHECK-NEXT: %BB2:
;; CHECK-NEXT:   %{{.*}} = PhiInst (:number) 100: number, %BB0
;; CHECK-NEXT:   %{{.*}} = PhiInst (:number) 0: number, %BB0
;; CHECK-NEXT:            BranchInst %BB1
;; CHECK-NEXT: function_end

;; -- if_i64: merge block has 2 phis for i64 result --
;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; CHECK:        CondBranchInst %{{.*}}: any, %BB2, %BB3
;; Exit block (BB1):
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %{{.*}} = PhiInst
;; CHECK-NEXT:   %{{.*}} = PhiInst
;; CHECK-NEXT:   %{{.*}} = CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiStash]
;; The merge block (BB4) has 2 phis with entries from both arms:
;; CHECK:      %BB4:
;; CHECK-NEXT:   %{{.*}} = PhiInst (:number) 1: number, %BB2, 2: number, %BB3
;; CHECK-NEXT:   %{{.*}} = PhiInst (:number) 0: number, %BB2, 0: number, %BB3
;; CHECK-NEXT:            BranchInst %BB1
;; CHECK-NEXT: function_end
