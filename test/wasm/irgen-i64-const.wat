;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: i64 constant split into lo32/hi32 pair.
;; Phase 1 represents i64 as two values on the stack (lo, hi).
;; i64 function results use the hi-stash pattern: stash hi, return lo.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; i64.const 0x0000000100000002 = 4294967298
  ;; Split: lo32 = 2, hi32 = 1
  (func (result i64)
    i64.const 4294967298))

;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK-NEXT: %BB0:
;; CHECK:              BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %[[LO:.*]] = PhiInst (:number) 2: number, %BB0
;; CHECK-NEXT:   %[[HI:.*]] = PhiInst (:number) 1: number, %BB0
;; CHECK-NEXT:   %{{.*}} = CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiStash]
;; CHECK:                    ReturnInst %[[LO]]: number
;; CHECK-NEXT: function_end
