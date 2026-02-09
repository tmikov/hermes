;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test module with a table and element segments.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc -emit-binary --wasm %t.wasm 2>&1 | %FileCheck %s

;; CHECK: Wasm module parsed successfully.
;; CHECK: Types: 2
;; CHECK: Imports: 0
;; CHECK: Functions: 3 (0 imported, 3 defined)
;; CHECK: Tables: 1
;; CHECK: Memories: 0
;; CHECK: Globals: 0 (0 imported, 0 defined)
;; CHECK: Exports: 1
;; CHECK: Element segments: 1
;; CHECK: Data segments: 0
;; CHECK: Export: call_indirect (func 2)

(module
  ;; A function table with minimum size 3.
  (table 3 funcref)

  ;; Two simple functions to put in the table.
  (func $f0 (result i32) (i32.const 10))
  (func $f1 (result i32) (i32.const 20))

  ;; Active element segment that initializes table[0..1] with $f0 and $f1.
  (elem (i32.const 0) $f0 $f1)

  ;; A function that calls indirectly.
  (func (export "call_indirect") (param i32) (result i32)
    local.get 0
    call_indirect (result i32)
  )
)
