;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc -emit-binary --wasm %t.wasm 2>&1 | %FileCheck %s

;; CHECK: Wasm module parsed successfully.
;; CHECK: Types: 3
;; CHECK: Imports: 2
;; CHECK: Functions: 3 (1 imported, 2 defined)
;; CHECK: Tables: 0
;; CHECK: Memories: 1
;; CHECK: Globals: 1 (1 imported, 0 defined)
;; CHECK: Exports: 1
;; CHECK: Element segments: 0
;; CHECK: Data segments: 0
;; CHECK: Export: main (func 1)

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
