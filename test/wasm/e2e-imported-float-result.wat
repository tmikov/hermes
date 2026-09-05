;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; An imported function's result is whatever the JS callee returns, so it is
;; typed :any. The trampoline coerced an i32 result with AsInt32Inst but
;; returned f32/f64 results "as-is (JS Numbers are doubles)". Once float
;; arithmetic started using FBinaryMathInst/FCompareInst -- which the verifier
;; type-checks -- that untyped value reached them and perfectly valid Wasm
;; stopped compiling:
;;
;;   FAddInst: FBinaryMathInst wrong type in function "wasm_export_add1"
;;   error: Lowered IR verification failed
;;
;; i32 imports were unaffected, which is why this went unnoticed. The result is
;; now converted with ToNumber (and fround for f32), so the module compiles and
;; a callee returning a non-number gets JS conversion semantics rather than a
;; type-confused value.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-imported-float-result-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "f64f" (func $f64f (result f64)))
  (import "e" "f32f" (func $f32f (result f32)))
  (import "e" "i32f" (func $i32f (result i32)))

  (func (export "f64_add") (result f64) f64.const 1 call $f64f f64.add)
  (func (export "f64_lt") (result i32) call $f64f f64.const 10 f64.lt)
  (func (export "f32_add") (result f32) f32.const 1 call $f32f f32.add)
  (func (export "i32_add") (result i32) i32.const 1 call $i32f i32.add)
  ;; Returns the imported f32 result with no arithmetic after it, so the only
  ;; rounding is the trampoline's.
  (func (export "f32_id") (result f32) call $f32f))

;; A number behaves normally.
;; CHECK: number 2.5: f64_add=3.5 f64_lt=1 f32_add=3.5 i32_add=3

;; Everything else gets ToNumber, not a reinterpreted value. Strings convert,
;; objects become NaN, and BigInt throws just as ToNumber requires.
;; CHECK-NEXT: string '3': f64_add=4 f64_lt=1 f32_add=4 i32_add=4
;; CHECK-NEXT: object {}: f64_add=NaN f64_lt=0 f32_add=NaN i32_add=1
;; CHECK-NEXT: valueOf 7: f64_add=8 f64_lt=1 f32_add=8 i32_add=8
;; CHECK-NEXT: bigint 5n: TypeError

;; The trampoline must round an f32 result to single precision. f32_id has no
;; arithmetic after the call, so this isolates that rounding: an unrounded
;; pass-through would print 0.2 exactly.
;; CHECK-NEXT: f32 trampoline rounding: 0.20000000298023224
;; CHECK-NEXT: done
