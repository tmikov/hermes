;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s
;; REQUIRES: wasm

;; Test i32 trapping truncations from f64/f32
(module
  ;; CHECK-LABEL: function wasm_func_0(p0: any): any
  ;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncF64S]
  ;; CHECK: ReturnInst
  (func (export "trunc_f64_s") (param f64) (result i32)
    local.get 0
    i32.trunc_f64_s)

  ;; CHECK-LABEL: function wasm_func_1(p0: any): any
  ;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncF64U]
  ;; CHECK: ReturnInst
  (func (export "trunc_f64_u") (param f64) (result i32)
    local.get 0
    i32.trunc_f64_u)

  ;; CHECK-LABEL: function wasm_func_2(p0: any): any
  ;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncF64S]
  ;; CHECK: ReturnInst
  (func (export "trunc_f32_s") (param f32) (result i32)
    local.get 0
    i32.trunc_f32_s)

  ;; CHECK-LABEL: function wasm_func_3(p0: any): any
  ;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncF64U]
  ;; CHECK: ReturnInst
  (func (export "trunc_f32_u") (param f32) (result i32)
    local.get 0
    i32.trunc_f32_u)

  ;; Test saturating truncations
  ;; CHECK-LABEL: function wasm_func_4(p0: any): any
  ;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncSatF64S]
  ;; CHECK: ReturnInst
  (func (export "trunc_sat_f64_s") (param f64) (result i32)
    local.get 0
    i32.trunc_sat_f64_s)

  ;; CHECK-LABEL: function wasm_func_5(p0: any): any
  ;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncSatF64U]
  ;; CHECK: ReturnInst
  (func (export "trunc_sat_f64_u") (param f64) (result i32)
    local.get 0
    i32.trunc_sat_f64_u)

  ;; CHECK-LABEL: function wasm_func_6(p0: any): any
  ;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncSatF64S]
  ;; CHECK: ReturnInst
  (func (export "trunc_sat_f32_s") (param f32) (result i32)
    local.get 0
    i32.trunc_sat_f32_s)

  ;; CHECK-LABEL: function wasm_func_7(p0: any): any
  ;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncSatF64U]
  ;; CHECK: ReturnInst
  (func (export "trunc_sat_f32_u") (param f32) (result i32)
    local.get 0
    i32.trunc_sat_f32_u)

  ;; Test int-to-float conversions
  ;; CHECK-LABEL: function wasm_func_8(p0: any): any
  ;; CHECK: AsInt32Inst
  ;; CHECK: ReturnInst
  (func (export "f64_convert_i32_s") (param i32) (result f64)
    local.get 0
    f64.convert_i32_s)

  ;; CHECK-LABEL: function wasm_func_9(p0: any): any
  ;; CHECK: AsUint32Inst
  ;; CHECK: ReturnInst
  (func (export "f64_convert_i32_u") (param i32) (result f64)
    local.get 0
    f64.convert_i32_u)

  ;; CHECK-LABEL: function wasm_func_10(p0: any): any
  ;; CHECK: AsInt32Inst
  ;; CHECK: ReturnInst
  (func (export "f32_convert_i32_s") (param i32) (result f32)
    local.get 0
    f32.convert_i32_s)

  ;; CHECK-LABEL: function wasm_func_11(p0: any): any
  ;; CHECK: AsUint32Inst
  ;; CHECK: ReturnInst
  (func (export "f32_convert_i32_u") (param i32) (result f32)
    local.get 0
    f32.convert_i32_u)

  ;; Test reinterpret/bitcast
  ;; CHECK-LABEL: function wasm_func_12(p0: any): any
  ;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32ReinterpretF32]
  ;; CHECK: ReturnInst
  (func (export "i32_reinterpret_f32") (param f32) (result i32)
    local.get 0
    i32.reinterpret_f32)

  ;; CHECK-LABEL: function wasm_func_13(p0: any): any
  ;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmF32ReinterpretI32]
  ;; CHECK: ReturnInst
  (func (export "f32_reinterpret_i32") (param i32) (result f32)
    local.get 0
    f32.reinterpret_i32)
)
