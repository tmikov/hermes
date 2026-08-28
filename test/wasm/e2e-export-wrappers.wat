;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for export wrapper functions (I.1).
;; Tests argument coercion, void functions, and f64 return values.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-export-wrappers-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (memory 1)

  ;; Test 1: i32 add — basic export wrapper.
  (func $add (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)

  ;; Test 2: void function (no result).
  ;; Stores a value in memory so the caller can verify it ran.
  (func $set_mem (export "set_mem") (param i32)
    i32.const 0
    local.get 0
    i32.store)

  ;; Helper to read memory for verification.
  (func $get_mem (export "get_mem") (result i32)
    i32.const 0
    i32.load)

  ;; Test 3: f64 return.
  (func $f64_add (export "f64_add") (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.add)

  ;; Test 4: f32 return.
  (func $f32_mul (export "f32_mul") (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.mul)

  ;; Test 5: mixed types.
  (func $mixed (export "mixed") (param i32 f64) (result f64)
    local.get 0
    f64.convert_i32_s
    local.get 1
    f64.add)
)

;; CHECK: add(3, 4) = 7
;; CHECK-NEXT: add(2147483647, 1) = -2147483648
;; CHECK-NEXT: set_mem returned: undefined
;; CHECK-NEXT: get_mem after set_mem(42) = 42
;; CHECK-NEXT: f64_add(1.5, 2.25) = 3.75
;; CHECK-NEXT: f32_mul(3, 4) = 12
;; CHECK-NEXT: mixed(10, 3.5) = 13.5
