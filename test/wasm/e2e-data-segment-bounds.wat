;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for data segment bounds checking at instantiation.
;; Tests that GlobalGet offsets work correctly and that OOB data segments trap.

;; REQUIRES: wasm
;; RUN: %wat2wasm --enable-extended-const %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-data-segment-bounds-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "env" "g" (global i32))
  (memory (export "memory") 1)

  ;; Active data segment using global.get for offset.
  ;; With global 0 initialized to 0, "Hi" is written at bytes 0 and 1.
  (data (global.get 0) "Hi")

  ;; A second active data segment with a constant offset, to verify
  ;; both kinds coexist correctly.
  (data (i32.const 10) "OK")

  ;; Extended const expression: (i32.add (i32.const 10) (i32.const 10))
  ;; places "EX" at byte 20.
  (data (i32.add (i32.const 10) (i32.const 10)) "EX")

  (func (export "get_byte") (param i32) (result i32)
    local.get 0
    i32.load8_u
  )
)

;; CHECK: byte 0: 72
;; CHECK-NEXT: byte 1: 105
;; CHECK-NEXT: byte 10: 79
;; CHECK-NEXT: byte 11: 75
;; CHECK-NEXT: byte 20: 69
;; CHECK-NEXT: byte 21: 88
;; CHECK-NEXT: done
