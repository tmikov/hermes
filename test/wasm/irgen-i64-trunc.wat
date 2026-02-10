;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test i64 truncation operations (G.4b): float→i64 trapping and saturating.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; i64.trunc_f64_s: trapping truncation from f64 to signed i64
  (func $trunc_f64_s (param f64) (result i64)
    local.get 0
    i64.trunc_f64_s)

;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64TruncF64S]
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiResult]

  ;; i64.trunc_f64_u: trapping truncation from f64 to unsigned i64
  (func $trunc_f64_u (param f64) (result i64)
    local.get 0
    i64.trunc_f64_u)

;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64TruncF64U]
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiResult]

  ;; i64.trunc_f32_s: same as f64 in Phase 1
  (func $trunc_f32_s (param f32) (result i64)
    local.get 0
    i64.trunc_f32_s)

;; CHECK-LABEL: function wasm_func_2(p0: any): any
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64TruncF64S]
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiResult]

  ;; i64.trunc_f32_u: same as f64 in Phase 1
  (func $trunc_f32_u (param f32) (result i64)
    local.get 0
    i64.trunc_f32_u)

;; CHECK-LABEL: function wasm_func_3(p0: any): any
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64TruncF64U]
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiResult]

  ;; i64.trunc_sat_f64_s: saturating truncation from f64 to signed i64
  (func $trunc_sat_f64_s (param f64) (result i64)
    local.get 0
    i64.trunc_sat_f64_s)

;; CHECK-LABEL: function wasm_func_4(p0: any): any
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64TruncSatF64S]
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiResult]

  ;; i64.trunc_sat_f64_u: saturating truncation from f64 to unsigned i64
  (func $trunc_sat_f64_u (param f64) (result i64)
    local.get 0
    i64.trunc_sat_f64_u)

;; CHECK-LABEL: function wasm_func_5(p0: any): any
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64TruncSatF64U]
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiResult]

  ;; i64.trunc_sat_f32_s: same as f64 sat in Phase 1
  (func $trunc_sat_f32_s (param f32) (result i64)
    local.get 0
    i64.trunc_sat_f32_s)

;; CHECK-LABEL: function wasm_func_6(p0: any): any
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64TruncSatF64S]
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiResult]

  ;; i64.trunc_sat_f32_u: same as f64 sat in Phase 1
  (func $trunc_sat_f32_u (param f32) (result i64)
    local.get 0
    i64.trunc_sat_f32_u)

;; CHECK-LABEL: function wasm_func_7(p0: any): any
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64TruncSatF64U]
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiResult]
)
