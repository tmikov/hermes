;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for import trampoline functions (I.2).
;; Tests calling imported JS functions from Wasm code.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-import-trampolines-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; Import a void function that logs its argument.
  (import "env" "log" (func $log (param i32)))

  ;; Import a function that returns i32.
  (import "env" "add" (func $add (param i32 i32) (result i32)))

  ;; Import a function that throws.
  (import "env" "throwing" (func $throwing))

  ;; Export: call $log with the given argument.
  (func (export "call_log") (param i32)
    local.get 0
    call $log)

  ;; Export: call $add with two arguments and return the result.
  (func (export "call_add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    call $add)

  ;; Export: call the throwing function.
  (func (export "call_throw")
    call $throwing)
)

;; CHECK: log: 42
;; CHECK-NEXT: log: -7
;; CHECK-NEXT: add(3, 4) = 7
;; CHECK-NEXT: add(100, 200) = 300
;; CHECK-NEXT: caught: import threw
