;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for i32 arithmetic operations.
;; Verifies that each operation produces the correct IR pattern.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir %t.wasm | %FileCheck %s

;; i32.add(a, b) → AsInt32(BinaryAdd(a, b))
;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryAddInst
;; CHECK:   %{{[0-9]+}} = AsInt32Inst
;; CHECK:        ReturnInst %{{[0-9]+}}: number
;; CHECK-NEXT: function_end

;; i32.sub(a, b) → AsInt32(BinarySubtract(a, b))
;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinarySubtractInst
;; CHECK:   %{{[0-9]+}} = AsInt32Inst
;; CHECK:        ReturnInst %{{[0-9]+}}: number
;; CHECK-NEXT: function_end

;; i32.mul(a, b) → CallBuiltinInst(Math.imul, a, b)
;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = CallBuiltinInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.and(a, b)
;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryAndInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.or(a, b)
;; CHECK-LABEL: function wasm_func_4(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryOrInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.xor(a, b)
;; CHECK-LABEL: function wasm_func_5(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryXorInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.shl(a, b)
;; CHECK-LABEL: function wasm_func_6(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryLeftShiftInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.shr_s(a, b)
;; CHECK-LABEL: function wasm_func_7(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryRightShiftInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; i32.shr_u(a, b)
;; CHECK-LABEL: function wasm_func_8(p0: any, p1: any): any
;; CHECK:   %{{[0-9]+}} = BinaryUnsignedRightShiftInst
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

(module
  ;; func 0: i32.add
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)

  ;; func 1: i32.sub
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.sub)

  ;; func 2: i32.mul
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.mul)

  ;; func 3: i32.and
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.and)

  ;; func 4: i32.or
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.or)

  ;; func 5: i32.xor
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.xor)

  ;; func 6: i32.shl
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.shl)

  ;; func 7: i32.shr_s
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.shr_s)

  ;; func 8: i32.shr_u
  (func (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.shr_u))
