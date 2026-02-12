;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR structure of export wrapper functions for different type signatures.
;; I.1: Export wrapper functions.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; (i32, i32) -> i32: wrapper coerces both args with AsInt32Inst.
  (func $add_i32 (export "add_i32") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)

  ;; () -> (): void function wrapper returns undefined.
  (func $void_func (export "void_func")
    nop)

  ;; (f64, f64) -> f64: wrapper passes args through (no coercion).
  (func $add_f64 (export "add_f64") (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.add)

  ;; (i32, f64) -> f64: mixed types.
  (func $mixed (export "mixed") (param i32 f64) (result f64)
    local.get 0
    f64.convert_i32_s
    local.get 1
    f64.add)

  ;; (i64) -> i64: i64 wrapper converts BigInt arg and returns BigInt.
  (func $id_i64 (export "id_i64") (param i64) (result i64)
    local.get 0)
)

;; --- Wrapper for add_i32: coerces both i32 args ---
;; CHECK-LABEL: function wasm_export_add_i32(p0: any, p1: any): any
;; CHECK:   GetParentScopeInst
;; CHECK-NEXT:   LoadFrameInst
;; CHECK-NEXT:   LoadParamInst
;; CHECK-NEXT:   AsInt32Inst
;; CHECK-NEXT:   LoadParamInst
;; CHECK-NEXT:   AsInt32Inst
;; CHECK-NEXT:   CallInst
;; CHECK-NEXT:   ReturnInst

;; --- Wrapper for void_func: no params, returns undefined ---
;; CHECK-LABEL: function wasm_export_void_func(): any
;; CHECK:   GetParentScopeInst
;; CHECK-NEXT:   LoadFrameInst
;; CHECK-NEXT:   CallInst
;; CHECK-NEXT:   ReturnInst {{.*}}undefined

;; --- Wrapper for add_f64: passes f64 args through (no coercion) ---
;; CHECK-LABEL: function wasm_export_add_f64(p0: any, p1: any): any
;; CHECK:   GetParentScopeInst
;; CHECK-NEXT:   LoadFrameInst
;; CHECK-NEXT:   LoadParamInst
;; CHECK-NEXT:   LoadParamInst
;; CHECK-NEXT:   CallInst
;; CHECK-NEXT:   ReturnInst

;; --- Wrapper for mixed: i32 coerced, f64 passed through ---
;; CHECK-LABEL: function wasm_export_mixed(p0: any, p1: any): any
;; CHECK:   GetParentScopeInst
;; CHECK-NEXT:   LoadFrameInst
;; CHECK-NEXT:   LoadParamInst
;; CHECK-NEXT:   AsInt32Inst
;; CHECK-NEXT:   LoadParamInst
;; CHECK-NEXT:   CallInst
;; CHECK-NEXT:   ReturnInst

;; --- Wrapper for id_i64: BigInt param converted to lo/hi, result back to BigInt ---
;; CHECK-LABEL: function wasm_export_id_i64(p0: any): any
;; CHECK:   GetParentScopeInst
;; CHECK-NEXT:   LoadFrameInst
;; CHECK-NEXT:   LoadParamInst
;; CHECK-NEXT:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmBigIntToI64]
;; CHECK-NEXT:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiResult]
;; CHECK-NEXT:   CallInst
;; CHECK-NEXT:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiResult]
;; CHECK-NEXT:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64ToBigInt]
;; CHECK-NEXT:   ReturnInst
