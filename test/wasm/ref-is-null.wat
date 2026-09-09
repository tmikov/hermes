;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; ref.is_null, for both reference types, at the DEFAULT optimization level.
;;
;; The optimization level is the point. wasmValTypeToIRType annotates a
;; funcref, and until that annotation admitted null, InstSimplify folded
;; `ref === null` against a disjoint type and answered false without emitting
;; a test. A module that only asked `ref.is_null` of a literal `ref.null`
;; would have passed anyway: the literal carries its own null type. So the
;; funcref rows below reach ref.is_null through the four places that
;; annotation lands instead -- a parameter, a local slot, a direct call
;; result, and a call_indirect result. Narrowing the annotation back to
;; Object turns the null answer of all four into 0.
;;
;; The externref rows are not folding cases (externref annotates as any).
;; They are here for the other half of strictness: `undefined` is an ordinary
;; non-null externref, and a loose comparison would report it as null.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/ref-is-null-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (type $ret_f (func (result funcref)))
  (type $ret_e (func (result externref)))

  ;; The non-null funcref has to come from somewhere, and function-body
  ;; ref.func is not implemented yet. An imported funcref global fed with the
  ;; module's own Exported Function is the route that exists today.
  (import "e" "fnull" (global $fnull funcref))
  (import "e" "ffn" (global $ffn funcref))
  (import "e" "enull" (global $enull externref))
  (import "e" "eundef" (global $eundef externref))
  (import "e" "eobj" (global $eobj externref))

  (table 5 funcref)
  (elem (i32.const 0) $get_fnull $get_ffn $get_enull $get_eundef $get_eobj)

  (func $get_fnull (result funcref) global.get $fnull)
  (func $get_ffn (result funcref) global.get $ffn)
  (func $get_enull (result externref) global.get $enull)
  (func $get_eundef (result externref) global.get $eundef)
  (func $get_eobj (result externref) global.get $eobj)

  ;; --- A literal ref.null. The row the annotation cannot affect. ---
  (func (export "lit_f") (result i32) ref.null func ref.is_null)
  (func (export "lit_e") (result i32) ref.null extern ref.is_null)

  ;; --- A parameter. WasmIRGen.cpp annotates the AllocStackInst that backs
  ;; it, and the value comes from JS through the export wrapper. ---
  (func (export "param_f") (param funcref) (result i32)
    local.get 0 ref.is_null)
  (func (export "param_e") (param externref) (result i32)
    local.get 0 ref.is_null)

  ;; --- A declared local. Zero-initialized to null; the second pair stores
  ;; the parameter into it first, so the same slot answers both ways. ---
  (func (export "local_f_init") (result i32)
    (local funcref)
    local.get 0 ref.is_null)
  (func (export "local_e_init") (result i32)
    (local externref)
    local.get 0 ref.is_null)
  (func (export "local_f_set") (param funcref) (result i32)
    (local funcref)
    local.get 0 local.set 1
    local.get 1 ref.is_null)
  (func (export "local_e_set") (param externref) (result i32)
    (local externref)
    local.get 0 local.set 1
    local.get 1 ref.is_null)

  ;; --- A direct call result. ---
  (func (export "call_fnull") (result i32) call $get_fnull ref.is_null)
  (func (export "call_ffn") (result i32) call $get_ffn ref.is_null)
  (func (export "call_enull") (result i32) call $get_enull ref.is_null)
  (func (export "call_eundef") (result i32) call $get_eundef ref.is_null)
  (func (export "call_eobj") (result i32) call $get_eobj ref.is_null)

  ;; --- A call_indirect result, dispatched through the table above. ---
  (func (export "ind_f") (param i32) (result i32)
    local.get 0
    call_indirect (type $ret_f)
    ref.is_null)
  (func (export "ind_e") (param i32) (result i32)
    local.get 0
    call_indirect (type $ret_e)
    ref.is_null)
)

;; A literal `ref.null` of either type is null.
;; CHECK: lit: 1 1

;; A parameter. The non-null rows are here so that "answer 1 always" is not
;; a passing implementation; the null funcref row is one of the four that
;; folded to a constant 0 before the annotation admitted null.
;; CHECK-NEXT: param_f(null): 1
;; CHECK-NEXT: param_f(exported fn): 0
;; CHECK-NEXT: param_e(null): 1
;; CHECK-NEXT: param_e(undefined): 0
;; CHECK-NEXT: param_e(object): 0

;; A declared local, zero-initialized, then the same slot after a store.
;; CHECK-NEXT: local init: 1 1
;; CHECK-NEXT: local_f_set(null/fn): 1 0
;; CHECK-NEXT: local_e_set(null/undefined/object): 1 0 0

;; A direct call result.
;; CHECK-NEXT: call_f(null/fn): 1 0
;; CHECK-NEXT: call_e(null/undefined/object): 1 0 0

;; A call_indirect result, through table slots 0..4.
;; CHECK-NEXT: ind_f(null/fn): 1 0
;; CHECK-NEXT: ind_e(null/undefined/object): 1 0 0

;; And the answer is an i32, not a boolean or a reference: `| 0` leaves 1 and
;; 0 alone but turns `true`, `false` and `undefined` into something else, and
;; typeof pins it against a Number-valued 1 that came from somewhere odd.
;; CHECK-NEXT: shape: number 1 0

;; CHECK-NEXT: done
