;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for i64 arithmetic operations (G.3).
;; i64 values are represented as two i32 stack slots [lo, hi].
;; Binary ops use CallBuiltinInst + HiResult pattern; and/or/xor are inline.
;; NOTE: Uses only i64 constants, not i64 params (G.5 needed for i64 locals).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; i64.add uses CallBuiltinInst
  (func $add (result i32)
    i64.const 100
    i64.const 200
    i64.add
    i64.eqz)

;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64Add]
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiResult]
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64Eqz]

  ;; i64.and uses inline BinaryAndInst on both lo and hi
  (func $and (result i32)
    i64.const 0xFF00
    i64.const 0x0FFF
    i64.and
    i64.eqz)

;; CHECK-LABEL: function wasm_func_1
;; CHECK: BinaryAndInst
;; CHECK: BinaryAndInst

  ;; i64.or uses inline BinaryOrInst on both lo and hi
  (func $or (result i32)
    i64.const 0xFF00
    i64.const 0x00FF
    i64.or
    i64.eqz)

;; CHECK-LABEL: function wasm_func_2
;; CHECK: BinaryOrInst
;; CHECK: BinaryOrInst

  ;; i64.xor uses inline BinaryXorInst on both lo and hi
  (func $xor (result i32)
    i64.const 0xFF
    i64.const 0x0F
    i64.xor
    i64.eqz)

;; CHECK-LABEL: function wasm_func_3
;; CHECK: BinaryXorInst
;; CHECK: BinaryXorInst

  ;; i64.shl uses CallBuiltinInst
  (func $shl (result i32)
    i64.const 1
    i64.const 32
    i64.shl
    i64.eqz)

;; CHECK-LABEL: function wasm_func_4
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64Shl]
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiResult]

  ;; i64.clz returns i64 (but result is always in [0,64])
  (func $clz (result i32)
    i64.const 1
    i64.clz
    i64.eqz)

;; CHECK-LABEL: function wasm_func_5
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64Clz]

  ;; i64.eq returns i32 (not i64)
  (func $eq (result i32)
    i64.const 42
    i64.const 42
    i64.eq)

;; CHECK-LABEL: function wasm_func_6
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64Eq]

  ;; i64.eqz returns i32
  (func $eqz (result i32)
    i64.const 0
    i64.eqz)

;; CHECK-LABEL: function wasm_func_7
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64Eqz]

  ;; i64.sub
  (func $sub (result i32)
    i64.const 500
    i64.const 200
    i64.sub
    i64.const 300
    i64.eq)

;; CHECK-LABEL: function wasm_func_8
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64Sub]

  ;; i64.mul
  (func $mul (result i32)
    i64.const 6
    i64.const 7
    i64.mul
    i64.const 42
    i64.eq)

;; CHECK-LABEL: function wasm_func_9
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64Mul]
)
