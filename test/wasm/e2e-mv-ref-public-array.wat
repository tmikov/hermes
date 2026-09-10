;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The PUBLIC result array of a multi-value export, reached by a direct JS
;; call. No second module is involved.
;;
;; The wrapper used to call globalThis.Array AFTER the internal call and then
;; interleave buffer reads with ordinary indexed stores into whatever that
;; constructor returned. Two things followed, and the driver tests them apart:
;; a replaced constructor decided what the export returned, and -- even with a
;; genuine constructor -- `new Array(n)` produces HOLES, so each indexed store
;; consulted the prototype chain and an accessor installed on Array.prototype
;; saw each element on its way in.
;;
;; The array is now built by wasmMakeResultArray from values already read, so
;; neither route is taken. The driver shows the inherited setter firing for an
;; ordinary array first, so a setter that never fires proves something.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-mv-ref-public-array-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (table 1 funcref)
  (elem (i32.const 0) $f)
  (func $f (result i32) (i32.const 7))

  ;; Single funcref result: no buffer, no result array. The identity oracle.
  (func (export "svGet") (result funcref)
    (table.get (i32.const 0)))

  ;; (i32, funcref): i32 -> integer slot 0, funcref -> reference slot 1.
  (func (export "pair") (result i32 funcref)
    (i32.const 42)
    (table.get (i32.const 0)))

  ;; (externref, i32): the reference is first, so it lands in reference slot 0.
  (func (export "epair") (param externref) (result externref i32)
    (local.get 0)
    (i32.const 3))

  ;; A nested call whose callee lays out MORE bytes than the caller's own
  ;; signature does. $wide is (f64 f64 externref): 8 + 8 + 4 = 20 bytes, so its
  ;; reference sits at slot index 4. `nested` is (i32 externref): 8 bytes, 2
  ;; slots. A caller that forwarded its own incoming container to the nested
  ;; call instead of sizing one for the callee would write slot 4 of a 2-slot
  ;; container, which wasmRefBufSet refuses. Extra NUMERIC results alone would
  ;; not show this: a callee can lay out more bytes and still put its reference
  ;; at an index the caller's container already has.
  (func $wide (param externref) (result f64 f64 externref)
    (f64.const 1) (f64.const 2) (local.get 0))
  (func (export "nested") (param externref) (result i32 externref)
    (local $r externref)
    (local.get 0)
    (call $wide)
    (local.set $r)
    (drop) (drop)
    (i32.const 5)
    (local.get $r)))

;; The inherited indexed setter is real: it fires for an ordinary array with a
;; hole at index 0. Everything below rests on this line.
;; CHECK: control: ordinary array hit the inherited setter: true
;; CHECK-NEXT: control: ordinary array hole read the inherited getter: true

;; pair(): a genuine Array whose elements are own data properties, holding the
;; real i32 and the real Exported Function.
;; CHECK-NEXT: pair: isArray=true length=2
;; CHECK-NEXT: pair[0]: 42
;; CHECK-NEXT: pair[1]: function calls -> 7 same=true
;; CHECK-NEXT: pair: own data elements: true true

;; epair(): the same, with the reference in slot 0 and the host object
;; delivered unchanged.
;; CHECK-NEXT: epair: isArray=true length=2
;; CHECK-NEXT: epair[0] is the host object: true
;; CHECK-NEXT: epair[1]: 3
;; CHECK-NEXT: epair: own data elements: true true

;; The nested call sized its container for the callee.
;; CHECK-NEXT: nested: [5, host object]: true true

;; Neither interception route was taken by any of the three calls.
;; CHECK-NEXT: replacement Array constructor calls: 0
;; CHECK-NEXT: inherited setter hits during the exports: 0
;; CHECK-NEXT: inherited getter hits during the exports: 0
;; CHECK-NEXT: done
