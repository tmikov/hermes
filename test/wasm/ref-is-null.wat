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
;; funcref rows below reach ref.is_null through four of the five places that
;; annotation lands -- a parameter, a local slot, a direct call result, and a
;; call_indirect result. Narrowing the annotation back to Object turns the
;; null answer of all four into 0. The fifth place is a function's own return
;; type; irgen-table.wat and irgen-mv-ref-retbuf.wat pin that one, and so does
;; the IR run below.
;;
;; The externref rows are not folding cases (externref annotates as any).
;; They are here for the other half of strictness: `undefined` is an ordinary
;; non-null externref, and a loose comparison would report it as null.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/ref-is-null-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck --check-prefix=IR %s

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

  ;; --- A declared local, zero-initialized to null. The second pair stores
  ;; the parameter into the slot first. At -O0 both pairs read the slot; at
  ;; the level the behavioural run uses, the slot in the second pair is
  ;; promoted away and its answer comes from the parameter instead, so
  ;; local_f_init is what holds the local's own annotation there. ---
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

  ;; --- The other consumer shape: the i32 feeding a branch rather than a
  ;; return. onBrIf runs the condition through peekThroughAsInt32, which
  ;; discards the AsInt32Inst and branches on the boolean underneath, so the
  ;; narrowing this opcode emits is undone again on this path. Answers 10 for
  ;; null and 20 for non-null, so a branch taken the wrong way is not merely
  ;; a swapped 0 and 1. ---
  (func (export "brif_f") (param funcref) (result i32)
    (block
      local.get 0
      ref.is_null
      br_if 0
      i32.const 20
      return)
    i32.const 10)
  (func (export "brif_e") (param externref) (result i32)
    (block
      local.get 0
      ref.is_null
      br_if 0
      i32.const 20
      return)
    i32.const 10)
)

;; A literal `ref.null` of either type is null.
;; CHECK: lit: 1 1

;; A parameter. The non-null rows are here so that "answer 1 always" is not
;; a passing implementation; the null funcref row folded to a constant 0
;; before the annotation admitted null.
;; CHECK-NEXT: param_f(null): 1
;; CHECK-NEXT: param_f(exported fn): 0
;; CHECK-NEXT: param_e(null): 1
;; CHECK-NEXT: param_e(undefined): 0
;; CHECK-NEXT: param_e(object): 0

;; A declared local, zero-initialized, then the second pair after a store.
;; CHECK-NEXT: local init: 1 1
;; CHECK-NEXT: local_f_set(null/fn): 1 0
;; CHECK-NEXT: local_e_set(null/undefined/object): 1 0 0

;; A direct call result.
;; CHECK-NEXT: call_f(null/fn): 1 0
;; CHECK-NEXT: call_e(null/undefined/object): 1 0 0

;; A call_indirect result, through table slots 0..4. A direct call and a
;; call_indirect get their result annotations at separate sites, so neither
;; row stands in for the other.
;; CHECK-NEXT: ind_f(null/fn): 1 0
;; CHECK-NEXT: ind_e(null/undefined/object): 1 0 0

;; The i32 consumed as a branch condition instead of returned. 10 is the null
;; answer and 20 the non-null one.
;; CHECK-NEXT: brif_f(null/fn): 10 20
;; CHECK-NEXT: brif_e(null/undefined/object): 10 20 20

;; And the answer is the number 1 or the number 0. Both halves of each pair
;; are needed: typeof refuses a boolean, `undefined` and a reference, while
;; the strict comparison refuses a string, since '1' === 1 is false and the
;; lines above cannot tell "0" from 0. Both results are checked, not just the
;; null one: a string returned for only the non-null case was measured to
;; satisfy the lines above.
;; CHECK-NEXT: shape: number true / number true

;; CHECK-NEXT: done

;; --- The annotation itself, at -O0, where no fold can hide it ---
;;
;; The behavioural rows above are counterfactual evidence: they answer
;; correctly, and answer wrongly when the annotation is narrowed. That works
;; only because InstSimplify is scheduled ahead of Inlining
;; (lib/Optimizer/PassManager/Pipeline.cpp), so the direct call is still a
;; CallInst when the fold happens; Inlining replaces a call with the returned
;; value without transferring the call's type. These lines do not depend on
;; that ordering. They name each place wasmValTypeToIRType's funcref
;; annotation lands and read it off the instruction.
;;
;; The function numbers follow the module's own function order, so inserting a
;; function above shifts them.

;; The function's own return type -- $get_fnull, the fifth site.
;; IR-LABEL: function wasm_func_0(): null|object

;; A parameter, and the stack slot it is stored into: param_f.
;; IR-LABEL: function wasm_func_7(p0: null|object): number
;; IR: %{{[0-9]+}} = AllocStackInst (:null|object) $local_0: any
;; IR: %{{[0-9]+}} = BinaryStrictlyEqualInst (:any) %{{[0-9]+}}: null|object, null: null

;; A declared local's slot: local_f_init.
;; IR-LABEL: function wasm_func_9(): number
;; IR: %{{[0-9]+}} = AllocStackInst (:null|object) $local_0: any
;; IR: %{{[0-9]+}} = BinaryStrictlyEqualInst (:any) %{{[0-9]+}}: null|object, null: null

;; A direct call result: call_fnull.
;; IR-LABEL: function wasm_func_13(): number
;; IR: %{{[0-9]+}} = CallInst (:null|object) %{{[0-9]+}}: any, %wasm_func_0(): functionCode
;; IR-NEXT: %{{[0-9]+}} = BinaryStrictlyEqualInst (:any) %{{[0-9]+}}: null|object, null: null

;; A call_indirect result: ind_f. A separate annotation site from the above.
;; IR-LABEL: function wasm_func_18(p0: number): number
;; IR: %{{[0-9]+}} = CallInst (:null|object) %{{[0-9]+}}: any, empty: any
;; IR-NEXT: %{{[0-9]+}} = BinaryStrictlyEqualInst (:any) %{{[0-9]+}}: null|object, null: null
