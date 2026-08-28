;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for f64 arithmetic operations.
;; Verifies that each operation produces the correct IR pattern.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: f64.add
  (func (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.add)

;; CHECK-LABEL: function wasm_func_0(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = FAddInst (:number) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 1: f64.sub
  (func (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.sub)

;; CHECK-LABEL: function wasm_func_1(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = FSubtractInst (:number) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 2: f64.mul
  (func (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.mul)

;; CHECK-LABEL: function wasm_func_2(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = FMultiplyInst (:number) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 3: f64.div
  (func (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.div)

;; CHECK-LABEL: function wasm_func_3(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = FDivideInst (:number) %[[A]]: number, %[[B]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 4: f64.neg
  (func (param f64) (result f64)
    local.get 0
    f64.neg)

;; CHECK-LABEL: function wasm_func_4(p0: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = FNegate (:number) %[[A]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 5: f64.abs
  (func (param f64) (result f64)
    local.get 0
    f64.abs)

;; CHECK-LABEL: function wasm_func_5(p0: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:any) [Math.abs]{{.*}}, %[[A]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 6: f64.sqrt
  (func (param f64) (result f64)
    local.get 0
    f64.sqrt)

;; CHECK-LABEL: function wasm_func_6(p0: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:any) [Math.sqrt]{{.*}}, %[[A]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 7: f64.ceil
  (func (param f64) (result f64)
    local.get 0
    f64.ceil)

;; CHECK-LABEL: function wasm_func_7(p0: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:any) [Math.ceil]{{.*}}, %[[A]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 8: f64.floor
  (func (param f64) (result f64)
    local.get 0
    f64.floor)

;; CHECK-LABEL: function wasm_func_8(p0: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:any) [Math.floor]{{.*}}, %[[A]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 9: f64.trunc
  (func (param f64) (result f64)
    local.get 0
    f64.trunc)

;; CHECK-LABEL: function wasm_func_9(p0: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:any) [Math.trunc]{{.*}}, %[[A]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 10: f64.nearest
  (func (param f64) (result f64)
    local.get 0
    f64.nearest)

;; CHECK-LABEL: function wasm_func_10(p0: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:number) [HermesBuiltin.wasmNearest]{{.*}}, %[[A]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[R]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 11: f64.min
  (func (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.min)

;; CHECK-LABEL: function wasm_func_11(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:any) [Math.min]{{.*}}, %[[A]]: number, %[[B]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 12: f64.max
  (func (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.max)

;; CHECK-LABEL: function wasm_func_12(p0: number, p1: number): number 
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:number)
;; CHECK-NEXT: %[[R:.*]] = CallBuiltinInst (:any) [Math.max]{{.*}}, %[[A]]: number, %[[B]]: number
;; CHECK-NEXT:             BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[R]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end

  ;; func 13: f64.promote_f32 - should be a no-op (value stays as-is)
  (func (param f32) (result f64)
    local.get 0
    f64.promote_f32))

;; CHECK-LABEL: function wasm_func_13(p0: number): number 
;; CHECK-NOT:   FPromote
;; CHECK:   %[[A:.*]] = LoadStackInst (:number)
;; CHECK-NEXT:           BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[A]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
