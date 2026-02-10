;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (memory 1)

  ;; Test 1: i32.load with align=1 (unaligned) uses byte-assembly path.
  ;; Natural alignment for i32.load is 4 (log2=2), so align=1 (log2=0) triggers
  ;; the byte-assembly path.
  (func (export "load_i32_unaligned") (param i32) (result i32)
    (i32.load align=1 (local.get 0))
  )
  ;; CHECK-LABEL: function wasm_func_0(p0: any): any
  ;; The unaligned path loads individual bytes from HEAPU8 and assembles them.
  ;; CHECK: [%VS0.HEAPU8]
  ;; CHECK: LoadPropertyInst
  ;; Byte 0 loaded, OOB check follows, then bytes 1-3 loaded and assembled.
  ;; CHECK: BinaryStrictlyEqualInst
  ;; CHECK: CondBranchInst
  ;; After OOB check, assemble bytes with shifts and OR.
  ;; CHECK: BinaryAddInst
  ;; CHECK: LoadPropertyInst
  ;; CHECK: BinaryLeftShiftInst
  ;; CHECK: BinaryOrInst

  ;; Test 2: i32.store with align=1 (unaligned) uses byte-decomposition path.
  (func (export "store_i32_unaligned") (param i32 i32)
    (i32.store align=1 (local.get 0) (local.get 1))
  )
  ;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
  ;; The unaligned store decomposes value into bytes via AND and shifts.
  ;; CHECK: [%VS0.HEAPU8]
  ;; CHECK: BinaryAndInst
  ;; CHECK: StorePropertyStrictInst
  ;; Byte 1: shift right by 8, mask, store.
  ;; CHECK: BinaryUnsignedRightShiftInst
  ;; CHECK: BinaryAndInst
  ;; CHECK: StorePropertyStrictInst

  ;; Test 3: i32.load with natural alignment (align=4) uses typed array path.
  (func (export "load_i32_aligned") (param i32) (result i32)
    (i32.load (local.get 0))
  )
  ;; CHECK-LABEL: function wasm_func_2(p0: any): any
  ;; Aligned path uses shift-right-by-2 then loads from HEAP32.
  ;; CHECK: BinaryUnsignedRightShiftInst
  ;; CHECK: [%VS0.HEAP32]
  ;; CHECK: LoadPropertyInst

  ;; Test 4: f64.load with align=1 (unaligned) uses byte-assembly + reinterpret.
  (func (export "load_f64_unaligned") (param i32) (result f64)
    (f64.load align=1 (local.get 0))
  )
  ;; CHECK-LABEL: function wasm_func_3(p0: any): any
  ;; f64 unaligned load: assemble lo 4 bytes, then hi 4 bytes, reinterpret.
  ;; CHECK: [%VS0.HEAPU8]
  ;; CHECK: LoadPropertyInst
  ;; Should see two groups of byte-assembly (for lo and hi halves),
  ;; followed by a reinterpret call.
  ;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmF64ReinterpretI64]
)
