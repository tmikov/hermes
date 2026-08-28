;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for type conversion operations.
;; Tests that conversions compile and execute without crashing.

;; REQUIRES: wasm

;; Test 1: Two-step compilation via WebAssembly API.
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/load-hbc.js_ -- %t.hbc

;; Test 2: Verify IR is well-formed.
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; --- Trapping truncation ---

  (func $trunc_f64_s (param f64) (result i32)
    local.get 0
    i32.trunc_f64_s)

  (func $trunc_f64_u (param f64) (result i32)
    local.get 0
    i32.trunc_f64_u)

  (func $trunc_f32_s (param f32) (result i32)
    local.get 0
    i32.trunc_f32_s)

  (func $trunc_f32_u (param f32) (result i32)
    local.get 0
    i32.trunc_f32_u)

  ;; --- Saturating truncation ---

  (func $sat_f64_s (param f64) (result i32)
    local.get 0
    i32.trunc_sat_f64_s)

  (func $sat_f64_u (param f64) (result i32)
    local.get 0
    i32.trunc_sat_f64_u)

  (func $sat_f32_s (param f32) (result i32)
    local.get 0
    i32.trunc_sat_f32_s)

  (func $sat_f32_u (param f32) (result i32)
    local.get 0
    i32.trunc_sat_f32_u)

  ;; --- Int-to-float conversions ---

  (func $f32_convert_s (param i32) (result f32)
    local.get 0
    f32.convert_i32_s)

  (func $f32_convert_u (param i32) (result f32)
    local.get 0
    f32.convert_i32_u)

  (func $f64_convert_s (param i32) (result f64)
    local.get 0
    f64.convert_i32_s)

  (func $f64_convert_u (param i32) (result f64)
    local.get 0
    f64.convert_i32_u)

  ;; --- Reinterpret ---

  (func $i32_reinterpret_f32 (param f32) (result i32)
    local.get 0
    i32.reinterpret_f32)

  (func $f32_reinterpret_i32 (param i32) (result f32)
    local.get 0
    f32.reinterpret_i32)

  ;; Start function: exercise all conversions.
  (func $start
    ;; Trapping truncations with valid values.
    (drop (call $trunc_f64_s (f64.const 2.9)))
    (drop (call $trunc_f64_s (f64.const -2.9)))
    (drop (call $trunc_f64_u (f64.const 3000000000.5)))
    (drop (call $trunc_f32_s (f32.const 42.7)))
    (drop (call $trunc_f32_u (f32.const 100.0)))

    ;; Saturating truncations (including edge cases).
    (drop (call $sat_f64_s (f64.const 3000000000.0)))
    (drop (call $sat_f64_s (f64.const -3000000000.0)))
    (drop (call $sat_f64_s (f64.const nan)))
    (drop (call $sat_f64_u (f64.const 5000000000.0)))
    (drop (call $sat_f64_u (f64.const -1.0)))
    (drop (call $sat_f64_u (f64.const nan)))
    (drop (call $sat_f32_s (f32.const nan)))
    (drop (call $sat_f32_u (f32.const nan)))

    ;; Int-to-float conversions.
    (drop (call $f32_convert_s (i32.const -42)))
    (drop (call $f32_convert_u (i32.const -1)))
    (drop (call $f64_convert_s (i32.const -42)))
    (drop (call $f64_convert_u (i32.const -1)))

    ;; Reinterpret.
    (drop (call $i32_reinterpret_f32 (f32.const 0.0)))
    (drop (call $f32_reinterpret_i32 (i32.const 0)))
  )

  (start $start)
)

;; CHECK-LABEL: function wasm_func_0(p0: number): number 
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncF64S]

;; CHECK-LABEL: function wasm_func_1(p0: number): number 
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncF64U]

;; CHECK-LABEL: function wasm_func_2(p0: number): number 
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncF64S]

;; CHECK-LABEL: function wasm_func_3(p0: number): number 
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncF64U]

;; CHECK-LABEL: function wasm_func_4(p0: number): number 
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncSatF64S]

;; CHECK-LABEL: function wasm_func_5(p0: number): number 
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncSatF64U]

;; CHECK-LABEL: function wasm_func_6(p0: number): number 
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncSatF64S]

;; CHECK-LABEL: function wasm_func_7(p0: number): number 
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32TruncSatF64U]

;; CHECK-LABEL: function wasm_func_8(p0: number): number 
;; CHECK:   AsInt32Inst

;; CHECK-LABEL: function wasm_func_9(p0: number): number 
;; CHECK:   AsUint32Inst

;; CHECK-LABEL: function wasm_func_10(p0: number): number 
;; CHECK:   AsInt32Inst

;; CHECK-LABEL: function wasm_func_11(p0: number): number 
;; CHECK:   AsUint32Inst

;; CHECK-LABEL: function wasm_func_12(p0: number): number 
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmI32ReinterpretF32]

;; CHECK-LABEL: function wasm_func_13(p0: number): number 
;; CHECK:   CallBuiltinInst {{.*}}[HermesBuiltin.wasmF32ReinterpretI32]
