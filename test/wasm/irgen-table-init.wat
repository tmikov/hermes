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
;; CHECK: scope %VS0 [wasm_type_id_0: any, table_0_funcs: any, table_0_types: any, table_0_exported: any, table_0_obj: any, retBufI: any, retBufF: any, closure_0: any, exported_func_0: any, closure_1: any, exported_func_1: any]
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
;; CHECK-NEXT:   %21 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_f0(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func::i": string, %21: object, "__wasm_type__": string
;; CHECK-NEXT:   %23 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %24 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %25 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %21: object, %23: any, %24: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %21: object, [%VS0.exported_func_0]: any
;; CHECK-NEXT:   %27 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_1(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func::i": string, %27: object, "__wasm_type__": string
;; CHECK-NEXT:   %29 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %30 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %31 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %27: object, %29: any, %30: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %27: object, [%VS0.exported_func_1]: any
;; CHECK-NEXT:   %33 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "anyfunc": string, %33: object, "element": string
;; CHECK-NEXT:         StorePropertyStrictInst 4: number, %33: object, "initial": string
;; CHECK-NEXT:   %36 = TryLoadGlobalPropertyInst (:any) globalObject: object, "WebAssembly": string
;; CHECK-NEXT:   %37 = LoadPropertyInst (:any) %36: any, "Table": string
;; CHECK-NEXT:   %38 = CreateThisInst (:any) %37: any, %37: any, empty: any
;; CHECK-NEXT:   %39 = CallInst (:any) %37: any, empty: any, false: boolean, empty: any, %37: any, %38: any, %33: object
;; CHECK-NEXT:   %40 = GetConstructedObjectInst (:object) %38: any, %39: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %40: object, [%VS0.table_0_obj]: any
;; CHECK-NEXT:   %42 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkTable]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %40: object, true: boolean
;; CHECK-NEXT:   %43 = BinaryStrictlyEqualInst (:any) %42: any, null: null
;; CHECK-NEXT:         CondBranchInst %43: any, %BB1, %BB2
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %45 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "WebAssembly.Table did not construct a table for this module's table 0": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB2:
;; CHECK-NEXT:   %47 = LoadPropertyInst (:any) %42: any, 0: number
;; CHECK-NEXT:   %48 = LoadPropertyInst (:any) %42: any, 1: number
;; CHECK-NEXT:   %49 = LoadPropertyInst (:any) %42: any, 2: number
;; CHECK-NEXT:   %50 = LoadPropertyInst (:any) %47: any, "length": string
;; CHECK-NEXT:   %51 = LoadPropertyInst (:any) %42: any, 3: number
;; CHECK-NEXT:   %52 = BinaryStrictlyEqualInst (:any) %50: any, 4: number
;; CHECK-NEXT:         CondBranchInst %52: any, %BB4, %BB3
;; CHECK-NEXT: %BB3:
;; CHECK-NEXT:   %54 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "WebAssembly.Table did not construct a table with this module's declared limits for table 0": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB4:
;; CHECK-NEXT:   %56 = BinaryStrictlyEqualInst (:any) %51: any, -1: number
;; CHECK-NEXT:         CondBranchInst %56: any, %BB5, %BB3
;; CHECK-NEXT: %BB5:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %47: any, [%VS0.table_0_funcs]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %48: any, [%VS0.table_0_types]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %49: any, [%VS0.table_0_exported]: any
;; CHECK-NEXT:   %61 = LoadFrameInst (:any) %0: environment, [%VS0.table_0_funcs]: any
;; CHECK-NEXT:   %62 = LoadFrameInst (:any) %0: environment, [%VS0.table_0_types]: any
;; CHECK-NEXT:   %63 = LoadFrameInst (:any) %0: environment, [%VS0.table_0_exported]: any
;; CHECK-NEXT:   %64 = BinaryAddInst (:any) 1: number, 0: number
;; CHECK-NEXT:   %65 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_0]: any
;; CHECK-NEXT:   %66 = CallBuiltinInst (:any) [HermesBuiltin.wasmTableSetSlot]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %61: any, %62: any, %63: any, %64: any, %65: any, 1: number
;; CHECK-NEXT:   %67 = BinaryAddInst (:any) 1: number, 1: number
;; CHECK-NEXT:   %68 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_1]: any
;; CHECK-NEXT:   %69 = CallBuiltinInst (:any) [HermesBuiltin.wasmTableSetSlot]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %61: any, %62: any, %63: any, %67: any, %68: any, 1: number
;; CHECK-NEXT:   %70 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:   %71 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_0]: any
;; CHECK-NEXT:         StorePropertyStrictInst %71: any, %70: object, "f0": string
;; CHECK-NEXT:         ReturnInst %70: object
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
