;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for import trampolines with float types (I.2).
;; Tests f64 argument passing and return values through imports.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-import-float-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; Import f64 addition.
  (import "env" "f64_add" (func $f64_add (param f64 f64) (result f64)))

  ;; Import comparison returning i32.
  (import "env" "f64_gt" (func $f64_gt (param f64 f64) (result i32)))

  ;; Export: test f64 import.
  (func (export "test_f64") (param f64 f64) (result f64)
    local.get 0
    local.get 1
    call $f64_add)

  ;; Export: test comparison import.
  (func (export "test_gt") (param f64 f64) (result i32)
    local.get 0
    local.get 1
    call $f64_gt)
)

;; CHECK: f64_result(1.5, 2.25) = 3.75
;; CHECK-NEXT: gt(3.0, 2.0) = 1
;; CHECK-NEXT: gt(1.0, 5.0) = 0
