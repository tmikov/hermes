;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for f64/f32 copysign operations.
;; Compiles to .hbc and runs via hermescli.loadHBC to verify results.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-copysign-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (func $f64_copysign (export "f64_copysign") (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.copysign
  )
  (func $f32_copysign (export "f32_copysign") (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.copysign
  )
)

;; CHECK: f64.copysign(1, -1) = -1
;; CHECK-NEXT: f64.copysign(-1, 1) = 1
;; CHECK-NEXT: f64.copysign(5.5, -3) = -5.5
;; CHECK-NEXT: f32.copysign(1, -1) = -1
;; CHECK-NEXT: f32.copysign(-1, 1) = 1
;; CHECK-NEXT: f32.copysign(5.5, -3) = -5.5
