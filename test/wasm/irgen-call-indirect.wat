;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for Wasm call_indirect instruction.
;; Verifies that call_indirect emits the wasmCallIndirect builtin for
;; validation, followed by a CallInst to the returned closure.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (type $void_to_i32 (func (result i32)))
  (type $i32_to_i32 (func (param i32) (result i32)))

  (table 3 funcref)
  (elem (i32.const 0) $f0 $f1 $f2)

  (func $f0 (result i32)
    i32.const 10)

  (func $f1 (result i32)
    i32.const 20)

  ;; type 1: (i32) -> i32
  (func $f2 (param i32) (result i32)
    local.get 0)

  ;; Test 1: Basic call_indirect with no args (type $void_to_i32).
  ;; Checks full IR for the call_indirect sequence.
  (func $test_basic (param i32) (result i32)
    local.get 0
    call_indirect (type $void_to_i32))
;; CHECK-LABEL: function wasm_func_3(p0: any): any
;; CHECK: %BB0:
;; CHECK:   %[[SCOPE:.*]] = GetParentScopeInst (:environment)
;; CHECK:   %[[IDX:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[FUNCS:.*]] = LoadFrameInst (:any) %[[SCOPE]]: environment, [%VS0.table_0_funcs]: any
;; CHECK-NEXT: %[[TYPES:.*]] = LoadFrameInst (:any) %[[SCOPE]]: environment, [%VS0.table_0_types]: any
;; The expected type is the interned id for this signature, loaded from the
;; frame. A module-local index literal would not be comparable across modules.
;; CHECK-NEXT: %[[TYPEID:.*]] = LoadFrameInst (:any) %[[SCOPE]]: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT: %[[CLOSURE:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmCallIndirect]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %[[FUNCS]]: any, %[[TYPES]]: any, %[[IDX]]: any, %[[TYPEID]]: any
;; CHECK-NEXT: %[[RESULT:.*]] = CallInst (:any) %[[CLOSURE]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:                  BranchInst %BB1

  ;; Test 2: call_indirect with args (type $i32_to_i32).
  ;; Verifies arguments are passed through to the CallInst.
  (func $test_with_args (param i32 i32) (result i32)
    local.get 1     ;; argument to pass
    local.get 0     ;; table index
    call_indirect (type $i32_to_i32))
;; CHECK-LABEL: function wasm_func_4(p0: any, p1: any): any
;; CHECK: %BB0:
;; CHECK:   %[[SCOPE2:.*]] = GetParentScopeInst (:environment)
;; CHECK:   %[[ARG:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[IDX2:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[FUNCS2:.*]] = LoadFrameInst (:any) %[[SCOPE2]]: environment, [%VS0.table_0_funcs]: any
;; CHECK-NEXT: %[[TYPES2:.*]] = LoadFrameInst (:any) %[[SCOPE2]]: environment, [%VS0.table_0_types]: any
;; A different signature, so a different interned id -- again loaded from the
;; frame rather than embedded as a module-local index.
;; CHECK-NEXT: %[[TYPEID2:.*]] = LoadFrameInst (:any) %[[SCOPE2]]: environment, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT: %[[CLOSURE2:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmCallIndirect]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %[[FUNCS2]]: any, %[[TYPES2]]: any, %[[IDX2]]: any, %[[TYPEID2]]: any
;; CHECK-NEXT: %[[RESULT2:.*]] = CallInst (:any) %[[CLOSURE2]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %[[ARG]]: any
;; CHECK-NEXT:                   BranchInst %BB1

  ;; Test 3: Void call_indirect (no result).
  (func $test_void (param i32)
    local.get 0
    call_indirect (type $void_to_i32)
    drop)
;; CHECK-LABEL: function wasm_func_5(p0: any): any
;; CHECK:   %[[CLOSURE3:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmCallIndirect]
;; CHECK-NEXT: %{{.*}} = CallInst (:any) %[[CLOSURE3]]: any
)
