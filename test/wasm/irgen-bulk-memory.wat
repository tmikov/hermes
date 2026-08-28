;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for bulk memory operations:
;; memory.fill, memory.copy, memory.init, data.drop.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (memory 1)
  (data "\01\02\03\04\05")

  (func $test_fill (param i32 i32 i32)
    (memory.fill (local.get 0) (local.get 1) (local.get 2))
  )

  (func $test_copy (param i32 i32 i32)
    (memory.copy (local.get 0) (local.get 1) (local.get 2))
  )

  (func $test_init (param i32 i32 i32)
    (memory.init 0 (local.get 0) (local.get 1) (local.get 2))
  )

  (func $test_drop
    (data.drop 0)
  )
)

;; CHECK-LABEL: function wasm_func_0(p0: number, p1: number, p2: number): undefined 
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmMemoryFill]

;; CHECK-LABEL: function wasm_func_1(p0: number, p1: number, p2: number): undefined 
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmMemoryCopy]

;; CHECK-LABEL: function wasm_func_2(p0: number, p1: number, p2: number): undefined 
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmMemoryInit]

;; CHECK-LABEL: function wasm_func_3(): undefined 
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmDataDrop]
