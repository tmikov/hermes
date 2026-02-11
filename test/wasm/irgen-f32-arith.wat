;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for f32 arithmetic operations (E.2).
;; f32 ops wrap their result in Math.fround for f32 precision.
;; Verifies that each operation produces the correct IR pattern with
;; correct data flow: params are loaded, fed to the operation, rounded
;; via Math.fround, and the result flows through phi to return.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: f32.add(a, b) — BinaryAddInst + Math.fround
  ;; The first function is checked exhaustively including param loading.
  (func $f32_add (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.add)
;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK: %BB0:
;; CHECK:   %[[L0_0:.*]] = AllocStackInst (:any)
;; CHECK:   %[[P0_0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:              StoreStackInst %[[P0_0]]: any, %[[L0_0]]: any
;; CHECK:   %[[L1_0:.*]] = AllocStackInst (:any)
;; CHECK:   %[[P1_0:.*]] = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:              StoreStackInst %[[P1_0]]: any, %[[L1_0]]: any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any) %[[L0_0]]: any
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any) %[[L1_0]]: any
;; CHECK-NEXT: %[[ADD:.*]] = BinaryAddInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[ADD]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 1: f32.sub(a, b) — BinarySubtractInst + Math.fround
  (func $f32_sub (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.sub)
;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SUB:.*]] = BinarySubtractInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[SUB]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 2: f32.mul(a, b) — BinaryMultiplyInst + Math.fround
  (func $f32_mul (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.mul)
;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[MUL:.*]] = BinaryMultiplyInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[MUL]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 3: f32.div(a, b) — BinaryDivideInst + Math.fround
  (func $f32_div (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.div)
;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[DIV:.*]] = BinaryDivideInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[DIV]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 4: f32.neg(a) — UnaryMinusInst + Math.fround
  (func $f32_neg (param f32) (result f32)
    local.get 0
    f32.neg)
;; CHECK-LABEL: function wasm_func_4(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[NEG:.*]] = UnaryMinusInst (:any) %[[A]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[NEG]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 5: f32.abs(a) — CallBuiltinInst [Math.abs] + Math.fround
  (func $f32_abs (param f32) (result f32)
    local.get 0
    f32.abs)
;; CHECK-LABEL: function wasm_func_5(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[ABS:.*]] = CallBuiltinInst (:any) [Math.abs]{{.*}}, %[[A]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[ABS]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 6: f32.sqrt(a) — CallBuiltinInst [Math.sqrt] + Math.fround
  (func $f32_sqrt (param f32) (result f32)
    local.get 0
    f32.sqrt)
;; CHECK-LABEL: function wasm_func_6(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SQRT:.*]] = CallBuiltinInst (:any) [Math.sqrt]{{.*}}, %[[A]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[SQRT]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 7: f32.ceil(a) — CallBuiltinInst [Math.ceil] + Math.fround
  (func $f32_ceil (param f32) (result f32)
    local.get 0
    f32.ceil)
;; CHECK-LABEL: function wasm_func_7(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[CEIL:.*]] = CallBuiltinInst (:any) [Math.ceil]{{.*}}, %[[A]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[CEIL]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 8: f32.floor(a) — CallBuiltinInst [Math.floor] + Math.fround
  (func $f32_floor (param f32) (result f32)
    local.get 0
    f32.floor)
;; CHECK-LABEL: function wasm_func_8(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[FLOOR:.*]] = CallBuiltinInst (:any) [Math.floor]{{.*}}, %[[A]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[FLOOR]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 9: f32.trunc(a) — CallBuiltinInst [Math.trunc] + Math.fround
  (func $f32_trunc (param f32) (result f32)
    local.get 0
    f32.trunc)
;; CHECK-LABEL: function wasm_func_9(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[TRUNC:.*]] = CallBuiltinInst (:any) [Math.trunc]{{.*}}, %[[A]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[TRUNC]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 10: f32.nearest(a) — CallBuiltinInst [Math.round] + Math.fround
  (func $f32_nearest (param f32) (result f32)
    local.get 0
    f32.nearest)
;; CHECK-LABEL: function wasm_func_10(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[ROUND:.*]] = CallBuiltinInst (:any) [Math.round]{{.*}}, %[[A]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[ROUND]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 11: f32.min(a, b) — CallBuiltinInst [Math.min] + Math.fround
  (func $f32_min (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.min)
;; CHECK-LABEL: function wasm_func_11(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[MIN:.*]] = CallBuiltinInst (:any) [Math.min]{{.*}}, %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[MIN]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 12: f32.max(a, b) — CallBuiltinInst [Math.max] + Math.fround
  (func $f32_max (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.max)
;; CHECK-LABEL: function wasm_func_12(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[MAX:.*]] = CallBuiltinInst (:any) [Math.max]{{.*}}, %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[MAX]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 13: f32.demote_f64 — CallBuiltinInst [Math.fround]
  (func $f32_demote (param f64) (result f32)
    local.get 0
    f32.demote_f64)
;; CHECK-LABEL: function wasm_func_13(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[FR:.*]] = CallBuiltinInst (:any) [Math.fround]{{.*}}, %[[A]]: any
;; CHECK-NEXT:              BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[FR]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 14: f64.promote_f32 — no-op (f32 values are already f64 doubles).
  ;; Just loads param and returns it through phi.
  (func $f64_promote (param f32) (result f64)
    local.get 0
    f64.promote_f32)
)
;; CHECK-LABEL: function wasm_func_14(p0: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT:           BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[A]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end
