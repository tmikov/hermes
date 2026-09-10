;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Reentrancy against the reference transport, in the two shapes that reach it.
;;
;; SHAPE 1 -- the export wrapper's result array. The wrapper used to call
;; globalThis.Array between the internal call and the buffer reads, so a
;; replacement constructor got control at exactly the moment one activation's
;; references were sitting unread in a per-module array. Re-entering an export
;; from there ran a second activation, which wrote the same array, and the
;; outer wrapper then read the inner activation's values. The wrapper no longer
;; calls the constructor at all, so the driver's count for it is expected to be
;; ZERO; what the outer/inner identities buy is that a count of zero is not the
;; only thing standing between this test and a false pass.
;;
;; SHAPE 2 -- the numeric buffer's accessors. Those buffers are still built from
;; globalThis.Uint32Array and globalThis.Float64Array and are still read and
;; written with ordinary property operations, so a replacement installed BEFORE
;; instantiation gets control between two reference stores and again between
;; two reference loads: `(result externref i32 externref)` lays the i32 out
;; between them. Per-activation containers mean the reentrant call writes
;; different storage, so both externrefs survive. The i32 between them does NOT
;; -- see the driver.
;;
;; The reference-load gap is also where the driver forces a GC. By then the
;; producing frame has returned and its globals are cleared, and the driver has
;; dropped its own references, so the second externref is still sitting in the
;; container waiting to be read.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-mv-ref-reentrancy-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; --- Shape 1 ---

  (global $fg (mut funcref) (ref.null func))
  (global $eg (mut externref) (ref.null extern))
  (func (export "setF") (param funcref) (global.set $fg (local.get 0)))
  (func (export "setE") (param externref) (global.set $eg (local.get 0)))
  (func (export "f7") (result i32) (i32.const 7))
  (func (export "f9") (result i32) (i32.const 9))
  (func (export "both") (result funcref externref)
    (global.get $fg)
    (global.get $eg))

  ;; --- Shape 2 ---

  (global $e0 (mut externref) (ref.null extern))
  (global $e1 (mut externref) (ref.null extern))
  (func (export "setE0") (param externref) (global.set $e0 (local.get 0)))
  (func (export "setE1") (param externref) (global.set $e1 (local.get 0)))

  ;; The two references are read into locals and the globals are cleared before
  ;; the results are produced, so once this function has returned its frame is
  ;; gone and the globals hold nothing: the transport container is the only
  ;; thing left holding them.
  (func (export "mixed") (result externref i32 externref)
    (local $a externref)
    (local $b externref)
    (local.set $a (global.get $e0))
    (local.set $b (global.get $e1))
    (global.set $e0 (ref.null extern))
    (global.set $e1 (ref.null extern))
    (local.get $a)
    (i32.const 11)
    (local.get $b)))

;; --- Shape 1 ---

;; The route the reentry used is gone, so the constructor is never reached.
;; CHECK: shape 1: replacement Array constructor calls: 0
;; CHECK-NEXT: shape 1: reentries from the constructor: 0

;; And the values are the outer activation's, which is what would differ if a
;; reentrant activation had written the same storage. The inner activation
;; deliberately installs a DIFFERENT function and a different object.
;; CHECK-NEXT: shape 1: funcref is the outer activation's: true
;; CHECK-NEXT: shape 1: externref is the outer activation's: true

;; --- Shape 2 ---

;; The replacement typed arrays really are the module's buffers: both halves of
;; the numeric path ran through them.
;; CHECK-NEXT: shape 2: typed-array setter fired: true
;; CHECK-NEXT: shape 2: typed-array getter fired: true
;; CHECK-NEXT: shape 2: reentered from both the store gap and the load gap: true
;; CHECK-NEXT: shape 2: collected once while the results were outstanding: true

;; Both externrefs are the outer activation's, and neither is the reentrant
;; one's.
;; CHECK-NEXT: shape 2: result 0 is the outer activation's reference: true
;; CHECK-NEXT: shape 2: result 2 is the outer activation's reference: true
;; CHECK-NEXT: shape 2: neither is the reentrant activation's: true
;; CHECK-NEXT: done
