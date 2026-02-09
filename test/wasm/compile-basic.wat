;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test basic Wasm module compilation to Hermes IR.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK:   AllocStackInst {{.*}} $local_0
;; CHECK:   LoadParamInst
;; CHECK:   AllocStackInst {{.*}} $local_1
;; CHECK:   LoadParamInst
;; CHECK:   function_end

(module
  (memory (export "memory") 1)
  (func (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
  )
)
