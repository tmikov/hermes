;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A Wasm function's internal closure has statically-typed params, and the
;; float backend trusts an f32/f64 param is a number and reads its raw double
;; bits. That is sound for calls through the export wrapper (which coerces)
;; and for Wasm-to-Wasm calls, but an element segment places the closure
;; itself in a table, which JS can read via WebAssembly.Table.prototype.get()
;; and call with any argument -- reaching the float backend with a non-number
;; and crashing (Debug assert / Release SIGSEGV). This is finding J4.
;;
;; Such an "escapable" function now coerces its f32/f64 params on entry
;; (ToNumber, plus fround for f32), so a bad argument becomes NaN with
;; ordinary JS semantics instead of crashing. A function whose closure cannot
;; reach JS is unaffected and keeps trusting its params -- see the many
;; irgen-f*/e2e-f* golden tests, whose signatures stay (p: number).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>/dev/null && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-escapable-float-param-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (table (export "tbl") 2 funcref)
  (func $addf64 (param f64 f64) (result f64) (f64.add (local.get 0) (local.get 1)))
  (func $addf32 (param f32 f32) (result f32) (f32.add (local.get 0) (local.get 1)))
  (elem (i32.const 0) $addf64 $addf32)
  ;; A direct call keeps the fast path: this exported wrapper drives $addf64
  ;; Wasm-to-Wasm, and must still add correctly.
  (func (export "add_direct") (param f64 f64) (result f64)
    (call $addf64 (local.get 0) (local.get 1))))

;; The internal closures are reachable from JS via the table.
;; A non-number argument is coerced (ToNumber -> NaN), not read as raw bits.
;; CHECK: f64 closure(2.5, 4.0) = 6.5
;; CHECK-NEXT: f64 closure("x", "y") = NaN
;; CHECK-NEXT: f32 closure(1.5, 2.25) = 3.75
;; CHECK-NEXT: f32 closure({}, 1) = NaN

;; The normal Wasm-to-Wasm path is unchanged.
;; CHECK-NEXT: add_direct(10.5, 0.25) = 10.75
;; CHECK-NEXT: done
