;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The return buffer's reference slots on the import side. An imported JS
;; function with a multi-value result list is called through a trampoline: the
;; trampoline reads the elements of the JS array the import returned and
;; stores each one into the buffer at its offset, and the Wasm caller reads
;; them back out. Before the fix the trampoline's `default:` store arm used
;; the Uint32Array view, so a funcref or externref element was coerced to 0
;; on the way in -- the same destruction as on the wasm->wasm path, one
;; boundary earlier.
;;
;; The parameter side needed no change and is left alone: it already passes
;; the JS value straight through to the callee.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-mv-ref-import-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; JS returns [number, arbitrary JS value].
  (import "env" "pairExtern" (func $pairExtern (result i32 externref)))
  ;; JS returns [number, JS function]. A funcref is a JS closure here, so a
  ;; plain JS function is exactly what the trampoline must preserve.
  (import "env" "pairFunc" (func $pairFunc (result i32 funcref)))

  (func (export "importedExtern") (result externref)
    (local $r externref)
    (call $pairExtern)
    (local.set $r)
    (drop)
    (local.get $r))

  ;; The i32 half of the same call, to show the reference store did not land
  ;; on top of it.
  (func (export "importedNum") (result i32)
    (call $pairExtern)
    (drop))

  (func (export "importedFunc") (result funcref)
    (local $r funcref)
    (call $pairFunc)
    (local.set $r)
    (drop)
    (local.get $r)))

;; Identity is preserved end to end: JS array element -> trampoline store ->
;; Wasm caller's load -> local -> single-result export wrapper -> JS.
;; CHECK: importedExtern: same=true
;; CHECK-NEXT: importedNum: 11
;; CHECK-NEXT: importedFunc: function same=true calls -> 7
