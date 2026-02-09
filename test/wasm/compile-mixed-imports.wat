;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test module with mixed imports (functions, table, memory, global)
;; from different modules, and a start function.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir %t.wasm | %FileCheck %s

;; Imported function placeholders.
;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK:   ReturnInst undefined
;; CHECK:   function_end

;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; CHECK:   ReturnInst undefined
;; CHECK:   function_end

;; Defined functions.
;; CHECK-LABEL: function wasm_func_2(): any
;; CHECK:   function_end

;; CHECK-LABEL: function wasm_func_3(): any
;; CHECK:   function_end

;; CHECK-LABEL: function wasm_func_4(p0: any): any
;; CHECK:   AllocStackInst {{.*}} $local_0
;; CHECK:   LoadParamInst
;; CHECK:   function_end

(module
  ;; Import a function from "env".
  (import "env" "log" (func $log (param i32)))

  ;; Import a global from "config".
  (import "config" "max_size" (global $max_size i32))

  ;; Import a memory from "env".
  (import "env" "memory" (memory 1 10))

  ;; Import a function from a different module "math".
  (import "math" "square" (func $square (param i32) (result i32)))

  ;; Table declared in this module (not imported).
  (table 4 funcref)

  ;; The start function is the first defined function (func index 2).
  (func $init
    i32.const 0
    call $log
  )
  (start $init)

  ;; Exported functions.
  (func (export "run") (result i32)
    global.get $max_size
  )
  (func (export "helper") (param i32) (result i32)
    local.get 0
    call $square
  )
)
