;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for call_indirect instruction (J.2).
;; Tests: correct dispatch, out-of-bounds, uninitialized entry, type mismatch.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-call-indirect-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (type $void_to_i32 (func (result i32)))
  (type $i32_to_i32 (func (param i32) (result i32)))

  (table 5 funcref)

  ;; Place f0 (type 0) at index 0, f1 (type 0) at index 1,
  ;; f2 (type 1) at index 3. Indices 2 and 4 are uninitialized.
  (elem (i32.const 0) $f0 $f1)
  (elem (i32.const 3) $f2)

  ;; type 0: () -> i32
  (func $f0 (result i32)
    i32.const 10)

  (func $f1 (result i32)
    i32.const 20)

  ;; type 1: (i32) -> i32
  (func $f2 (param i32) (result i32)
    local.get 0
    i32.const 1
    i32.add)

  ;; Call with type $void_to_i32 (type 0).
  (func $call_type0 (export "call_type0") (param i32) (result i32)
    local.get 0
    call_indirect (type $void_to_i32))

  ;; Call with type $i32_to_i32 (type 1), passing an argument.
  (func $call_type1 (export "call_type1") (param i32 i32) (result i32)
    local.get 1
    local.get 0
    call_indirect (type $i32_to_i32))
)

;; CHECK: call f0: 10
;; CHECK-NEXT: call f1: 20
;; CHECK-NEXT: call f2(5): 6
;; CHECK-NEXT: oob: call_indirect: undefined element
;; CHECK-NEXT: null entry: call_indirect: uninitialized element
;; CHECK-NEXT: type mismatch: call_indirect: type mismatch
