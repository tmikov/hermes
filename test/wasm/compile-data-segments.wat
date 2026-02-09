;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test module with memory and data segments compiles to IR.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK:   AllocStackInst {{.*}} $local_0
;; CHECK:   LoadParamInst
;; CHECK:   function_end

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
)
