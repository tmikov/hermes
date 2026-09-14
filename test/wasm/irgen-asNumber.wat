;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test that asNumber() converts an :any value to :number
;; when a value loaded from a global variable (LoadFrameInst, typed :any) flows
;; into a typed instruction (FAddInst).
;;
;; Direct calls now have typed results (CallInst :number), so they don't need
;; narrowing. But global.get produces LoadFrameInst :any because frame
;; variables are untyped in the IR, so asNumber() must convert the value
;; before it can be used by FAddInst.
;;
;; The conversion is AsNumberInst, a real ToNumber, and deliberately not
;; UnionNarrowTrustedInst: the verifier does not check UnionNarrowTrustedInst,
;; so asserting the type here would silence the very check that catches a
;; non-number reaching FAddInst.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (global $g (mut f64) (f64.const 1.0))

  ;; Adds a parameter to a global. The global.get produces LoadFrameInst :any,
  ;; so asNumber() narrows it to :number for FAddInst.
  (func (export "add_global") (param f64) (result f64)
    local.get 0
    global.get $g
    f64.add)
)

;; CHECK: scope %VS0 [wasm_type_id_0: any, global_0: any, retBufI: any, retBufF: any, closure_0: any, exported_func_0: any, intrinsics: any]
;; CHECK-EMPTY:
;; CHECK-NEXT: function global(): object
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:   %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %__wasm_instantiate__(): functionCode
;; CHECK-NEXT:   %2 = TryLoadGlobalPropertyInst (:any) globalObject: object, "HermesInternal": string
;; CHECK-NEXT:   %3 = LoadPropertyInst (:any) %2: any, "intrinsics": string
;; CHECK-NEXT:   %4 = LoadPropertyInst (:any) %3: any, "Array": string
;; CHECK-NEXT:   %5 = CreateThisInst (:any) %4: any, %4: any, empty: any
;; CHECK-NEXT:   %6 = CallInst (:any) %4: any, empty: any, false: boolean, empty: any, %4: any, %5: any, 1: number
;; CHECK-NEXT:   %7 = GetConstructedObjectInst (:object) %5: any, %6: any
;; CHECK-NEXT:   %8 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:        DefineOwnPropertyInst "add_global": string, %8: object, "name": string, true: boolean
;; CHECK-NEXT:         DefineOwnPropertyInst "function": string, %8: object, "kind": string, true: boolean
;; CHECK-NEXT:         DefineOwnPropertyInst %8: object, %7: object, 0: number, true: boolean
;; CHECK-NEXT:   %12 = LoadPropertyInst (:any) %3: any, "Array": string
;; CHECK-NEXT:   %13 = CreateThisInst (:any) %12: any, %12: any, empty: any
;; CHECK-NEXT:   %14 = CallInst (:any) %12: any, empty: any, false: boolean, empty: any, %12: any, %13: any, 0: number
;; CHECK-NEXT:   %15 = GetConstructedObjectInst (:object) %13: any, %14: any
;; CHECK-NEXT:   %16 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         DefineOwnPropertyInst %1: object, %16: object, "instantiate": string, true: boolean
;; CHECK-NEXT:         DefineOwnPropertyInst %7: object, %16: object, "exportDescs": string, true: boolean
;; CHECK-NEXT:         DefineOwnPropertyInst %15: object, %16: object, "importDescs": string, true: boolean
;; CHECK-NEXT:         ReturnInst %16: object
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
;; CHECK-NEXT:   %6 = LoadFrameInst (:any) %0: environment, [%VS0.global_0]: any
;; CHECK-NEXT:   %7 = AsNumberInst (:number) %6: any
;; CHECK-NEXT:   %8 = FAddInst (:number) %5: number, %7: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %10 = PhiInst (:number) %8: number, %BB0
;; CHECK-NEXT:         ReturnInst %10: number
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
;; CHECK-NEXT:   %6 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %7 = LoadPropertyInst (:any) %6: any, "ArrayBuffer": string
;; CHECK-NEXT:   %8 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %8: any, "Uint32Array": string
;; CHECK-NEXT:   %10 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %11 = LoadPropertyInst (:any) %10: any, "Float64Array": string
;; CHECK-NEXT:   %12 = CreateThisInst (:any) %7: any, %7: any, empty: any
;; CHECK-NEXT:   %13 = CallInst (:any) %7: any, empty: any, false: boolean, empty: any, %7: any, %12: any, 8: number
;; CHECK-NEXT:   %14 = GetConstructedObjectInst (:object) %12: any, %13: any
;; CHECK-NEXT:   %15 = CreateThisInst (:any) %9: any, %9: any, empty: any
;; CHECK-NEXT:   %16 = CallInst (:any) %9: any, empty: any, false: boolean, empty: any, %9: any, %15: any, %14: object
;; CHECK-NEXT:   %17 = GetConstructedObjectInst (:object) %15: any, %16: any
;; CHECK-NEXT:   %18 = CreateThisInst (:any) %11: any, %11: any, empty: any
;; CHECK-NEXT:   %19 = CallInst (:any) %11: any, empty: any, false: boolean, empty: any, %11: any, %18: any, %14: object
;; CHECK-NEXT:   %20 = GetConstructedObjectInst (:object) %18: any, %19: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %17: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %20: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %23 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:d:d": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %23: any, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %25 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_add_global(): functionCode
;; CHECK-NEXT:   %26 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %27 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %28 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %25: object, %26: any, %27: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %25: object, [%VS0.exported_func_0]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, 1: number, [%VS0.global_0]: any
;; CHECK-NEXT:   %31 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:   %32 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_0]: any
;; CHECK-NEXT:         DefineOwnPropertyInst %32: any, %31: object, "add_global": string, true: boolean
;; CHECK-NEXT:         ReturnInst %31: object
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_export_add_global(p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:   %3 = AsNumberInst (:number) %2: any
;; CHECK-NEXT:   %4 = CallInst (:any) %1: any, %wasm_func_0(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %3: number
;; CHECK-NEXT:        ReturnInst %4: any
;; CHECK-NEXT: function_end
