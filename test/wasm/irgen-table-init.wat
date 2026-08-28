;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s
;; REQUIRES: wasm

;; Test: Verify that tables are created and element segments are applied
;; in the top-level (global) function.

(module
  (type $void_to_i32 (func (result i32)))

  (table 4 funcref)

  ;; Element segment: place f0 at index 1, f1 at index 2.
  (elem (i32.const 1) $f0 $f1)

  (func $f0 (result i32)
    i32.const 42
  )

  (func $f1 (result i32)
    i32.const 99
  )

  (export "f0" (func $f0))
)

;; The instantiate function creates table arrays and applies elem segments.
;; CHECK: scope %VS0 [wasm_type_id_0: any, table_0_funcs: any, table_0_types: any, table_0_obj: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any]
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
;; CHECK-NEXT:        StorePropertyStrictInst "f0": string, %6: object, "name": string
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
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %3 = PhiInst (:number) 99: number, %BB0
;; CHECK-NEXT:        ReturnInst %3: number
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
;; CHECK-NEXT:   %19 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::i": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %19: any, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %21 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "anyfunc": string, %21: object, "element": string
;; CHECK-NEXT:         StorePropertyStrictInst 4: number, %21: object, "initial": string
;; CHECK-NEXT:   %24 = TryLoadGlobalPropertyInst (:any) globalObject: object, "WebAssembly": string
;; CHECK-NEXT:   %25 = LoadPropertyInst (:any) %24: any, "Table": string
;; CHECK-NEXT:   %26 = CreateThisInst (:any) %25: any, %25: any, empty: any
;; CHECK-NEXT:   %27 = CallInst (:any) %25: any, empty: any, false: boolean, empty: any, %25: any, %26: any, %21: object
;; CHECK-NEXT:   %28 = GetConstructedObjectInst (:object) %26: any, %27: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %28: object, [%VS0.table_0_obj]: any
;; CHECK-NEXT:   %30 = LoadPropertyInst (:any) %28: object, "__wasm_funcs__": string
;; CHECK-NEXT:   %31 = LoadPropertyInst (:any) %28: object, "__wasm_types__": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %30: any, [%VS0.table_0_funcs]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %31: any, [%VS0.table_0_types]: any
;; CHECK-NEXT:   %34 = LoadFrameInst (:any) %0: environment, [%VS0.table_0_funcs]: any
;; CHECK-NEXT:   %35 = LoadFrameInst (:any) %0: environment, [%VS0.table_0_types]: any
;; CHECK-NEXT:   %36 = BinaryAddInst (:any) 1: number, 0: number
;; CHECK-NEXT:   %37 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:         StorePropertyStrictInst %37: any, %34: any, %36: any
;; CHECK-NEXT:   %39 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:         StorePropertyStrictInst %39: any, %35: any, %36: any
;; CHECK-NEXT:   %41 = BinaryAddInst (:any) 1: number, 1: number
;; CHECK-NEXT:   %42 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:         StorePropertyStrictInst %42: any, %34: any, %41: any
;; CHECK-NEXT:   %44 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:         StorePropertyStrictInst %44: any, %35: any, %41: any
;; CHECK-NEXT:   %46 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:   %47 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_f0(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func::i": string, %47: object, "__wasm_type__": string
;; CHECK-NEXT:         StorePropertyStrictInst %47: object, %46: object, "f0": string
;; CHECK-NEXT:         ReturnInst %46: object
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_export_f0(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %2 = CallInst (:any) %1: any, %wasm_func_0(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:        ReturnInst %2: any
;; CHECK-NEXT: function_end
