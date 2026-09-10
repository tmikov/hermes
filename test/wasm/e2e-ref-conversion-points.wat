;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The two JS-to-Wasm conversion points that no setter covers: a funcref
;; PARAMETER of an exported function, and the funcref RESULT of an imported
;; JS function. Both used to hand the JS value straight to Wasm.
;;
;; What makes the refusals here mean something is that the bodies below have
;; nothing downstream that would refuse the value on its own. A funcref that
;; reaches a table slot or an imported mutable global is refused by the table
;; funnel or by wasmGlobalSet, so a test built on either would pass with the
;; conversion-point check entirely absent. These bodies only bump a counter
;; and hand the value back, so the counter is the oracle: it stays put when
;; the value was refused before the body ran, and it moves on every accepted
;; call, which is what says the counter is not simply stuck.
;;
;; The import side is asked the same way. `call_one` and `call_pair` bump
;; AFTER the call returns, so a counter that did not move says the trampoline
;; refused the JS function's result rather than the caller having quietly
;; carried on. The driver also counts the import's own invocations, so a
;; refusal is distinguishable from an import that was never reached.
;;
;; Rejection alone would be satisfied by refusing everything, so the accepted
;; cases carry as much weight: `null`, the canonical wrapper for a function
;; the module DEFINES, and the canonical wrapper for a function the module
;; IMPORTS (`ref_import`, which is why `$one` is in a declarative segment).
;;
;; The externref rows are here to fail if a check ever appears on that arm:
;; every JS value is a valid externref, so a plain function, a number and
;; `undefined` must all pass through unchanged, as parameters and as results.
;;
;; Multi-value import results are asserted in this file too, at result index
;; 1, which is what distinguishes the multi-value arm of the trampoline from
;; the single-result one. e2e-mv-ref-import.wat covers the same arm's
;; transport.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-ref-conversion-points-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; Imported JS functions whose RESULTS are references.
  (import "e" "one" (func $one (result funcref)))
  (import "e" "pair" (func $pair (result i32 funcref)))
  (import "e" "ext" (func $ext (result externref)))
  (import "e" "extpair" (func $extpair (result i32 externref)))

  ;; $one is named by `ref.func` below, which requires a declaration site.
  ;; It is not exported and appears in no other segment here, so this is the
  ;; one it has.
  (elem declare func $one)

  ;; The "was the body entered" counter.
  (global $n (mut i32) (i32.const 0))
  (func $bump
    (global.set $n (i32.add (global.get $n) (i32.const 1))))
  (func (export "count") (result i32) (global.get $n))
  (func (export "reset") (global.set $n (i32.const 0)))

  ;; A genuine Exported Function, and the wrappers the module can name.
  (func $target (export "target") (param i32) (result i32)
    (i32.add (local.get 0) (i32.const 1)))
  (func (export "ref_target") (result funcref) (ref.func $target))
  (func (export "ref_import") (result funcref) (ref.func $one))

  ;; --- Export-wrapper parameters ---

  ;; Counts, then returns the parameter untouched. No table, no global, no
  ;; call_indirect: nothing here would refuse a bad funcref.
  (func (export "take_fn") (param funcref) (result funcref)
    (call $bump)
    (local.get 0))

  (func (export "take_ext") (param externref) (result externref)
    (call $bump)
    (local.get 0))

  ;; A funcref at argument index 1, behind an f64 the caller must coerce.
  ;; The f64's ToNumber has to have run by the time argument 1 is refused,
  ;; which is what pins the conversion to argument order.
  (func (export "take_num_fn") (param f64) (param funcref) (result funcref)
    (call $bump)
    (local.get 1))

  ;; An externref at argument 0 and a funcref at argument 1: the externref
  ;; must pass whatever it is handed, including a plain JS function.
  (func (export "take_ext_fn") (param externref) (param funcref)
    (result externref)
    (call $bump)
    (local.get 0))

  ;; --- Imported-function results ---

  ;; Single result. The bump is after the call, so it does not run when the
  ;; trampoline refuses.
  (func (export "call_one") (result funcref)
    (local $t funcref)
    (local.set $t (call $one))
    (call $bump)
    (local.get $t))

  ;; Multi-value result: the funcref is result 1.
  (func (export "call_pair") (result funcref)
    (local $t funcref)
    (call $pair)
    (local.set $t)
    (drop)
    (call $bump)
    (local.get $t))

  (func (export "call_ext") (result externref)
    (local $t externref)
    (local.set $t (call $ext))
    (call $bump)
    (local.get $t))

  (func (export "call_extpair") (result externref)
    (local $t externref)
    (call $extpair)
    (local.set $t)
    (drop)
    (call $bump)
    (local.get $t)))

;; The oracle is printed refusing a plain JS function first, so a broken
;; oracle cannot vouch for the brand assertions below.
;; CHECK: oracle refuses a plain JS function: not an Exported Function
;; CHECK-NEXT: ref_import is an Exported Function: wrapper
;; CHECK-NEXT: ref_import is one object: true
;;
;; --- funcref parameters: accepted ---
;; CHECK-NEXT: take_fn(null): null true 1
;; CHECK-NEXT: take_fn(target): true 2
;; CHECK-NEXT: take_fn(ref_target()): true 3
;; CHECK-NEXT: take_fn(ref_import()): true 4
;;
;; --- funcref parameters: refused, and the body was not entered ---
;; CHECK-NEXT: take_fn(plain function): TypeError: Wasm call: funcref argument 0 requires null or a WebAssembly exported function 4
;; CHECK-NEXT: take_fn(5): TypeError: Wasm call: funcref argument 0 requires null or a WebAssembly exported function 4
;; CHECK-NEXT: take_fn(undefined): TypeError: Wasm call: funcref argument 0 requires null or a WebAssembly exported function 4
;; CHECK-NEXT: take_fn({}): TypeError: Wasm call: funcref argument 0 requires null or a WebAssembly exported function 4
;; CHECK-NEXT: take_fn(a wrapper again): true 5
;;
;; Argument 1, and the f64 at argument 0 was coerced before the refusal.
;; CHECK-NEXT: take_num_fn(probe, plain function): TypeError: Wasm call: funcref argument 1 requires null or a WebAssembly exported function 5 1
;; CHECK-NEXT: take_num_fn(probe, target): true 6 2
;;
;; --- externref parameters: every JS value passes ---
;; CHECK-NEXT: take_ext(plain function): true 7
;; CHECK-NEXT: take_ext(5): true 8
;; CHECK-NEXT: take_ext(undefined): true 9
;; CHECK-NEXT: take_ext(null): true 10
;; CHECK-NEXT: take_ext({}): true 11
;; CHECK-NEXT: take_ext_fn(plain function, target): true 12
;;
;; --- single-result funcref import ---
;; CHECK-NEXT: call_one(null): null true 13 1
;; CHECK-NEXT: call_one(target): true 14 2
;; CHECK-NEXT: call_one(ref_import()): true 15 3
;; CHECK-NEXT: call_one(plain function): TypeError: Wasm import: funcref result 0 requires null or a WebAssembly exported function 15 4
;; CHECK-NEXT: call_one(5): TypeError: Wasm import: funcref result 0 requires null or a WebAssembly exported function 15 5
;; CHECK-NEXT: call_one(undefined): TypeError: Wasm import: funcref result 0 requires null or a WebAssembly exported function 15 6
;; CHECK-NEXT: call_one(target again): true 16 7
;;
;; --- multi-value funcref import, at result index 1 ---
;; CHECK-NEXT: call_pair(null): null true 17 1
;; CHECK-NEXT: call_pair(target): true 18 2
;; CHECK-NEXT: call_pair(ref_import()): true 19 3
;; CHECK-NEXT: call_pair(plain function): TypeError: Wasm import: funcref result 1 requires null or a WebAssembly exported function 19 4
;; CHECK-NEXT: call_pair(undefined): TypeError: Wasm import: funcref result 1 requires null or a WebAssembly exported function 19 5
;; CHECK-NEXT: call_pair(target again): true 20 6
;;
;; --- externref results: every JS value passes, both arms ---
;; CHECK-NEXT: call_ext(plain function): true 21
;; CHECK-NEXT: call_ext(5): true 22
;; CHECK-NEXT: call_ext(undefined): true 23
;; CHECK-NEXT: call_ext(null): true 24
;; CHECK-NEXT: call_extpair(plain function): true 25
;; CHECK-NEXT: call_extpair(undefined): true 26
;; CHECK-NEXT: call_extpair(null): true 27
;; CHECK-NEXT: done
