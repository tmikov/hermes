;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR structure of export wrapper functions for different type signatures.
;; I.1: Export wrapper functions.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; (i32, i32) -> i32: wrapper coerces both args with AsInt32Inst.
  (func $add_i32 (export "add_i32") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)

  ;; () -> (): void function wrapper returns undefined.
  (func $void_func (export "void_func")
    nop)

  ;; (f64, f64) -> f64: wrapper passes args through (no coercion).
  (func $add_f64 (export "add_f64") (param f64 f64) (result f64)
    local.get 0
    local.get 1
    f64.add)

  ;; (i32, f64) -> f64: mixed types.
  (func $mixed (export "mixed") (param i32 f64) (result f64)
    local.get 0
    f64.convert_i32_s
    local.get 1
    f64.add)

  ;; (i64) -> i64: i64 wrapper converts BigInt arg and returns BigInt.
  (func $id_i64 (export "id_i64") (param i64) (result i64)
    local.get 0)
)

;; --- Wrapper for add_i32: coerces both i32 args ---

;; --- Wrapper for void_func: no params, returns undefined ---

;; --- Wrapper for add_f64: passes f64 args through (no coercion) ---

;; --- Wrapper for mixed: i32 coerced, f64 passed through ---

;; --- Wrapper for id_i64: BigInt param converted to lo/hi, result back to BigInt ---

;; CHECK: scope %VS0 [wasm_type_id_0: any, wasm_type_id_1: any, wasm_type_id_2: any, wasm_type_id_3: any, wasm_type_id_4: any, retBufI: any, retBufF: any, closure_0: any, exported_func_0: any, closure_1: any, exported_func_1: any, closure_2: any, exported_func_2: any, closure_3: any, exported_func_3: any, closure_4: any, exported_func_4: any, intrinsics: any]
;; CHECK-EMPTY:
;; CHECK-NEXT: function global(): object
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:   %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %__wasm_instantiate__(): functionCode
;; CHECK-NEXT:   %2 = TryLoadGlobalPropertyInst (:any) globalObject: object, "HermesInternal": string
;; CHECK-NEXT:   %3 = LoadPropertyInst (:any) %2: any, "intrinsics": string
;; CHECK-NEXT:   %4 = LoadPropertyInst (:any) %3: any, "Array": string
;; CHECK-NEXT:   %5 = CreateThisInst (:any) %4: any, %4: any, empty: any
;; CHECK-NEXT:   %6 = CallInst (:any) %4: any, empty: any, false: boolean, empty: any, %4: any, %5: any, 5: number
;; CHECK-NEXT:   %7 = GetConstructedObjectInst (:object) %5: any, %6: any
;; CHECK-NEXT:   %8 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:        StorePropertyStrictInst "add_i32": string, %8: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %8: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %8: object, %7: object, 0: number
;; CHECK-NEXT:   %12 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "void_func": string, %12: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %12: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %12: object, %7: object, 1: number
;; CHECK-NEXT:   %16 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "add_f64": string, %16: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %16: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %16: object, %7: object, 2: number
;; CHECK-NEXT:   %20 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "mixed": string, %20: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %20: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %20: object, %7: object, 3: number
;; CHECK-NEXT:   %24 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "id_i64": string, %24: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %24: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %24: object, %7: object, 4: number
;; CHECK-NEXT:   %28 = LoadPropertyInst (:any) %3: any, "Array": string
;; CHECK-NEXT:   %29 = CreateThisInst (:any) %28: any, %28: any, empty: any
;; CHECK-NEXT:   %30 = CallInst (:any) %28: any, empty: any, false: boolean, empty: any, %28: any, %29: any, 0: number
;; CHECK-NEXT:   %31 = GetConstructedObjectInst (:object) %29: any, %30: any
;; CHECK-NEXT:   %32 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst %1: object, %32: object, "instantiate": string
;; CHECK-NEXT:         StorePropertyStrictInst %7: object, %32: object, "exportDescs": string
;; CHECK-NEXT:         StorePropertyStrictInst %31: object, %32: object, "importDescs": string
;; CHECK-NEXT:         ReturnInst %32: object
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_0(p0: number, p1: number): number
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
;; CHECK-NEXT: function wasm_func_1(): undefined
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
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
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %12 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:         ReturnInst %12: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_3(p0: number, p1: number): number
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
;; CHECK-NEXT:   %9 = AsInt32Inst (:number) %8: number
;; CHECK-NEXT:   %10 = LoadStackInst (:number) %5: number
;; CHECK-NEXT:   %11 = FAddInst (:number) %9: number, %10: number
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         ReturnInst %13: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_4(retbuf_I: object, retbuf_F: object, p0_lo: number, p0_hi: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT:   %2 = LoadParamInst (:object) %retbuf_F: object
;; CHECK-NEXT:   %3 = AllocStackInst (:number) $local_0_lo: any
;; CHECK-NEXT:   %4 = AllocStackInst (:number) $local_0_hi: any
;; CHECK-NEXT:   %5 = LoadParamInst (:number) %p0_lo: number
;; CHECK-NEXT:        StoreStackInst %5: number, %3: number
;; CHECK-NEXT:   %7 = LoadParamInst (:number) %p0_hi: number
;; CHECK-NEXT:        StoreStackInst %7: number, %4: number
;; CHECK-NEXT:   %9 = LoadStackInst (:number) %3: number
;; CHECK-NEXT:   %10 = LoadStackInst (:number) %4: number
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %12 = PhiInst (:number) %9: number, %BB0
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %12: number, %1: object, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: object, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
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
;; CHECK-NEXT:   %14 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %15 = LoadPropertyInst (:any) %14: any, "ArrayBuffer": string
;; CHECK-NEXT:   %16 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %17 = LoadPropertyInst (:any) %16: any, "Uint32Array": string
;; CHECK-NEXT:   %18 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %19 = LoadPropertyInst (:any) %18: any, "Float64Array": string
;; CHECK-NEXT:   %20 = CreateThisInst (:any) %15: any, %15: any, empty: any
;; CHECK-NEXT:   %21 = CallInst (:any) %15: any, empty: any, false: boolean, empty: any, %15: any, %20: any, 8: number
;; CHECK-NEXT:   %22 = GetConstructedObjectInst (:object) %20: any, %21: any
;; CHECK-NEXT:   %23 = CreateThisInst (:any) %17: any, %17: any, empty: any
;; CHECK-NEXT:   %24 = CallInst (:any) %17: any, empty: any, false: boolean, empty: any, %17: any, %23: any, %22: object
;; CHECK-NEXT:   %25 = GetConstructedObjectInst (:object) %23: any, %24: any
;; CHECK-NEXT:   %26 = CreateThisInst (:any) %19: any, %19: any, empty: any
;; CHECK-NEXT:   %27 = CallInst (:any) %19: any, empty: any, false: boolean, empty: any, %19: any, %26: any, %22: object
;; CHECK-NEXT:   %28 = GetConstructedObjectInst (:object) %26: any, %27: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %25: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %28: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %31 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:ii:i": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %31: any, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %33 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %33: any, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:   %35 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:dd:d": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %35: any, [%VS0.wasm_type_id_2]: any
;; CHECK-NEXT:   %37 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:id:d": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %37: any, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:   %39 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:l:l": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %39: any, [%VS0.wasm_type_id_4]: any
;; CHECK-NEXT:   %41 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_add_i32(): functionCode
;; CHECK-NEXT:   %42 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %43 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %44 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %41: object, %42: any, %43: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %41: object, [%VS0.exported_func_0]: any
;; CHECK-NEXT:   %46 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_void_func(): functionCode
;; CHECK-NEXT:   %47 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %48 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:   %49 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %46: object, %47: any, %48: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %46: object, [%VS0.exported_func_1]: any
;; CHECK-NEXT:   %51 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_add_f64(): functionCode
;; CHECK-NEXT:   %52 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:   %53 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_2]: any
;; CHECK-NEXT:   %54 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %51: object, %52: any, %53: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %51: object, [%VS0.exported_func_2]: any
;; CHECK-NEXT:   %56 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_mixed(): functionCode
;; CHECK-NEXT:   %57 = LoadFrameInst (:any) %0: environment, [%VS0.closure_3]: any
;; CHECK-NEXT:   %58 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:   %59 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %56: object, %57: any, %58: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %56: object, [%VS0.exported_func_3]: any
;; CHECK-NEXT:   %61 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_id_i64(): functionCode
;; CHECK-NEXT:   %62 = LoadFrameInst (:any) %0: environment, [%VS0.closure_4]: any
;; CHECK-NEXT:   %63 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_4]: any
;; CHECK-NEXT:   %64 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %61: object, %62: any, %63: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %61: object, [%VS0.exported_func_4]: any
;; CHECK-NEXT:   %66 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:   %67 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_0]: any
;; CHECK-NEXT:         StorePropertyStrictInst %67: any, %66: object, "add_i32": string
;; CHECK-NEXT:   %69 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_1]: any
;; CHECK-NEXT:         StorePropertyStrictInst %69: any, %66: object, "void_func": string
;; CHECK-NEXT:   %71 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_2]: any
;; CHECK-NEXT:         StorePropertyStrictInst %71: any, %66: object, "add_f64": string
;; CHECK-NEXT:   %73 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_3]: any
;; CHECK-NEXT:         StorePropertyStrictInst %73: any, %66: object, "mixed": string
;; CHECK-NEXT:   %75 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_4]: any
;; CHECK-NEXT:         StorePropertyStrictInst %75: any, %66: object, "id_i64": string
;; CHECK-NEXT:         ReturnInst %66: object
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_export_add_i32(p0: any, p1: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:   %3 = AsInt32Inst (:number) %2: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:   %5 = AsInt32Inst (:number) %4: any
;; CHECK-NEXT:   %6 = CallInst (:any) %1: any, %wasm_func_0(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %3: number, %5: number
;; CHECK-NEXT:        ReturnInst %6: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_export_void_func(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %2 = CallInst (:any) %1: any, %wasm_func_1(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_export_add_f64(p0: any, p1: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:   %3 = AsNumberInst (:number) %2: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:   %5 = AsNumberInst (:number) %4: any
;; CHECK-NEXT:   %6 = CallInst (:any) %1: any, %wasm_func_2(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %3: number, %5: number
;; CHECK-NEXT:        ReturnInst %6: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_export_mixed(p0: any, p1: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_3]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:   %3 = AsInt32Inst (:number) %2: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:   %5 = AsNumberInst (:number) %4: any
;; CHECK-NEXT:   %6 = CallInst (:any) %1: any, %wasm_func_3(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %3: number, %5: number
;; CHECK-NEXT:        ReturnInst %6: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_export_id_i64(p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_4]: any
;; CHECK-NEXT:   %2 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %3 = LoadFrameInst (:any) %0: environment, [%VS0.retBufF]: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:   %5 = CallBuiltinInst (:any) [HermesBuiltin.wasmBigIntToI64]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: any, %4: any
;; CHECK-NEXT:   %6 = LoadPropertyInst (:any) %2: any, 0: number
;; CHECK-NEXT:   %7 = AsInt32Inst (:number) %6: any
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %2: any, 1: number
;; CHECK-NEXT:   %9 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %10 = CallInst (:any) %1: any, %wasm_func_4(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %2: any, %3: any, %7: number, %9: number
;; CHECK-NEXT:   %11 = LoadPropertyInst (:any) %2: any, 0: number
;; CHECK-NEXT:   %12 = AsInt32Inst (:number) %11: any
;; CHECK-NEXT:   %13 = LoadPropertyInst (:any) %2: any, 1: number
;; CHECK-NEXT:   %14 = AsInt32Inst (:number) %13: any
;; CHECK-NEXT:   %15 = CallBuiltinInst (:bigint) [HermesBuiltin.wasmI64ToBigInt]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %12: number, %14: number
;; CHECK-NEXT:         ReturnInst %15: bigint
;; CHECK-NEXT: function_end
