;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test module with memory and data segments compiles to IR.
;; The i32.load8_u opcode now produces a LoadPropertyInst on the HEAPU8 view.

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
;; CHECK-LABEL: function wasm_func_0(p0: number): number 
;; CHECK: LoadFrameInst {{.*}}[%VS0.HEAPU8]
;; CHECK: LoadPropertyInst
;; CHECK: BinaryStrictlyEqualInst
;; CHECK: CondBranchInst
)
