;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for mutual recursion between two Wasm functions.
;; is_even(n) = if n == 0 then 1 else is_odd(n - 1)
;; is_odd(n)  = if n == 0 then 0 else is_even(n - 1)
;; Verifies each function loads the other's closure for the cross-call.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: is_even — calls is_odd (closure_1) in the else branch.
  ;; First function checked exhaustively including param loading.
  (func $is_even (param i32) (result i32)
    local.get 0
    i32.eqz
    if (result i32)
      i32.const 1
    else
      local.get 0
      i32.const 1
      i32.sub
      call $is_odd
    end)

  ;; func 1: is_odd — calls is_even (closure_0) in the else branch.
  (func $is_odd (param i32) (result i32)
    local.get 0
    i32.eqz
    if (result i32)
      i32.const 0
    else
      local.get 0
      i32.const 1
      i32.sub
      call $is_even
    end)
)

;; CHECK: scope %VS0 [wasm_type_id_0: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any, intrinsics: any]
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
;; CHECK-NEXT:         StorePropertyStrictInst %1: object, %12: object, "instantiate": string
;; CHECK-NEXT:         StorePropertyStrictInst %7: object, %12: object, "exportDescs": string
;; CHECK-NEXT:         StorePropertyStrictInst %11: object, %12: object, "importDescs": string
;; CHECK-NEXT:         ReturnInst %12: object
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_0(p0: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:   %3 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:        StoreStackInst %3: number, %2: number
;; CHECK-NEXT:   %5 = LoadStackInst (:number) %2: number
;; CHECK-NEXT:   %6 = FEqualInst (:boolean) %5: number, 0: number
;; CHECK-NEXT:   %7 = AsInt32Inst (:number) %6: boolean
;; CHECK-NEXT:        CondBranchInst %6: boolean, %BB2, %BB3
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %9 = PhiInst (:number) %18: number, %BB4
;; CHECK-NEXT:         ReturnInst %9: number
;; CHECK-NEXT: %BB2:
;; CHECK-NEXT:         BranchInst %BB4
;; CHECK-NEXT: %BB3:
;; CHECK-NEXT:   %12 = LoadStackInst (:number) %2: number
;; CHECK-NEXT:   %13 = FSubtractInst (:number) %12: number, 1: number
;; CHECK-NEXT:   %14 = AsInt32Inst (:number) %13: number
;; CHECK-NEXT:   %15 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %16 = CallInst (:number) %15: any, %wasm_func_1(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %14: number
;; CHECK-NEXT:         BranchInst %BB4
;; CHECK-NEXT: %BB4:
;; CHECK-NEXT:   %18 = PhiInst (:number) 1: number, %BB2, %16: number, %BB3
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_1(p0: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:   %3 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:        StoreStackInst %3: number, %2: number
;; CHECK-NEXT:   %5 = LoadStackInst (:number) %2: number
;; CHECK-NEXT:   %6 = FEqualInst (:boolean) %5: number, 0: number
;; CHECK-NEXT:   %7 = AsInt32Inst (:number) %6: boolean
;; CHECK-NEXT:        CondBranchInst %6: boolean, %BB2, %BB3
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %9 = PhiInst (:number) %18: number, %BB4
;; CHECK-NEXT:         ReturnInst %9: number
;; CHECK-NEXT: %BB2:
;; CHECK-NEXT:         BranchInst %BB4
;; CHECK-NEXT: %BB3:
;; CHECK-NEXT:   %12 = LoadStackInst (:number) %2: number
;; CHECK-NEXT:   %13 = FSubtractInst (:number) %12: number, 1: number
;; CHECK-NEXT:   %14 = AsInt32Inst (:number) %13: number
;; CHECK-NEXT:   %15 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %16 = CallInst (:number) %15: any, %wasm_func_0(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %14: number
;; CHECK-NEXT:         BranchInst %BB4
;; CHECK-NEXT: %BB4:
;; CHECK-NEXT:   %18 = PhiInst (:number) 0: number, %BB2, %16: number, %BB3
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
