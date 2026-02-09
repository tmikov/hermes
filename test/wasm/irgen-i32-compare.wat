;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for i32 comparison operations.
;; Verifies that each operation produces the correct IR pattern:
;; comparison → boolean → BitOr(bool, 0) → i32.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; i32.eq(a, b) → BinaryStrictlyEqual, then BitOr to convert to i32
;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryStrictlyEqualInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.ne(a, b) → BinaryStrictlyNotEqual, then BitOr
;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryStrictlyNotEqualInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.lt_s(a, b) → AsInt32 both, BinaryLessThan, BitOr
;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = AsInt32Inst
;; CHECK:   %{{[0-9]+}} = AsInt32Inst
;; CHECK:   %{{[0-9]+}} = BinaryLessThanInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.gt_s(a, b) → AsInt32 both, BinaryGreaterThan, BitOr
;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = AsInt32Inst
;; CHECK:   %{{[0-9]+}} = AsInt32Inst
;; CHECK:   %{{[0-9]+}} = BinaryGreaterThanInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.le_s(a, b) → AsInt32 both, BinaryLessThanOrEqual, BitOr
;; CHECK-LABEL: function wasm_func_4(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = AsInt32Inst
;; CHECK:   %{{[0-9]+}} = AsInt32Inst
;; CHECK:   %{{[0-9]+}} = BinaryLessThanOrEqualInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.ge_s(a, b) → AsInt32 both, BinaryGreaterThanOrEqual, BitOr
;; CHECK-LABEL: function wasm_func_5(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = AsInt32Inst
;; CHECK:   %{{[0-9]+}} = AsInt32Inst
;; CHECK:   %{{[0-9]+}} = BinaryGreaterThanOrEqualInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.lt_u(a, b) → AsUint32 both, BinaryLessThan, BitOr
;; CHECK-LABEL: function wasm_func_6(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = AsUint32Inst
;; CHECK:   %{{[0-9]+}} = AsUint32Inst
;; CHECK:   %{{[0-9]+}} = BinaryLessThanInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.gt_u(a, b) → AsUint32 both, BinaryGreaterThan, BitOr
;; CHECK-LABEL: function wasm_func_7(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = AsUint32Inst
;; CHECK:   %{{[0-9]+}} = AsUint32Inst
;; CHECK:   %{{[0-9]+}} = BinaryGreaterThanInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.le_u(a, b) → AsUint32 both, BinaryLessThanOrEqual, BitOr
;; CHECK-LABEL: function wasm_func_8(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = AsUint32Inst
;; CHECK:   %{{[0-9]+}} = AsUint32Inst
;; CHECK:   %{{[0-9]+}} = BinaryLessThanOrEqualInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.ge_u(a, b) → AsUint32 both, BinaryGreaterThanOrEqual, BitOr
;; CHECK-LABEL: function wasm_func_9(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = AsUint32Inst
;; CHECK:   %{{[0-9]+}} = AsUint32Inst
;; CHECK:   %{{[0-9]+}} = BinaryGreaterThanOrEqualInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.eqz(a) → BinaryStrictlyEqual(a, 0), BitOr
;; CHECK-LABEL: function wasm_func_10(p0: any): any
;; CHECK:   %{{[0-9]+}} = BinaryStrictlyEqualInst
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

(module
  ;; func 0: i32.eq
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.eq)

  ;; func 1: i32.ne
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.ne)

  ;; func 2: i32.lt_s
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.lt_s)

  ;; func 3: i32.gt_s
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.gt_s)

  ;; func 4: i32.le_s
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.le_s)

  ;; func 5: i32.ge_s
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.ge_s)

  ;; func 6: i32.lt_u
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.lt_u)

  ;; func 7: i32.gt_u
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.gt_u)

  ;; func 8: i32.le_u
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.le_u)

  ;; func 9: i32.ge_u
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.ge_u)

  ;; func 10: i32.eqz (unary)
  (func (param i32) (result i32)
    local.get 0
    i32.eqz))
