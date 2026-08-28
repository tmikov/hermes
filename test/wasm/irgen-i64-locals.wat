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
;; local.get 0 loads both lo and hi, then branches to exit
;; -- exit block: 2 phis (lo, hi), stash hi, return lo --

;; -- local_i64: i64 declared local has 2 AllocStackInst slots --
;; init lo=0, hi=0
;; local.set 0 stores both lo=42 and hi=0
;; local.get 0 loads both
;; exit block: stash hi, return lo

;; -- mixed: i32 param, i64 param, i32 param --
;; Verify interleaved alloc+load pattern:
;; i32 param 0
;; i64 param 1 (2 allocs + 2 loads)
;; i32 param 2

;; CHECK: scope %VS0 [wasm_type_id_0: any, wasm_type_id_1: any, wasm_type_id_2: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any, closure_2: any]
;; CHECK-EMPTY:
;; CHECK-NEXT: function global(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:   %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %__wasm_instantiate__(): functionCode
;; CHECK-NEXT:   %2 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK-NEXT:   %3 = CreateThisInst (:any) %2: any, %2: any, empty: any
;; CHECK-NEXT:   %4 = CallInst (:any) %2: any, empty: any, false: boolean, empty: any, %2: any, %3: any, 0: number
;; CHECK-NEXT:   %5 = GetConstructedObjectInst (:object) %3: any, %4: any
;; CHECK-NEXT:   %6 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK-NEXT:   %7 = CreateThisInst (:any) %6: any, %6: any, empty: any
;; CHECK-NEXT:   %8 = CallInst (:any) %6: any, empty: any, false: boolean, empty: any, %6: any, %7: any, 0: number
;; CHECK-NEXT:   %9 = GetConstructedObjectInst (:object) %7: any, %8: any
;; CHECK-NEXT:   %10 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst %1: object, %10: object, "instantiate": string
;; CHECK-NEXT:         StorePropertyStrictInst %5: object, %10: object, "exportDescs": string
;; CHECK-NEXT:         StorePropertyStrictInst %9: object, %10: object, "importDescs": string
;; CHECK-NEXT:         ReturnInst %10: object
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_0(retbuf_I: any, retbuf_F: any, p0_lo: any, p0_hi: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %retbuf_I: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %retbuf_F: any
;; CHECK-NEXT:   %3 = AllocStackInst (:any) $local_0_lo: any
;; CHECK-NEXT:   %4 = AllocStackInst (:any) $local_0_hi: any
;; CHECK-NEXT:   %5 = LoadParamInst (:any) %p0_lo: any
;; CHECK-NEXT:        StoreStackInst %5: any, %3: any
;; CHECK-NEXT:   %7 = LoadParamInst (:any) %p0_hi: any
;; CHECK-NEXT:        StoreStackInst %7: any, %4: any
;; CHECK-NEXT:   %9 = LoadStackInst (:any) %3: any
;; CHECK-NEXT:   %10 = LoadStackInst (:any) %4: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %12 = PhiInst (:any) %9: any, %BB0
;; CHECK-NEXT:   %13 = PhiInst (:any) %10: any, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %12: any, %1: any, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %13: any, %1: any, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_1(retbuf_I: any, retbuf_F: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %retbuf_I: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %retbuf_F: any
;; CHECK-NEXT:   %3 = AllocStackInst (:any) $local_0_lo: any
;; CHECK-NEXT:   %4 = AllocStackInst (:any) $local_0_hi: any
;; CHECK-NEXT:        StoreStackInst 0: number, %3: any
;; CHECK-NEXT:        StoreStackInst 0: number, %4: any
;; CHECK-NEXT:        StoreStackInst 42: number, %3: any
;; CHECK-NEXT:        StoreStackInst 0: number, %4: any
;; CHECK-NEXT:   %9 = LoadStackInst (:any) %3: any
;; CHECK-NEXT:   %10 = LoadStackInst (:any) %4: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %12 = PhiInst (:any) %9: any, %BB0
;; CHECK-NEXT:   %13 = PhiInst (:any) %10: any, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %12: any, %1: any, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %13: any, %1: any, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_2(retbuf_I: any, retbuf_F: any, p0: any, p1_lo: any, p1_hi: any, p2: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %retbuf_I: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %retbuf_F: any
;; CHECK-NEXT:   %3 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %4: any, %3: any
;; CHECK-NEXT:   %6 = AllocStackInst (:any) $local_1_lo: any
;; CHECK-NEXT:   %7 = AllocStackInst (:any) $local_1_hi: any
;; CHECK-NEXT:   %8 = LoadParamInst (:any) %p1_lo: any
;; CHECK-NEXT:        StoreStackInst %8: any, %6: any
;; CHECK-NEXT:   %10 = LoadParamInst (:any) %p1_hi: any
;; CHECK-NEXT:         StoreStackInst %10: any, %7: any
;; CHECK-NEXT:   %12 = AllocStackInst (:any) $local_2: any
;; CHECK-NEXT:   %13 = LoadParamInst (:any) %p2: any
;; CHECK-NEXT:         StoreStackInst %13: any, %12: any
;; CHECK-NEXT:   %15 = LoadStackInst (:any) %6: any
;; CHECK-NEXT:   %16 = LoadStackInst (:any) %7: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %18 = PhiInst (:any) %15: any, %BB0
;; CHECK-NEXT:   %19 = PhiInst (:any) %16: any, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %18: any, %1: any, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %19: any, %1: any, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function __wasm_instantiate__(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:   %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_0(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %1: object, [%VS0.closure_0]: any
;; CHECK-NEXT:   %3 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_1(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %3: object, [%VS0.closure_1]: any
;; CHECK-NEXT:   %5 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_2(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %5: object, [%VS0.closure_2]: any
;; CHECK-NEXT:   %7 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:   %8 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:   %9 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:   %10 = CreateThisInst (:any) %7: any, %7: any, empty: any
;; CHECK-NEXT:   %11 = CallInst (:any) %7: any, empty: any, false: boolean, empty: any, %7: any, %10: any, 8: number
;; CHECK-NEXT:   %12 = GetConstructedObjectInst (:object) %10: any, %11: any
;; CHECK-NEXT:   %13 = CreateThisInst (:any) %8: any, %8: any, empty: any
;; CHECK-NEXT:   %14 = CallInst (:any) %8: any, empty: any, false: boolean, empty: any, %8: any, %13: any, %12: object
;; CHECK-NEXT:   %15 = GetConstructedObjectInst (:object) %13: any, %14: any
;; CHECK-NEXT:   %16 = CreateThisInst (:any) %9: any, %9: any, empty: any
;; CHECK-NEXT:   %17 = CallInst (:any) %9: any, empty: any, false: boolean, empty: any, %9: any, %16: any, %12: object
;; CHECK-NEXT:   %18 = GetConstructedObjectInst (:object) %16: any, %17: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %15: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %18: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %21 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         ReturnInst %21: object
;; CHECK-NEXT: function_end
