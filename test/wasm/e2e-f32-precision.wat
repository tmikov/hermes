;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for f32 precision: verifies that f32 arithmetic produces
;; f32-precision results (via Math.fround), not f64-precision.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm
;; RUN: %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/load-hbc.js_ -- %t.hbc test_add_precision | %FileCheck --match-full-lines --check-prefix=ADD %s
;; RUN: %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/load-hbc.js_ -- %t.hbc test_demote_precision | %FileCheck --match-full-lines --check-prefix=DEMOTE %s

;; In f32: 1.0f + 2^-24 = 1.0f (the added bit is at the ULP boundary, rounds down).
;; In f64: 1.0  + 2^-24 = 1.000000059604644775390625 (exact, no rounding).
;; The test returns 1 if the result equals 1.0 (f32 precision), 0 otherwise.

;; ADD: 1
(module
  (func (export "test_add_precision") (result i32)
    f32.const 1.0
    f32.const 0x1p-24       ;; 2^-24 = ULP of 1.0 in f32
    f32.add
    f32.const 1.0
    f32.eq)

  ;; f32.demote_f64 of a value that is slightly above 1.0 in f64 but rounds
  ;; to 1.0 in f32. Returns 1 if demoted value equals 1.0.
  ;; DEMOTE: 1
  (func (export "test_demote_precision") (result i32)
    f64.const 1.000000059604644775390625  ;; 1.0 + 2^-24 in f64
    f32.demote_f64
    f32.const 1.0
    f32.eq)
)
