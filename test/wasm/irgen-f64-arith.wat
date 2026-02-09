;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for f64 arithmetic operations.
;; Verifies that each operation produces the correct IR pattern.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; f64.add(a, b) → BinaryAddInst
;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryAddInst
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.sub(a, b) → BinarySubtractInst
;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinarySubtractInst
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.mul(a, b) → BinaryMultiplyInst
;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryMultiplyInst
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.div(a, b) → BinaryDivideInst
;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryDivideInst
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.neg(a) → UnaryMinusInst
;; CHECK-LABEL: function wasm_func_4(p0: any): any
;; CHECK:   %{{[0-9]+}} = UnaryMinusInst
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.abs(a) → CallBuiltinInst {{.*}}[Math.abs]
;; CHECK-LABEL: function wasm_func_5(p0: any): any
;; CHECK:   %{{[0-9]+}} = CallBuiltinInst {{.*}}[Math.abs]
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.sqrt(a) → CallBuiltinInst {{.*}}[Math.sqrt]
;; CHECK-LABEL: function wasm_func_6(p0: any): any
;; CHECK:   %{{[0-9]+}} = CallBuiltinInst {{.*}}[Math.sqrt]
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.ceil(a) → CallBuiltinInst {{.*}}[Math.ceil]
;; CHECK-LABEL: function wasm_func_7(p0: any): any
;; CHECK:   %{{[0-9]+}} = CallBuiltinInst {{.*}}[Math.ceil]
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.floor(a) → CallBuiltinInst {{.*}}[Math.floor]
;; CHECK-LABEL: function wasm_func_8(p0: any): any
;; CHECK:   %{{[0-9]+}} = CallBuiltinInst {{.*}}[Math.floor]
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.trunc(a) → CallBuiltinInst {{.*}}[Math.trunc]
;; CHECK-LABEL: function wasm_func_9(p0: any): any
;; CHECK:   %{{[0-9]+}} = CallBuiltinInst {{.*}}[Math.trunc]
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.nearest(a) → CallBuiltinInst {{.*}}[Math.round]
;; CHECK-LABEL: function wasm_func_10(p0: any): any
;; CHECK:   %{{[0-9]+}} = CallBuiltinInst {{.*}}[Math.round]
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.min(a, b) → CallBuiltinInst {{.*}}[Math.min]
;; CHECK-LABEL: function wasm_func_11(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = CallBuiltinInst {{.*}}[Math.min]
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.max(a, b) → CallBuiltinInst {{.*}}[Math.max]
;; CHECK-LABEL: function wasm_func_12(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = CallBuiltinInst {{.*}}[Math.max]
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.promote_f32 — should be a no-op (value stays as-is)
;; CHECK-LABEL: function wasm_func_13(p0: any): any
;; CHECK-NOT:   FPromote
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

(module
  ;; func 0: f64.add
  (func (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.add)

  ;; func 1: f64.sub
  (func (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.sub)

  ;; func 2: f64.mul
  (func (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.mul)

  ;; func 3: f64.div
  (func (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.div)

  ;; func 4: f64.neg
  (func (param f64) (result f64)
    local.get 0
    f64.neg)

  ;; func 5: f64.abs
  (func (param f64) (result f64)
    local.get 0
    f64.abs)

  ;; func 6: f64.sqrt
  (func (param f64) (result f64)
    local.get 0
    f64.sqrt)

  ;; func 7: f64.ceil
  (func (param f64) (result f64)
    local.get 0
    f64.ceil)

  ;; func 8: f64.floor
  (func (param f64) (result f64)
    local.get 0
    f64.floor)

  ;; func 9: f64.trunc
  (func (param f64) (result f64)
    local.get 0
    f64.trunc)

  ;; func 10: f64.nearest
  (func (param f64) (result f64)
    local.get 0
    f64.nearest)

  ;; func 11: f64.min
  (func (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.min)

  ;; func 12: f64.max
  (func (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.max)

  ;; func 13: f64.promote_f32 (no-op)
  (func (param f32) (result f64)
    local.get 0
    f64.promote_f32))
