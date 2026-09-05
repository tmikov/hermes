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

;; CHECK: scope %VS0 [wasm_type_id_0: any, wasm_type_id_1: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any]
;; CHECK-EMPTY:
;; CHECK-NEXT: function global(): object
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
;; CHECK-NEXT:   %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_0(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %1: object, [%VS0.closure_0]: any
;; CHECK-NEXT:   %3 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_1(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %3: object, [%VS0.closure_1]: any
;; CHECK-NEXT:   %5 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:   %6 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:   %7 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:   %8 = CreateThisInst (:any) %5: any, %5: any, empty: any
;; CHECK-NEXT:   %9 = CallInst (:any) %5: any, empty: any, false: boolean, empty: any, %5: any, %8: any, 8: number
;; CHECK-NEXT:   %10 = GetConstructedObjectInst (:object) %8: any, %9: any
;; CHECK-NEXT:   %11 = CreateThisInst (:any) %6: any, %6: any, empty: any
;; CHECK-NEXT:   %12 = CallInst (:any) %6: any, empty: any, false: boolean, empty: any, %6: any, %11: any, %10: object
;; CHECK-NEXT:   %13 = GetConstructedObjectInst (:object) %11: any, %12: any
;; CHECK-NEXT:   %14 = CreateThisInst (:any) %7: any, %7: any, empty: any
;; CHECK-NEXT:   %15 = CallInst (:any) %7: any, empty: any, false: boolean, empty: any, %7: any, %14: any, %10: object
;; CHECK-NEXT:   %16 = GetConstructedObjectInst (:object) %14: any, %15: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %13: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %16: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %19 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         ReturnInst %19: object
;; CHECK-NEXT: function_end
