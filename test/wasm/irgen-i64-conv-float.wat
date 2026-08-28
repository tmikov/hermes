;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test i64→float conversions and reinterpret (G.4c).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; f64.convert_i64_s: signed i64 → f64
  (func $f64_convert_i64_s (result f64)
    i64.const 42
    f64.convert_i64_s)

;; CHECK-LABEL: function wasm_func_0()
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmF64ConvertI64S]

  ;; f64.convert_i64_u: unsigned i64 → f64
  (func $f64_convert_i64_u (result f64)
    i64.const -1  ;; = 0xFFFFFFFF_FFFFFFFF unsigned
    f64.convert_i64_u)

;; CHECK-LABEL: function wasm_func_1()
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmF64ConvertI64U]

  ;; f32.convert_i64_s: signed i64 → f32
  (func $f32_convert_i64_s (result f32)
    i64.const 100
    f32.convert_i64_s)

;; CHECK-LABEL: function wasm_func_2()
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmF32ConvertI64S]

  ;; f32.convert_i64_u: unsigned i64 → f32
  (func $f32_convert_i64_u (result f32)
    i64.const 200
    f32.convert_i64_u)

;; CHECK-LABEL: function wasm_func_3()
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmF32ConvertI64U]

  ;; i64.reinterpret_f64: bitcast f64 to i64
  (func $i64_reinterpret_f64 (result i64)
    f64.const 1.0
    i64.reinterpret_f64)

;; CHECK-LABEL: function wasm_func_4()
;; Constant-folded: f64.const 1.0 has bits 0x3FF0000000000000 (lo=0, hi=1072693248).
;; CHECK-NOT: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64ReinterpretF64]
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiStash]

  ;; f64.reinterpret_i64: bitcast i64 to f64
  (func $f64_reinterpret_i64 (result f64)
    i64.const 4607182418800017408  ;; 0x3FF0000000000000 = 1.0 as f64 bits
    f64.reinterpret_i64)

;; CHECK-LABEL: function wasm_func_5()
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmF64ReinterpretI64]
)
