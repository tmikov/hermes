;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for i64 inline conversions (G.4a).
;; i32.wrap_i64, i64.extend_i32_s/u, i64.extend8/16/32_s.
;; NOTE: Uses only i64 constants, not i64 params (G.5 needed for i64 locals).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; i32.wrap_i64: takes lo32, discards hi32. Result is i32 (single value).
  (func $wrap (result i32)
    i64.const 0x1FFFFFFFF  ;; lo=0xFFFFFFFF, hi=1
    i32.wrap_i64)

;; CHECK-LABEL: function wasm_func_0(): number 
;; CHECK: PhiInst (:number) -1: number
;; CHECK-NEXT: ReturnInst %{{.*}}: number

  ;; i64.extend_i32_s: sign-extend positive i32 to i64.
  ;; 42 -> lo=42, hi=0 (positive, sign bit is 0).
  (func $extend_s_pos (result i32)
    i32.const 42
    i64.extend_i32_s
    i64.eqz)

;; CHECK-LABEL: function wasm_func_1
;; CHECK: AsInt32Inst
;; CHECK: BinaryRightShiftInst
;; CHECK: BinaryOrInst
;; CHECK: FEqualInst
;; CHECK: AsInt32Inst

  ;; i64.extend_i32_s: sign-extend negative i32 to i64.
  ;; -1 -> lo=0xFFFFFFFF, hi=0xFFFFFFFF (sign bit propagated).
  (func $extend_s_neg (result i32)
    i32.const -1
    i64.extend_i32_s
    i64.eqz)

;; CHECK-LABEL: function wasm_func_2
;; CHECK: AsInt32Inst
;; CHECK: BinaryRightShiftInst
;; CHECK: BinaryOrInst
;; CHECK: FEqualInst
;; CHECK: AsInt32Inst

  ;; i64.extend_i32_u: zero-extend i32 to i64.
  ;; -1 (0xFFFFFFFF) -> lo=0xFFFFFFFF, hi=0.
  (func $extend_u (result i32)
    i32.const -1
    i64.extend_i32_u
    i64.eqz)

;; CHECK-LABEL: function wasm_func_3
;; CHECK-NOT: BinaryRightShiftInst
;; CHECK: AsInt32Inst
;; CHECK: BinaryOrInst
;; CHECK: FEqualInst
;; CHECK: AsInt32Inst

  ;; i64.extend8_s: sign-extend lowest 8 bits of i64.
  (func $ext8s (result i32)
    i64.const 0x80  ;; lo=128 (0x80), hi=0
    i64.extend8_s
    i64.eqz)

;; CHECK-LABEL: function wasm_func_4
;; CHECK: BinaryLeftShiftInst
;; CHECK: BinaryRightShiftInst
;; CHECK: BinaryRightShiftInst
;; CHECK: BinaryOrInst
;; CHECK: FEqualInst
;; CHECK: AsInt32Inst

  ;; i64.extend16_s: sign-extend lowest 16 bits of i64.
  (func $ext16s (result i32)
    i64.const 0x8000  ;; lo=32768 (0x8000), hi=0
    i64.extend16_s
    i64.eqz)

;; CHECK-LABEL: function wasm_func_5
;; CHECK: BinaryLeftShiftInst
;; CHECK: BinaryRightShiftInst
;; CHECK: BinaryRightShiftInst
;; CHECK: BinaryOrInst
;; CHECK: FEqualInst
;; CHECK: AsInt32Inst

  ;; i64.extend32_s: sign-extend lowest 32 bits of i64.
  (func $ext32s (result i32)
    i64.const 0x100000000  ;; lo=0, hi=1
    i64.extend32_s
    i64.eqz)

;; CHECK-LABEL: function wasm_func_6
;; CHECK: AsInt32Inst
;; CHECK: BinaryRightShiftInst
;; CHECK: BinaryOrInst
;; CHECK: FEqualInst
;; CHECK: AsInt32Inst
)
