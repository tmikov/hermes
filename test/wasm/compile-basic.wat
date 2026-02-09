;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc -emit-binary --wasm %t.wasm 2>&1 | %FileCheck %s

;; CHECK: Wasm module parsed successfully.
;; CHECK: Types: 1
;; CHECK: Imports: 0
;; CHECK: Functions: 1 (0 imported, 1 defined)
;; CHECK: Tables: 0
;; CHECK: Memories: 1
;; CHECK: Globals: 0 (0 imported, 0 defined)
;; CHECK: Exports: 2
;; CHECK: Element segments: 0
;; CHECK: Data segments: 0
;; CHECK: Export: memory (memory 0)
;; CHECK: Export: add (func 0)

(module
  (memory (export "memory") 1)
  (func (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
  )
)
