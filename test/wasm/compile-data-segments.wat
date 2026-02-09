;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test module with memory and data segments compiles to IR.
;; The i32.load8_u opcode is not yet supported, so the function body
;; falls through to a default undefined return.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (memory (export "memory") 1)

  ;; Active data segment at offset 0.
  (data (i32.const 0) "Hello, ")

  ;; Active data segment at offset 7.
  (data (i32.const 7) "World!")

  ;; A function that loads a byte from memory.
  (func (export "load") (param i32) (result i32)
    local.get 0
    i32.load8_u
  )
;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT: %[[P0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:              StoreStackInst %[[P0]]: any, %[[L0]]: any
;; CHECK:                   BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %{{.*}} = PhiInst (:undefined) undefined: undefined, %BB0
;; CHECK-NEXT:           ReturnInst
;; CHECK-NEXT: function_end
)
