;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test that the name section is parsed and function names appear.

;; REQUIRES: wasm
;; RUN: %wat2wasm --debug-names %s -o %t.wasm
;; RUN: %hermesc -emit-binary --wasm %t.wasm 2>&1 | %FileCheck %s

;; CHECK: Wasm module parsed successfully.
;; CHECK: Types: 2
;; CHECK: Imports: 0
;; CHECK: Functions: 2 (0 imported, 2 defined)
;; CHECK: Tables: 0
;; CHECK: Memories: 0
;; CHECK: Globals: 0 (0 imported, 0 defined)
;; CHECK: Exports: 2
;; CHECK: Element segments: 0
;; CHECK: Data segments: 0
;; CHECK: Export: add (func 0)
;; CHECK: Export: negate (func 1)
;; CHECK: Function 0 name: add
;; CHECK: Function 1 name: negate

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
