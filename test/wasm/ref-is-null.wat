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

  ;; The non-null funcref has to come from somewhere. An imported funcref
  ;; global, fed with the module's own Exported Function, is the route this
  ;; file uses. Function-body `ref.func` would be another one now that it is
  ;; implemented, but it is not added here: it pushes a value out of a frame
  ;; variable, so it lands on none of the five annotation sites this file is
  ;; about, and a row that does not exercise the subject does not belong in
  ;; it. `ref.is_null` of a `ref.func` is covered in e2e-ref-func-body.wat
  ;; instead.
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
  ;; return. onBrIf passes the condition through peekThroughAsInt32, which is
  ;; a no-op here: it unwraps only an operand that is already boolean, and the
  ;; comparison onRefIsNull builds is typed `any` when onBrIf sees it. So
  ;; br_if branches on the AsInt32Inst itself, and the -O0 dump shows
  ;; `CondBranchInst %7: number` consuming it. (Whether that guard is ever
  ;; true at IRGen time, which would make the helper dead, is dz
  ;; 01a085ae-1c61.) Answers 10 for null and 20 for non-null, so a branch
  ;; taken the wrong way is not merely a swapped 0 and 1. ---
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

;; And the answer is the number 1 or the number 0. The strict comparison is
;; what does the work -- `=== 1` and `=== 0` fix both the value and the type,
;; which the lines above cannot, since string concatenation prints "0" and 0
;; the same. The typeof half is redundant with it and kept only because it
;; names the wrong answer in the output instead of just saying false. What is
;; not redundant is checking BOTH results: a string returned for only the
;; non-null case was measured to satisfy every other line the behavioural run
;; prints.
;; CHECK-NEXT: shape: number true / number true

;; CHECK-NEXT: done

;; --- The annotation itself, at -O0, where no fold can hide it ---
;;
;; The behavioural rows above are counterfactual evidence: they answer
;; correctly, and answer wrongly when the annotation is narrowed. For the
;; call_f row that depends on pass order -- Inlining replaces a call with the
;; returned value without transferring the call's type, so the fold that
;; makes the row go red has to happen first, and it does only because
;; InstSimplify is added ahead of Inlining in
;; lib/Optimizer/PassManager/Pipeline.cpp. The parameter, local and
;; call_indirect rows have no inlinable call and do not depend on that.
;;
;; The lines below are read from a -O0 dump, so no optimization pass has run
;; by the time they are matched. They name each place wasmValTypeToIRType's
;; funcref annotation lands, read it off the instruction, and require the
;; compared operand to be the very value the annotated instruction produced --
;; adjacency alone would also be satisfied by a comparison against some other
;; value of the same type.

;; The function's own return type -- $get_fnull, the fifth site. There is no
;; comparison in it to connect, and it is not exported, so the direct-call pin
;; below is what anchors its number: it names $get_fnull as call_fnull's
;; callee.
;; IR-LABEL: function wasm_func_0(): null|object
;; IR-LABEL: function_end

;; A parameter: param_f. The slot, the load out of that slot, and the operand
;; the comparison consumes are required to be one value, by name.
;; IR-LABEL: function wasm_func_7(p0: null|object): number
;; IR: %[[PSLOT:[0-9]+]] = AllocStackInst (:null|object) $local_0: any
;; IR: %[[PVAL:[0-9]+]] = LoadStackInst (:null|object) %[[PSLOT]]: null|object
;; IR-NEXT: %{{[0-9]+}} = BinaryStrictlyEqualInst (:any) %[[PVAL]]: null|object, null: null
;; IR-LABEL: function_end

;; A declared local's slot: local_f_init, connected the same way.
;; IR-LABEL: function wasm_func_9(): number
;; IR: %[[LSLOT:[0-9]+]] = AllocStackInst (:null|object) $local_0: any
;; IR: %[[LVAL:[0-9]+]] = LoadStackInst (:null|object) %[[LSLOT]]: null|object
;; IR-NEXT: %{{[0-9]+}} = BinaryStrictlyEqualInst (:any) %[[LVAL]]: null|object, null: null
;; IR-LABEL: function_end

;; A direct call result: call_fnull. The comparison consumes the call, not
;; merely a value printed next to it.
;; IR-LABEL: function wasm_func_13(): number
;; IR: %[[DCALL:[0-9]+]] = CallInst (:null|object) %{{[0-9]+}}: any, %wasm_func_0(): functionCode
;; IR-NEXT: %{{[0-9]+}} = BinaryStrictlyEqualInst (:any) %[[DCALL]]: null|object, null: null
;; IR-LABEL: function_end

;; A call_indirect result: ind_f. A separate annotation site from the above.
;; IR-LABEL: function wasm_func_18(p0: number): number
;; IR: %[[ICALL:[0-9]+]] = CallInst (:null|object) %{{[0-9]+}}: any, empty: any
;; IR-NEXT: %{{[0-9]+}} = BinaryStrictlyEqualInst (:any) %[[ICALL]]: null|object, null: null
;; IR-LABEL: function_end

;; --- And those numbers, anchored by name ---
;;
;; The pins above address functions by index, which a function inserted into
;; the module renumbers. The four export wrappers below keep their export's
;; name and call their internal function by an explicit target, so these lines
;; turn a renumbering into a failure even when the function that lands on the
;; old number has the same shape -- measured by inserting a copy of param_f
;; above it, which leaves the shape pin for wasm_func_7 green.

;; IR-LABEL: function wasm_export_param_f(p0: any): any
;; IR: CallInst (:any) %{{[0-9]+}}: any, %wasm_func_7(): functionCode
;; IR-LABEL: function_end

;; IR-LABEL: function wasm_export_local_f_init(): any
;; IR: CallInst (:any) %{{[0-9]+}}: any, %wasm_func_9(): functionCode
;; IR-LABEL: function_end

;; IR-LABEL: function wasm_export_call_fnull(): any
;; IR: CallInst (:any) %{{[0-9]+}}: any, %wasm_func_13(): functionCode
;; IR-LABEL: function_end

;; IR-LABEL: function wasm_export_ind_f(p0: any): any
;; IR: CallInst (:any) %{{[0-9]+}}: any, %wasm_func_18(): functionCode
;; IR-LABEL: function_end
