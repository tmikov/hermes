;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for f64 comparison operations.
;; Verifies that each comparison produces a compare + BinaryOrInst (bool→i32).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; f64.eq → BinaryStrictlyEqualInst + BinaryOrInst (boolean to i32)
;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryStrictlyEqualInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.ne → BinaryStrictlyNotEqualInst + BinaryOrInst
;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryStrictlyNotEqualInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.lt → BinaryLessThanInst + BinaryOrInst
;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryLessThanInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.gt → BinaryGreaterThanInst + BinaryOrInst
;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryGreaterThanInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.le → BinaryLessThanOrEqualInst + BinaryOrInst
;; CHECK-LABEL: function wasm_func_4(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryLessThanOrEqualInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

;; f64.ge → BinaryGreaterThanOrEqualInst + BinaryOrInst
;; CHECK-LABEL: function wasm_func_5(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryGreaterThanOrEqualInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst %{{[0-9]+}}
;; CHECK-NEXT: function_end

(module
  ;; func 0: f64.eq
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.eq)

  ;; func 1: f64.ne
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.ne)

  ;; func 2: f64.lt
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.lt)

  ;; func 3: f64.gt
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.gt)

  ;; func 4: f64.le
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.le)

  ;; func 5: f64.ge
  (func (param f64 f64) (result i32)
    local.get 0
    local.get 1
    f64.ge))
