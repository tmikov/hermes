;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for f64/f32 nearest operations.
;; Verifies IEEE 754 round-ties-to-even (banker's rounding).

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-nearest-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (func $f64_nearest (export "f64_nearest") (param f64) (result f64)
    local.get 0
    f64.nearest
  )
  (func $f32_nearest (export "f32_nearest") (param f32) (result f32)
    local.get 0
    f32.nearest
  )
)

;; Ties-to-even: 0.5 rounds to 0 (even), not 1.
;; CHECK: f64.nearest(0.5) = 0
;; CHECK-NEXT: f64.nearest(-0.5) = 0
;; Ties-to-even: 1.5 rounds to 2 (even), not 1.
;; CHECK-NEXT: f64.nearest(1.5) = 2
;; CHECK-NEXT: f64.nearest(-1.5) = -2
;; Ties-to-even: 2.5 rounds to 2 (even), not 3.
;; CHECK-NEXT: f64.nearest(2.5) = 2
;; Non-tie cases.
;; CHECK-NEXT: f64.nearest(1.4) = 1
;; CHECK-NEXT: f64.nearest(1.6) = 2
;; CHECK-NEXT: f64.nearest(-1.4) = -1
;; CHECK-NEXT: f64.nearest(-1.6) = -2
;; f32 ties-to-even.
;; CHECK-NEXT: f32.nearest(0.5) = 0
;; CHECK-NEXT: f32.nearest(-0.5) = 0
;; CHECK-NEXT: f32.nearest(1.5) = 2
;; CHECK-NEXT: f32.nearest(2.5) = 2
