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
;; The funcref half travels a genuine Exported Function -- this module's own
;; `seven` -- because a funcref result now takes null or a WebAssembly
;; Exported Function and `importedBad` below is what refuses the rest. It used
;; to be a plain JS function, from back when the trampoline handed whatever the
;; import returned straight to Wasm.
;;
;; In `importedBad` the import returns a plain JS function and the trampoline
;; refuses it. The counter is what makes the
;; refusal mean something. It is bumped AFTER the call returns and nothing in
;; that body would refuse a bad funcref on its own, so `count` still reading 0
;; says the trampoline said no rather than the body having carried on with a
;; JS function in a funcref. (e2e-ref-conversion-points.wat asks the same
;; question of the single-result arm and of export-wrapper parameters, over a
;; wider set of values.)
;;
;; The parameter side needed no change and is left alone: it already passes
;; the JS value straight through to the callee.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-mv-ref-import-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; JS returns [number, arbitrary JS value].
  (import "env" "pairExtern" (func $pairExtern (result i32 externref)))
  ;; JS returns [number, this module's exported `seven`].
  (import "env" "pairFunc" (func $pairFunc (result i32 funcref)))
  ;; JS returns [number, a plain JS function]: refused.
  (import "env" "pairBad" (func $pairBad (result i32 funcref)))

  ;; The Exported Function the driver hands back through pairFunc.
  (func (export "seven") (result i32) (i32.const 7))

  (global $n (mut i32) (i32.const 0))
  (func (export "count") (result i32) (global.get $n))

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
    (local.get $r))

  (func (export "importedBad") (result funcref)
    (local $r funcref)
    (call $pairBad)
    (local.set $r)
    (drop)
    (global.set $n (i32.add (global.get $n) (i32.const 1)))
    (local.get $r)))

;; Identity is preserved end to end: JS array element -> trampoline store ->
;; Wasm caller's load -> local -> single-result export wrapper -> JS.
;; CHECK: importedExtern: same=true
;; CHECK-NEXT: importedNum: 11
;; CHECK-NEXT: importedFunc: function same=true calls -> 7
;; The import ran (its own counter is 1) and the body after the call did not
;; (the module's counter is still 0).
;; CHECK-NEXT: importedBad: TypeError: Wasm import: funcref result 1 requires null or a WebAssembly exported function
;; CHECK-NEXT: importedBad reached the import but not the body: 1 0
