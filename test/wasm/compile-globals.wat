;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test module with mutable and immutable globals, various init expressions.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc -emit-binary --wasm %t.wasm 2>&1 | %FileCheck %s

;; CHECK: Wasm module parsed successfully.
;; CHECK: Types: 1
;; CHECK: Imports: 0
;; CHECK: Functions: 1 (0 imported, 1 defined)
;; CHECK: Tables: 0
;; CHECK: Memories: 0
;; CHECK: Globals: 3 (0 imported, 3 defined)
;; CHECK: Exports: 3
;; CHECK: Element segments: 0
;; CHECK: Data segments: 0
;; CHECK: Export: g_const (global 0)
;; CHECK: Export: g_mut (global 1)
;; CHECK: Export: get_g_mut (func 0)

(module
  ;; Immutable i32 global with i32.const init.
  (global $g_const (export "g_const") i32 (i32.const 42))

  ;; Mutable i32 global with i32.const init.
  (global $g_mut (export "g_mut") (mut i32) (i32.const 100))

  ;; Mutable f64 global initialized via f64.const.
  (global $g_f64 (mut f64) (f64.const 3.14))

  ;; A function that reads the mutable global.
  (func (export "get_g_mut") (result i32)
    global.get 1
  )
)
