;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: i64 locals and parameters (G.5).
;; i64 params get 2 JSDynamicParams (lo, hi) and 2 AllocStackInst.
;; i64 locals get 2 AllocStackInst.
;; local.get/set/tee operate on both lo and hi slots.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Identity function for i64 param
  (func $identity (param i64) (result i64)
    local.get 0)

  ;; Function with i64 local: store and load
  (func $local_i64 (result i64)
    (local i64)
    i64.const 42
    local.set 0
    local.get 0)

  ;; Mixed params: i32, i64, i32 -- verify slot indexing
  (func $mixed (param i32) (param i64) (param i32) (result i64)
    local.get 1))

;; -- identity: i64 param has 2 JSDynamicParams and 2 AllocStackInst --
;; CHECK-LABEL: function wasm_func_0(p0_lo: any, p0_hi: any): any
;; CHECK-NEXT: %BB0:
;; CHECK:        AllocStackInst (:any) $local_0_lo
;; CHECK:        AllocStackInst (:any) $local_0_hi
;; CHECK:        LoadParamInst (:any) %p0_lo
;; CHECK:        LoadParamInst (:any) %p0_hi
;; local.get 0 loads both lo and hi, then branches to exit
;; CHECK:        LoadStackInst
;; CHECK:        LoadStackInst
;; CHECK:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; -- exit block: 2 phis (lo, hi), stash hi, return lo --
;; CHECK-NEXT:   %{{.*}} = PhiInst
;; CHECK-NEXT:   %{{.*}} = PhiInst
;; CHECK-NEXT:   %{{.*}} = CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiStash]
;; CHECK:                  ReturnInst
;; CHECK-NEXT: function_end

;; -- local_i64: i64 declared local has 2 AllocStackInst slots --
;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK-NEXT: %BB0:
;; CHECK:        AllocStackInst (:any) $local_0_lo
;; CHECK:        AllocStackInst (:any) $local_0_hi
;; init lo=0, hi=0
;; CHECK:        StoreStackInst 0
;; CHECK:        StoreStackInst 0
;; local.set 0 stores both lo=42 and hi=0
;; CHECK:        StoreStackInst 42
;; CHECK:        StoreStackInst 0
;; local.get 0 loads both
;; CHECK:        LoadStackInst
;; CHECK:        LoadStackInst
;; CHECK:        BranchInst
;; exit block: stash hi, return lo
;; CHECK:        CallBuiltinInst {{.*}}[HermesBuiltin.wasmI64HiStash]
;; CHECK:        ReturnInst
;; CHECK-NEXT: function_end

;; -- mixed: i32 param, i64 param, i32 param --
;; CHECK-LABEL: function wasm_func_2(p0: any, p1_lo: any, p1_hi: any, p2: any): any
;; CHECK-NEXT: %BB0:
;; Verify interleaved alloc+load pattern:
;; i32 param 0
;; CHECK:        AllocStackInst (:any) $local_0
;; CHECK:        LoadParamInst (:any) %p0:
;; i64 param 1 (2 allocs + 2 loads)
;; CHECK:        AllocStackInst (:any) $local_1_lo
;; CHECK:        AllocStackInst (:any) $local_1_hi
;; CHECK:        LoadParamInst (:any) %p1_lo
;; CHECK:        LoadParamInst (:any) %p1_hi
;; i32 param 2
;; CHECK:        AllocStackInst (:any) $local_2
;; CHECK:        LoadParamInst (:any) %p2:
