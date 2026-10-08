;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: i64 block results and if/else with i64 (G.5).
;; Each i64 result type produces 2 PhiInst nodes (lo, hi).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Block returning i64: 2 phis in continuation block
  (func $block_i64 (result i64)
    (block (result i64)
      i64.const 100))

  ;; If/else returning i64: 2 phis in merge block
  (func $if_i64 (param i32) (result i64)
    (if (result i64) (local.get 0)
      (then (i64.const 1))
      (else (i64.const 2)))))

;; -- block_i64: inner block's continuation has 2 phis for i64 result --
;; Exit block (BB1) was created first by beginFunction:
;; Block continuation (BB2) has 2 phis for i64 result:

;; -- if_i64: merge block has 2 phis for i64 result --
;; Exit block (BB1):
;; The merge block (BB4) has 2 phis with entries from both arms:

;; CHECK: scope %VS0 [wasm_type_id_0: any, wasm_type_id_1: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any, intrinsics: any]
;; CHECK-EMPTY:
;; CHECK-NEXT: function global(): object
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:   %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %__wasm_instantiate__(): functionCode
;; CHECK-NEXT:   %2 = TryLoadGlobalPropertyInst (:any) globalObject: object, "HermesInternal": string
;; CHECK-NEXT:   %3 = LoadPropertyInst (:any) %2: any, "intrinsics": string
;; CHECK-NEXT:   %4 = LoadPropertyInst (:any) %3: any, "Array": string
;; CHECK-NEXT:   %5 = CreateThisInst (:any) %4: any, %4: any, empty: any
;; CHECK-NEXT:   %6 = CallInst (:any) %4: any, empty: any, false: boolean, empty: any, %4: any, %5: any, 0: number
;; CHECK-NEXT:   %7 = GetConstructedObjectInst (:object) %5: any, %6: any
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %3: any, "Array": string
;; CHECK-NEXT:   %9 = CreateThisInst (:any) %8: any, %8: any, empty: any
;; CHECK-NEXT:   %10 = CallInst (:any) %8: any, empty: any, false: boolean, empty: any, %8: any, %9: any, 0: number
;; CHECK-NEXT:   %11 = GetConstructedObjectInst (:object) %9: any, %10: any
;; CHECK-NEXT:   %12 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         DefineOwnPropertyInst %1: object, %12: object, "instantiate": string, true: boolean
;; CHECK-NEXT:         DefineOwnPropertyInst %7: object, %12: object, "exportDescs": string, true: boolean
;; CHECK-NEXT:         DefineOwnPropertyInst %11: object, %12: object, "importDescs": string, true: boolean
;; CHECK-NEXT:         ReturnInst %12: object
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_0(retbuf_I: object, retbuf_F: object): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT:   %2 = LoadParamInst (:object) %retbuf_F: object
;; CHECK-NEXT:        BranchInst %BB2
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %4 = PhiInst (:number) %9: number, %BB2
;; CHECK-NEXT:   %5 = PhiInst (:number) %10: number, %BB2
;; CHECK-NEXT:        StorePropertyStrictInst %4: number, %1: object, 0: number
;; CHECK-NEXT:        StorePropertyStrictInst %5: number, %1: object, 1: number
;; CHECK-NEXT:        ReturnInst 0: number
;; CHECK-NEXT: %BB2:
;; CHECK-NEXT:   %9 = PhiInst (:number) 100: number, %BB0
;; CHECK-NEXT:   %10 = PhiInst (:number) 0: number, %BB0
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_1(retbuf_I: object, retbuf_F: object, p0: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT:   %2 = LoadParamInst (:object) %retbuf_F: object
;; CHECK-NEXT:   %3 = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:        StoreStackInst %4: number, %3: number
;; CHECK-NEXT:   %6 = LoadStackInst (:number) %3: number
;; CHECK-NEXT:        CondBranchInst %6: number, %BB2, %BB3
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %8 = PhiInst (:number) %15: number, %BB4
;; CHECK-NEXT:   %9 = PhiInst (:number) %16: number, %BB4
;; CHECK-NEXT:         StorePropertyStrictInst %8: number, %1: object, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %9: number, %1: object, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: %BB2:
;; CHECK-NEXT:         BranchInst %BB4
;; CHECK-NEXT: %BB3:
;; CHECK-NEXT:         BranchInst %BB4
;; CHECK-NEXT: %BB4:
;; CHECK-NEXT:   %15 = PhiInst (:number) 1: number, %BB2, 2: number, %BB3
;; CHECK-NEXT:   %16 = PhiInst (:number) 0: number, %BB2, 0: number, %BB3
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function __wasm_instantiate__(imports: any): object
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:   %1 = TryLoadGlobalPropertyInst (:any) globalObject: object, "HermesInternal": string
;; CHECK-NEXT:   %2 = LoadPropertyInst (:any) %1: any, "intrinsics": string
;; CHECK-NEXT:        StoreFrameInst %0: environment, %2: any, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %4 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_0(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %4: object, [%VS0.closure_0]: any
;; CHECK-NEXT:   %6 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_1(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %6: object, [%VS0.closure_1]: any
;; CHECK-NEXT:   %8 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %8: any, "ArrayBuffer": string
;; CHECK-NEXT:   %10 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %11 = LoadPropertyInst (:any) %10: any, "Uint32Array": string
;; CHECK-NEXT:   %12 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %13 = LoadPropertyInst (:any) %12: any, "Float64Array": string
;; CHECK-NEXT:   %14 = CreateThisInst (:any) %9: any, %9: any, empty: any
;; CHECK-NEXT:   %15 = CallInst (:any) %9: any, empty: any, false: boolean, empty: any, %9: any, %14: any, 8: number
;; CHECK-NEXT:   %16 = GetConstructedObjectInst (:object) %14: any, %15: any
;; CHECK-NEXT:   %17 = CreateThisInst (:any) %11: any, %11: any, empty: any
;; CHECK-NEXT:   %18 = CallInst (:any) %11: any, empty: any, false: boolean, empty: any, %11: any, %17: any, %16: object
;; CHECK-NEXT:   %19 = GetConstructedObjectInst (:object) %17: any, %18: any
;; CHECK-NEXT:   %20 = CreateThisInst (:any) %13: any, %13: any, empty: any
;; CHECK-NEXT:   %21 = CallInst (:any) %13: any, empty: any, false: boolean, empty: any, %13: any, %20: any, %16: object
;; CHECK-NEXT:   %22 = GetConstructedObjectInst (:object) %20: any, %21: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %19: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %22: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %25 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         ReturnInst %25: object
;; CHECK-NEXT: function_end
