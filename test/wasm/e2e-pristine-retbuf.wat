;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The numeric multi-value return buffer is built from pristine constructors
;; taken from HermesInternal.intrinsics, not from globalThis, so replacing
;; ArrayBuffer / Uint32Array / Float64Array cannot reach it.
;;
;; What this used to allow was not merely a wrong number. The f32/f64 arm of
;; the result load pushes the loaded value with no coercion, the value is then
;; passed as an argument annotated `number` by wasmValTypeToIRType, asNumber
;; short-circuits on isNumberType(), and the interpreter reads it with
;; getNumber() -> getDouble(). A replaced Float64Array could therefore put a
;; string where the engine had already decided a double was.
;;
;; The timing is load-bearing: the buffer is built during instantiation and
;; kept in the module's frame, so the driver replaces the globals BEFORE
;; instantiating. A replacement installed afterwards would never be reached
;; and the test would pass for the wrong reason.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-pristine-retbuf-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (func (export "twoF64") (result f64 f64)
    (f64.const 1.5)
    (f64.const 2.25))
  (func (export "twoI32") (result i32 i32)
    (i32.const 11)
    (i32.const 22))
  ;; 0.1 is not representable in f32, so the value that reaches the buffer is
  ;; fround(0.1) and the value a reader must get back is fround(0.1), never
  ;; the f64 0.1 the literal was written as.
  (func (export "twoF32") (result f32 f32)
    (f32.const 0.25)
    (f32.const 0.1)))

;; The replacements really do replace: built directly, they hand back the
;; poisoned storage. Without this the rest of the file would pass just as well
;; against a driver whose replacement did nothing.
;; CHECK: replacement is live: true

;; And the module never called them -- it allocated from the pristine copies.
;; CHECK-NEXT: module called the replaced constructors: 0

;; typeof AND a strict comparison. A stringified assertion here is satisfied
;; by a number, a string and a boxed Number alike, which is how a defect of
;; exactly this shape survived review four times on this branch.
;; CHECK-NEXT: f64 result 1: typeof number, === 2.25: true
;; CHECK-NEXT: i32 result 1: typeof number, === 22: true

;; An f32 result must come back at f32 precision. F32 and F64 share this arm
;; of the result load and both go through the Float64Array, and nothing on the
;; path rounds -- which was fine only because every producer of an f32 value
;; rounds already. A replaced Float64Array was the one way to get an unrounded
;; double into an f32 slot: measured against the old, interceptable buffer,
;; this same module answered 0.1 where f32 can only hold 0.10000000149011612.
;; That is the rounding half of 01a0850f-ac3e and this pins it.
;; CHECK-NEXT: f32 result 1: === fround(0.1): true, !== 0.1: true
;; CHECK-NEXT: done
