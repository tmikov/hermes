;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s
;; REQUIRES: wasm

;; Test: Verify that tables are created and element segments are applied
;; in the top-level (global) function.
;;
;; The two BinaryStrictlyEqualInst pairs after wasmLinkTable are the defined
;; table's limits check: `funcs.length` against the declared 4, and the table's
;; own maximum (index 3 of the builtin's result) against -1, this table having
;; declared none. Both share one LinkError block. Behaviour is pinned by
;; e2e-defined-table-limits.wat; this file pins that the code is EMITTED, and
;; that the -1 sentinel is what an unbounded declaration compares against.

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
;; CHECK: scope %VS0 [wasm_type_id_0: any, table_0_funcs: any, table_0_types: any, table_0_exported: any, table_0_obj: any, retBufI: any, retBufF: any, closure_0: any, exported_func_0: any, closure_1: any, exported_func_1: any, intrinsics: any]
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
;; CHECK-NEXT:        StorePropertyStrictInst "f0": string, %8: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %8: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %8: object, %7: object, 0: number
;; CHECK-NEXT:   %12 = LoadPropertyInst (:any) %3: any, "Array": string
;; CHECK-NEXT:   %13 = CreateThisInst (:any) %12: any, %12: any, empty: any
;; CHECK-NEXT:   %14 = CallInst (:any) %12: any, empty: any, false: boolean, empty: any, %12: any, %13: any, 0: number
;; CHECK-NEXT:   %15 = GetConstructedObjectInst (:object) %13: any, %14: any
;; CHECK-NEXT:   %16 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst %1: object, %16: object, "instantiate": string
;; CHECK-NEXT:         StorePropertyStrictInst %7: object, %16: object, "exportDescs": string
;; CHECK-NEXT:         StorePropertyStrictInst %15: object, %16: object, "importDescs": string
;; CHECK-NEXT:         ReturnInst %16: object
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
;; CHECK-NEXT:   %25 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::i": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %25: any, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %27 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_f0(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func::i": string, %27: object, "__wasm_type__": string
;; CHECK-NEXT:   %29 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %30 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %31 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %27: object, %29: any, %30: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %27: object, [%VS0.exported_func_0]: any
;; CHECK-NEXT:   %33 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_1(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func::i": string, %33: object, "__wasm_type__": string
;; CHECK-NEXT:   %35 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %36 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %37 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %33: object, %35: any, %36: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %33: object, [%VS0.exported_func_1]: any
;; CHECK-NEXT:   %39 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "anyfunc": string, %39: object, "element": string
;; CHECK-NEXT:         StorePropertyStrictInst 4: number, %39: object, "initial": string
;; CHECK-NEXT:   %42 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %43 = LoadPropertyInst (:any) %42: any, "WebAssembly": string
;; CHECK-NEXT:   %44 = LoadPropertyInst (:any) %43: any, "Table": string
;; CHECK-NEXT:   %45 = CreateThisInst (:any) %44: any, %44: any, empty: any
;; CHECK-NEXT:   %46 = CallInst (:any) %44: any, empty: any, false: boolean, empty: any, %44: any, %45: any, %39: object
;; CHECK-NEXT:   %47 = GetConstructedObjectInst (:object) %45: any, %46: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %47: object, [%VS0.table_0_obj]: any
;; CHECK-NEXT:   %49 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkTable]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %47: object, true: boolean
;; CHECK-NEXT:   %50 = BinaryStrictlyEqualInst (:any) %49: any, null: null
;; CHECK-NEXT:         CondBranchInst %50: any, %BB1, %BB2
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %52 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "WebAssembly.Table did not construct a table for this module's table 0": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB2:
;; CHECK-NEXT:   %54 = LoadPropertyInst (:any) %49: any, 0: number
;; CHECK-NEXT:   %55 = LoadPropertyInst (:any) %49: any, 1: number
;; CHECK-NEXT:   %56 = LoadPropertyInst (:any) %49: any, 2: number
;; CHECK-NEXT:   %57 = LoadPropertyInst (:any) %54: any, "length": string
;; CHECK-NEXT:   %58 = LoadPropertyInst (:any) %49: any, 3: number
;; CHECK-NEXT:   %59 = BinaryStrictlyEqualInst (:any) %57: any, 4: number
;; CHECK-NEXT:         CondBranchInst %59: any, %BB4, %BB3
;; CHECK-NEXT: %BB3:
;; CHECK-NEXT:   %61 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "WebAssembly.Table did not construct a table with this module's declared limits for table 0": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB4:
;; CHECK-NEXT:   %63 = BinaryStrictlyEqualInst (:any) %58: any, -1: number
;; CHECK-NEXT:         CondBranchInst %63: any, %BB5, %BB3
;; CHECK-NEXT: %BB5:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %54: any, [%VS0.table_0_funcs]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %55: any, [%VS0.table_0_types]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %56: any, [%VS0.table_0_exported]: any
;; CHECK-NEXT:   %68 = LoadFrameInst (:any) %0: environment, [%VS0.table_0_funcs]: any
;; CHECK-NEXT:   %69 = LoadFrameInst (:any) %0: environment, [%VS0.table_0_types]: any
;; CHECK-NEXT:   %70 = LoadFrameInst (:any) %0: environment, [%VS0.table_0_exported]: any
;; CHECK-NEXT:   %71 = BinaryAddInst (:any) 1: number, 0: number
;; CHECK-NEXT:   %72 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_0]: any
;; CHECK-NEXT:   %73 = CallBuiltinInst (:any) [HermesBuiltin.wasmTableSetSlot]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %68: any, %69: any, %70: any, %71: any, %72: any, 1: number
;; CHECK-NEXT:   %74 = BinaryAddInst (:any) 1: number, 1: number
;; CHECK-NEXT:   %75 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_1]: any
;; CHECK-NEXT:   %76 = CallBuiltinInst (:any) [HermesBuiltin.wasmTableSetSlot]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %68: any, %69: any, %70: any, %74: any, %75: any, 1: number
;; CHECK-NEXT:   %77 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:   %78 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_0]: any
;; CHECK-NEXT:         StorePropertyStrictInst %78: any, %77: object, "f0": string
;; CHECK-NEXT:         ReturnInst %77: object
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_export_f0(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %2 = CallInst (:any) %1: any, %wasm_func_0(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:        ReturnInst %2: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_funcref_1(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %2 = CallInst (:any) %1: any, %wasm_func_1(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:        ReturnInst %2: any
;; CHECK-NEXT: function_end
