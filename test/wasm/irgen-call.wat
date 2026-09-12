;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for Wasm call instruction.
;; Verifies that calls load the callee closure from the parent scope
;; and pass the correct arguments via CallInst.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: returns constant 42 (no call, just branch to return).
  (func $getConst (result i32)
    i32.const 42)

  ;; func 1: calls $getConst (func 0) and returns its result.
  ;; First call function checked exhaustively.
  (func $callAndReturn (result i32)
    call $getConst)

  ;; func 2: add(a, b) = a + b (callee for func 3).
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)

  ;; func 3: calls $add (func 2) with constants 10, 20.
  (func $callWithArgs (result i32)
    i32.const 10
    i32.const 20
    call $add)

  ;; func 4: void callee (empty function).
  (func $voidCallee)

  ;; func 5: calls void callee (func 4).
  (func $callVoid
    call $voidCallee)
)

;; CHECK: scope %VS0 [wasm_type_id_0: any, wasm_type_id_1: any, wasm_type_id_2: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any, closure_2: any, closure_3: any, closure_4: any, closure_5: any, intrinsics: any]
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
;; CHECK-NEXT: function wasm_func_0(): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %3 = PhiInst (:number) 42: number, %BB0
;; CHECK-NEXT:        ReturnInst %3: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_1(): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %3 = CallInst (:number) %2: any, %wasm_func_0(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %5 = PhiInst (:number) %3: number, %BB0
;; CHECK-NEXT:        ReturnInst %5: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_2(p0: number, p1: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:   %3 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:        StoreStackInst %3: number, %2: number
;; CHECK-NEXT:   %5 = AllocStackInst (:number) $local_1: any
;; CHECK-NEXT:   %6 = LoadParamInst (:number) %p1: number
;; CHECK-NEXT:        StoreStackInst %6: number, %5: number
;; CHECK-NEXT:   %8 = LoadStackInst (:number) %2: number
;; CHECK-NEXT:   %9 = LoadStackInst (:number) %5: number
;; CHECK-NEXT:   %10 = FAddInst (:number) %8: number, %9: number
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %10: number
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         ReturnInst %13: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_3(): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:   %3 = CallInst (:number) %2: any, %wasm_func_2(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, 10: number, 20: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %5 = PhiInst (:number) %3: number, %BB0
;; CHECK-NEXT:        ReturnInst %5: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_4(): undefined
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_5(): undefined
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = LoadFrameInst (:any) %0: environment, [%VS0.closure_4]: any
;; CHECK-NEXT:   %3 = CallInst (:undefined) %2: any, %wasm_func_4(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
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
;; CHECK-NEXT:   %8 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_2(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %8: object, [%VS0.closure_2]: any
;; CHECK-NEXT:   %10 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_3(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %10: object, [%VS0.closure_3]: any
;; CHECK-NEXT:   %12 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_4(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %12: object, [%VS0.closure_4]: any
;; CHECK-NEXT:   %14 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_5(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %14: object, [%VS0.closure_5]: any
;; CHECK-NEXT:   %16 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %17 = LoadPropertyInst (:any) %16: any, "ArrayBuffer": string
;; CHECK-NEXT:   %18 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %19 = LoadPropertyInst (:any) %18: any, "Uint32Array": string
;; CHECK-NEXT:   %20 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %21 = LoadPropertyInst (:any) %20: any, "Float64Array": string
;; CHECK-NEXT:   %22 = CreateThisInst (:any) %17: any, %17: any, empty: any
;; CHECK-NEXT:   %23 = CallInst (:any) %17: any, empty: any, false: boolean, empty: any, %17: any, %22: any, 8: number
;; CHECK-NEXT:   %24 = GetConstructedObjectInst (:object) %22: any, %23: any
;; CHECK-NEXT:   %25 = CreateThisInst (:any) %19: any, %19: any, empty: any
;; CHECK-NEXT:   %26 = CallInst (:any) %19: any, empty: any, false: boolean, empty: any, %19: any, %25: any, %24: object
;; CHECK-NEXT:   %27 = GetConstructedObjectInst (:object) %25: any, %26: any
;; CHECK-NEXT:   %28 = CreateThisInst (:any) %21: any, %21: any, empty: any
;; CHECK-NEXT:   %29 = CallInst (:any) %21: any, empty: any, false: boolean, empty: any, %21: any, %28: any, %24: object
;; CHECK-NEXT:   %30 = GetConstructedObjectInst (:object) %28: any, %29: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %27: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %30: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %33 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         ReturnInst %33: object
;; CHECK-NEXT: function_end
