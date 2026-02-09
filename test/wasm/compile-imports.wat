;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test Wasm module with imports compiles to IR.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; Imported function placeholder.
;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK:   ReturnInst undefined
;; CHECK:   function_end

;; First defined function.
;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK:   function_end

;; Second defined function.
;; CHECK-LABEL: function wasm_func_2(p0: any): any
;; CHECK:   AllocStackInst {{.*}} $local_0
;; CHECK:   LoadParamInst
;; CHECK:   function_end

(module
  (import "env" "log" (func $log (param i32)))
  (import "env" "g" (global i32))
  (memory 1)
  (func (export "main") (result i32)
    global.get 0
  )
  (func $helper (param i32) (result i32)
    local.get 0
    i32.const 1
    i32.add
  )
)
