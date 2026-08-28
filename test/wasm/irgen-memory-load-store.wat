;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; IR generation test for memory load and store operations.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (memory 1)

  ;; i32 store and load at offset 0 (func 0).
  (func (export "store_load_i32") (param i32) (result i32)
    i32.const 0
    local.get 0
    i32.store offset=0
    i32.const 0
    i32.load offset=0
  )

  ;; f64 store and load (func 1).
  (func (export "store_load_f64") (param f64) (result f64)
    i32.const 8
    local.get 0
    f64.store offset=0
    i32.const 8
    f64.load offset=0
  )

  ;; i32.load with a non-zero offset (func 2).
  (func (export "load_offset") (param i32) (result i32)
    local.get 0
    i32.load offset=4
  )
)

;; Check that the top-level function creates ArrayBuffer and typed array views.
;; CHECK-LABEL: function global(): any
;; CHECK: TryLoadGlobalPropertyInst {{.*}}"ArrayBuffer"
;; CHECK: TryLoadGlobalPropertyInst {{.*}}"Int8Array"
;; CHECK: TryLoadGlobalPropertyInst {{.*}}"Int32Array"

;; Check the i32 store/load function (wasm_func_0).
;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK: LoadFrameInst {{.*}}[%VS0.HEAP32]
;; CHECK: StorePropertyStrictInst
;; CHECK: LoadFrameInst {{.*}}[%VS0.HEAP32]
;; CHECK: LoadPropertyInst
;; CHECK: BinaryStrictlyEqualInst
;; CHECK: CondBranchInst

;; Check the f64 store/load function (wasm_func_1).
;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; CHECK: LoadFrameInst {{.*}}[%VS0.HEAPF64]
;; CHECK: StorePropertyStrictInst
;; CHECK: LoadFrameInst {{.*}}[%VS0.HEAPF64]
;; CHECK: LoadPropertyInst

;; Check the load with offset function (wasm_func_2).
;; CHECK-LABEL: function wasm_func_2(p0: any): any
;; CHECK: BinaryAddInst
;; CHECK: BinaryUnsignedRightShiftInst
;; CHECK: LoadFrameInst {{.*}}[%VS0.HEAP32]
;; CHECK: LoadPropertyInst
