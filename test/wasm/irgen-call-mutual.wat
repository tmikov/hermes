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

;; CHECK: scope %VS0 [wasm_type_id_0: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any]
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
;; CHECK-NEXT: function wasm_func_0(p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %3 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %3: any, %2: any
;; CHECK-NEXT:   %5 = LoadStackInst (:any) %2: any
;; CHECK-NEXT:   %6 = BinaryStrictlyEqualInst (:any) %5: any, 0: number
;; CHECK-NEXT:   %7 = BinaryOrInst (:any) %6: any, 0: number
;; CHECK-NEXT:        CondBranchInst %7: any, %BB2, %BB3
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %9 = PhiInst (:any) %18: any, %BB4
;; CHECK-NEXT:         ReturnInst %9: any
;; CHECK-NEXT: %BB2:
;; CHECK-NEXT:         BranchInst %BB4
;; CHECK-NEXT: %BB3:
;; CHECK-NEXT:   %12 = LoadStackInst (:any) %2: any
;; CHECK-NEXT:   %13 = BinarySubtractInst (:any) %12: any, 1: number
;; CHECK-NEXT:   %14 = AsInt32Inst (:number) %13: any
;; CHECK-NEXT:   %15 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %16 = CallInst (:any) %15: any, %wasm_func_1(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %14: number
;; CHECK-NEXT:         BranchInst %BB4
;; CHECK-NEXT: %BB4:
;; CHECK-NEXT:   %18 = PhiInst (:any) 1: number, %BB2, %16: any, %BB3
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_1(p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %3 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %3: any, %2: any
;; CHECK-NEXT:   %5 = LoadStackInst (:any) %2: any
;; CHECK-NEXT:   %6 = BinaryStrictlyEqualInst (:any) %5: any, 0: number
;; CHECK-NEXT:   %7 = BinaryOrInst (:any) %6: any, 0: number
;; CHECK-NEXT:        CondBranchInst %7: any, %BB2, %BB3
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %9 = PhiInst (:any) %18: any, %BB4
;; CHECK-NEXT:         ReturnInst %9: any
;; CHECK-NEXT: %BB2:
;; CHECK-NEXT:         BranchInst %BB4
;; CHECK-NEXT: %BB3:
;; CHECK-NEXT:   %12 = LoadStackInst (:any) %2: any
;; CHECK-NEXT:   %13 = BinarySubtractInst (:any) %12: any, 1: number
;; CHECK-NEXT:   %14 = AsInt32Inst (:number) %13: any
;; CHECK-NEXT:   %15 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %16 = CallInst (:any) %15: any, %wasm_func_0(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %14: number
;; CHECK-NEXT:         BranchInst %BB4
;; CHECK-NEXT: %BB4:
;; CHECK-NEXT:   %18 = PhiInst (:any) 0: number, %BB2, %16: any, %BB3
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function __wasm_instantiate__(): any
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
