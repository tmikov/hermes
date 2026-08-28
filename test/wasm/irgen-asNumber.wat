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

;; CHECK: scope %VS0 [wasm_type_id_0: any, global_0: any, retBufI: any, retBufF: any, closure_0: any, exported_func_0: any]
;; CHECK-EMPTY:
;; CHECK-NEXT: function global(): object
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:   %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %__wasm_instantiate__(): functionCode
;; CHECK-NEXT:   %2 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK-NEXT:   %3 = CreateThisInst (:any) %2: any, %2: any, empty: any
;; CHECK-NEXT:   %4 = CallInst (:any) %2: any, empty: any, false: boolean, empty: any, %2: any, %3: any, 1: number
;; CHECK-NEXT:   %5 = GetConstructedObjectInst (:object) %3: any, %4: any
;; CHECK-NEXT:   %6 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:        StorePropertyStrictInst "add_global": string, %6: object, "name": string
;; CHECK-NEXT:        StorePropertyStrictInst "function": string, %6: object, "kind": string
;; CHECK-NEXT:        StorePropertyStrictInst %6: object, %5: object, 0: number
;; CHECK-NEXT:   %10 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK-NEXT:   %11 = CreateThisInst (:any) %10: any, %10: any, empty: any
;; CHECK-NEXT:   %12 = CallInst (:any) %10: any, empty: any, false: boolean, empty: any, %10: any, %11: any, 0: number
;; CHECK-NEXT:   %13 = GetConstructedObjectInst (:object) %11: any, %12: any
;; CHECK-NEXT:   %14 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst %1: object, %14: object, "instantiate": string
;; CHECK-NEXT:         StorePropertyStrictInst %5: object, %14: object, "exportDescs": string
;; CHECK-NEXT:         StorePropertyStrictInst %13: object, %14: object, "importDescs": string
;; CHECK-NEXT:         ReturnInst %14: object
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
;; CHECK-NEXT:   %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_0(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %1: object, [%VS0.closure_0]: any
;; CHECK-NEXT:   %3 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:   %4 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:   %5 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:   %6 = CreateThisInst (:any) %3: any, %3: any, empty: any
;; CHECK-NEXT:   %7 = CallInst (:any) %3: any, empty: any, false: boolean, empty: any, %3: any, %6: any, 8: number
;; CHECK-NEXT:   %8 = GetConstructedObjectInst (:object) %6: any, %7: any
;; CHECK-NEXT:   %9 = CreateThisInst (:any) %4: any, %4: any, empty: any
;; CHECK-NEXT:   %10 = CallInst (:any) %4: any, empty: any, false: boolean, empty: any, %4: any, %9: any, %8: object
;; CHECK-NEXT:   %11 = GetConstructedObjectInst (:object) %9: any, %10: any
;; CHECK-NEXT:   %12 = CreateThisInst (:any) %5: any, %5: any, empty: any
;; CHECK-NEXT:   %13 = CallInst (:any) %5: any, empty: any, false: boolean, empty: any, %5: any, %12: any, %8: object
;; CHECK-NEXT:   %14 = GetConstructedObjectInst (:object) %12: any, %13: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %11: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %14: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %17 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:d:d": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %17: any, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %19 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_add_global(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func:d:d": string, %19: object, "__wasm_type__": string
;; CHECK-NEXT:   %21 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %22 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %23 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %19: object, %21: any, %22: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %19: object, [%VS0.exported_func_0]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, 1: number, [%VS0.global_0]: any
;; CHECK-NEXT:   %26 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:   %27 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_0]: any
;; CHECK-NEXT:         StorePropertyStrictInst %27: any, %26: object, "add_global": string
;; CHECK-NEXT:         ReturnInst %26: object
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
