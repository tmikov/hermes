;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for type conversion operations (F.4).
;; Covers trapping truncations, saturating truncations, int-to-float
;; conversions, and reinterpret/bitcast operations.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: i32.trunc_f64_s — trapping signed truncation from f64.
  ;; First function checked exhaustively including param loading.
  (func (export "trunc_f64_s") (param f64) (result i32)
    local.get 0
    i32.trunc_f64_s)
;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:any)
;; CHECK:   %[[P0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:            StoreStackInst %[[P0]]: any, %[[L0]]: any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any) %[[L0]]: any
;; CHECK-NEXT: %[[TRUNC:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmI32TruncF64S]{{.*}}, %[[A]]: any
;; CHECK-NEXT:                 BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[TRUNC]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 1: i32.trunc_f64_u — trapping unsigned truncation from f64.
  (func (export "trunc_f64_u") (param f64) (result i32)
    local.get 0
    i32.trunc_f64_u)
;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[TRUNC:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmI32TruncF64U]{{.*}}, %[[A]]: any
;; CHECK-NEXT:                 BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[TRUNC]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 2: i32.trunc_f32_s — trapping signed truncation from f32.
  ;; f32 uses same builtin as f64 (wasmI32TruncF64S) in Phase 1.
  (func (export "trunc_f32_s") (param f32) (result i32)
    local.get 0
    i32.trunc_f32_s)
;; CHECK-LABEL: function wasm_func_2(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[TRUNC:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmI32TruncF64S]{{.*}}, %[[A]]: any
;; CHECK-NEXT:                 BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[TRUNC]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 3: i32.trunc_f32_u — trapping unsigned truncation from f32.
  ;; f32 uses same builtin as f64 (wasmI32TruncF64U) in Phase 1.
  (func (export "trunc_f32_u") (param f32) (result i32)
    local.get 0
    i32.trunc_f32_u)
;; CHECK-LABEL: function wasm_func_3(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[TRUNC:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmI32TruncF64U]{{.*}}, %[[A]]: any
;; CHECK-NEXT:                 BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[TRUNC]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 4: i32.trunc_sat_f64_s — saturating signed truncation from f64.
  (func (export "trunc_sat_f64_s") (param f64) (result i32)
    local.get 0
    i32.trunc_sat_f64_s)
;; CHECK-LABEL: function wasm_func_4(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SAT:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmI32TruncSatF64S]{{.*}}, %[[A]]: any
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[SAT]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 5: i32.trunc_sat_f64_u — saturating unsigned truncation from f64.
  (func (export "trunc_sat_f64_u") (param f64) (result i32)
    local.get 0
    i32.trunc_sat_f64_u)
;; CHECK-LABEL: function wasm_func_5(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SAT:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmI32TruncSatF64U]{{.*}}, %[[A]]: any
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[SAT]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 6: i32.trunc_sat_f32_s — saturating signed truncation from f32.
  ;; f32 uses same builtin as f64 (wasmI32TruncSatF64S) in Phase 1.
  (func (export "trunc_sat_f32_s") (param f32) (result i32)
    local.get 0
    i32.trunc_sat_f32_s)
;; CHECK-LABEL: function wasm_func_6(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SAT:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmI32TruncSatF64S]{{.*}}, %[[A]]: any
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[SAT]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 7: i32.trunc_sat_f32_u — saturating unsigned truncation from f32.
  ;; f32 uses same builtin as f64 (wasmI32TruncSatF64U) in Phase 1.
  (func (export "trunc_sat_f32_u") (param f32) (result i32)
    local.get 0
    i32.trunc_sat_f32_u)
;; CHECK-LABEL: function wasm_func_7(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SAT:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmI32TruncSatF64U]{{.*}}, %[[A]]: any
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[SAT]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 8: f64.convert_i32_s — signed int to f64 via AsInt32Inst.
  (func (export "f64_convert_i32_s") (param i32) (result f64)
    local.get 0
    f64.convert_i32_s)
;; CHECK-LABEL: function wasm_func_8(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CONV:.*]] = AsInt32Inst (:number) %[[A]]: any
;; CHECK-NEXT:                BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[CONV]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 9: f64.convert_i32_u — unsigned int to f64 via AsUint32Inst.
  (func (export "f64_convert_i32_u") (param i32) (result f64)
    local.get 0
    f64.convert_i32_u)
;; CHECK-LABEL: function wasm_func_9(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CONV:.*]] = AsUint32Inst (:number) %[[A]]: any
;; CHECK-NEXT:                BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[CONV]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 10: f32.convert_i32_s — signed int to f32 via AsInt32Inst + Math.fround.
  (func (export "f32_convert_i32_s") (param i32) (result f32)
    local.get 0
    f32.convert_i32_s)
;; CHECK-LABEL: function wasm_func_10(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CONV:.*]] = AsInt32Inst (:number) %[[A]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[CONV]]: number
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 11: f32.convert_i32_u — unsigned int to f32 via AsUint32Inst + Math.fround.
  (func (export "f32_convert_i32_u") (param i32) (result f32)
    local.get 0
    f32.convert_i32_u)
;; CHECK-LABEL: function wasm_func_11(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CONV:.*]] = AsUint32Inst (:number) %[[A]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[CONV]]: number
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 12: i32.reinterpret_f32 — bitcast f32 to i32.
  (func (export "i32_reinterpret_f32") (param f32) (result i32)
    local.get 0
    i32.reinterpret_f32)
;; CHECK-LABEL: function wasm_func_12(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[REINT:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmI32ReinterpretF32]{{.*}}, %[[A]]: any
;; CHECK-NEXT:                 BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[REINT]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 13: f32.reinterpret_i32 — bitcast i32 to f32.
  (func (export "f32_reinterpret_i32") (param i32) (result f32)
    local.get 0
    f32.reinterpret_i32)
)
;; CHECK-LABEL: function wasm_func_13(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[REINT:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmF32ReinterpretI32]{{.*}}, %[[A]]: any
;; CHECK-NEXT:                 BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[REINT]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end
