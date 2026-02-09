;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test module with globals compiles to IR without errors.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir %t.wasm | %FileCheck %s

;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK:   function_end

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
