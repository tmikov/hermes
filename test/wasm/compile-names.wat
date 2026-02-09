;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test Wasm module with named functions compiles to IR.
;; Note: The name section is parsed after the code section in the Wasm binary,
;; so function names are not yet applied to IR functions. This will be
;; improved in a future step. For now, functions use wasm_func_N names.

;; REQUIRES: wasm
;; RUN: %wat2wasm --debug-names %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir %t.wasm | %FileCheck %s

;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK:   function_end

;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; CHECK:   function_end

(module
  (func $add (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
  )
  (func $negate (export "negate") (param i32) (result i32)
    i32.const 0
    local.get 0
    i32.sub
  )
)
