;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test module with mixed imports (functions, table, memory, global)
;; from different modules, and a start function.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc -emit-binary --wasm %t.wasm 2>&1 | %FileCheck %s

;; CHECK: Wasm module parsed successfully.
;; CHECK: Types: 4
;; CHECK: Imports: 4
;; CHECK: Functions: 5 (2 imported, 3 defined)
;; CHECK: Tables: 1
;; CHECK: Memories: 1
;; CHECK: Globals: 1 (1 imported, 0 defined)
;; CHECK: Exports: 2
;; CHECK: Start function: 2
;; CHECK: Element segments: 0
;; CHECK: Data segments: 0
;; CHECK: Export: run (func 3)
;; CHECK: Export: helper (func 4)

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
