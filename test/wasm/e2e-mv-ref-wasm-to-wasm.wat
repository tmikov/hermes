;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The return buffer's reference slots, exercised across a wasm->wasm call --
;; the path where the silent 0 actually originated, and the one the export
;; wrapper test cannot reach.
;;
;; $mv is NOT exported. Its multi-value result crosses only an internal
;; boundary: emitRetBufStores writes the results, the caller's emitRetBufLoads
;; reads them straight back. Before the fix both `default:` arms used the
;; Uint32Array view, so the funcref was coerced to 0 at the store; the caller
;; then pushed that 0 onto the value stack and returned it as a funcref. No
;; JS-visible marshalling was involved, so nothing warned.
;;
;; svGet is the control: a single funcref result bypasses the buffer entirely,
;; so it must return the same closure mvGet does.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-mv-ref-wasm-to-wasm-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (table 1 funcref)
  (elem (i32.const 0) $f)
  (func $f (result i32) (i32.const 7))

  ;; (i32, funcref) through the buffer, wasm-internal only.
  (func $mv (result i32 funcref)
    (i32.const 42)
    (table.get (i32.const 0)))

  ;; (externref, i32) through the buffer, wasm-internal only. The reference is
  ;; first in the result list here, so it lands in reference slot 0 while the
  ;; i32 lands in integer slot 1.
  (func $mvE (param externref) (result externref i32)
    (local.get 0)
    (i32.const 3))

  ;; Control: no buffer involved.
  (func (export "svGet") (result funcref)
    (table.get (i32.const 0)))

  ;; Take the funcref half out of the multi-value return and hand it back.
  (func (export "mvGet") (result funcref)
    (local $r funcref)
    (call $mv)
    (local.set $r)
    (drop)
    (local.get $r))

  ;; The i32 half of the same call must be undisturbed by the reference store.
  (func (export "mvNum") (result i32)
    (call $mv)
    (drop))

  ;; Same, for externref.
  (func (export "mvExternGet") (param externref) (result externref)
    (local.get 0)
    (call $mvE)
    (drop)))

;; The funcref that came back through the buffer is callable and is the same
;; closure the buffer-free path returns -- not a 0 wearing its type.
;; CHECK: svGet: function calls -> 7
;; CHECK-NEXT: mvGet: function calls -> 7 same=true
;; CHECK-NEXT: mvNum: 42
;; CHECK-NEXT: mvExternGet: same=true
