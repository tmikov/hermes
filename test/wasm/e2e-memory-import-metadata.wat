;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; __wasm_min__ and __wasm_max__ are ordinary properties on a script-supplied
;; object, so a getter or Proxy can answer differently on each read.
;; __wasm_max__ was loaded once to validate and a second time to store, so a
;; getter could pass validation and then raise the ceiling, letting
;; memory.grow exceed the declared maximum. And with no declared maximum
;; nothing validated it at all, yet it still reached a native builtin that
;; calls getNumber() on it, asserting on a non-number. Each property is now
;; read once, in a block dominating both paths, and coerced to a number.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %wat2wasm %S/e2e-memory-import-metadata-nomax.wat_ -o %t-nomax.wasm && %hermesc --wasm -emit-binary -out %t-nomax.hbc %t-nomax.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-memory-import-metadata-driver.js_ -- %t.hbc %t-nomax.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "m" (memory 1 2))
  (func (export "grow") (result i32) (memory.grow (i32.const 100)))
  (func (export "grow1") (result i32) (memory.grow (i32.const 1))))

;; Growing past the declared maximum of 2 fails even when the getter tries to
;; raise it on a later read, and the property is read exactly once.
;; CHECK: grow past max = -1, __wasm_max__ reads = 1

;; With no declared maximum, a non-number __wasm_max__ must not reach
;; getNumber(). Coerced to NaN it yields a maximum of 0, so grow fails closed.
;; CHECK-NEXT: non-number max: grow = -1

;; A grow that is within the declared maximum must still succeed, returning
;; the previous size -- otherwise the -1 results above could be produced by a
;; blanket failure rather than by the checks under test.
;; CHECK-NEXT: grow within max = 1

;; And the memory that grew is the embedder's own object, not a copy of it.
;; CHECK-NEXT: imported object followed grow: true
;; CHECK-NEXT: done
