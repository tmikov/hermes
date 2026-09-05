;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for Wasm globals: compile, run, verify via JS driver.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-globals-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; Immutable global initialized to 42.
  (global $g_imm i32 (i32.const 42))

  ;; Mutable global initialized to 100.
  (global $g_mut (mut i32) (i32.const 100))

  ;; Mutable f64 global.
  (global $g_f64 (mut f64) (f64.const 3.14))

  ;; Read immutable global.
  (func (export "get_imm") (result i32)
    global.get $g_imm)

  ;; Read mutable global.
  (func (export "get_mut") (result i32)
    global.get $g_mut)

  ;; Set mutable global.
  (func (export "set_mut") (param i32)
    local.get 0
    global.set $g_mut)

  ;; Read f64 global.
  (func (export "get_f64") (result f64)
    global.get $g_f64)

  ;; Set f64 global.
  (func (export "set_f64") (param f64)
    local.get 0
    global.set $g_f64)
)

;; CHECK: get_imm: 42
;; CHECK-NEXT: get_mut: 100
;; CHECK-NEXT: set_mut(200)
;; CHECK-NEXT: get_mut: 200
;; CHECK-NEXT: get_f64: 3.14
;; CHECK-NEXT: set_f64(6.28)
;; CHECK-NEXT: get_f64: 6.28
