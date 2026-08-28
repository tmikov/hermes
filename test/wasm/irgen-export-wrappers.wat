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

;; CHECK: scope %VS0 [wasm_type_id_0: any, wasm_type_id_1: any, wasm_type_id_2: any, wasm_type_id_3: any, wasm_type_id_4: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any, closure_2: any, closure_3: any, closure_4: any]
;; CHECK-EMPTY:
;; CHECK-NEXT: function global(): object
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:   %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %__wasm_instantiate__(): functionCode
;; CHECK-NEXT:   %2 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK-NEXT:   %3 = CreateThisInst (:any) %2: any, %2: any, empty: any
;; CHECK-NEXT:   %4 = CallInst (:any) %2: any, empty: any, false: boolean, empty: any, %2: any, %3: any, 5: number
;; CHECK-NEXT:   %5 = GetConstructedObjectInst (:object) %3: any, %4: any
;; CHECK-NEXT:   %6 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:        StorePropertyStrictInst "add_i32": string, %6: object, "name": string
;; CHECK-NEXT:        StorePropertyStrictInst "function": string, %6: object, "kind": string
;; CHECK-NEXT:        StorePropertyStrictInst %6: object, %5: object, 0: number
;; CHECK-NEXT:   %10 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "void_func": string, %10: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %10: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %10: object, %5: object, 1: number
;; CHECK-NEXT:   %14 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "add_f64": string, %14: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %14: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %14: object, %5: object, 2: number
;; CHECK-NEXT:   %18 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "mixed": string, %18: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %18: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %18: object, %5: object, 3: number
;; CHECK-NEXT:   %22 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "id_i64": string, %22: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %22: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %22: object, %5: object, 4: number
;; CHECK-NEXT:   %26 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK-NEXT:   %27 = CreateThisInst (:any) %26: any, %26: any, empty: any
;; CHECK-NEXT:   %28 = CallInst (:any) %26: any, empty: any, false: boolean, empty: any, %26: any, %27: any, 0: number
;; CHECK-NEXT:   %29 = GetConstructedObjectInst (:object) %27: any, %28: any
;; CHECK-NEXT:   %30 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst %1: object, %30: object, "instantiate": string
;; CHECK-NEXT:         StorePropertyStrictInst %5: object, %30: object, "exportDescs": string
;; CHECK-NEXT:         StorePropertyStrictInst %29: object, %30: object, "importDescs": string
;; CHECK-NEXT:         ReturnInst %30: object
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
;; CHECK-NEXT:   %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_0(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %1: object, [%VS0.closure_0]: any
;; CHECK-NEXT:   %3 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_1(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %3: object, [%VS0.closure_1]: any
;; CHECK-NEXT:   %5 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_2(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %5: object, [%VS0.closure_2]: any
;; CHECK-NEXT:   %7 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_3(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %7: object, [%VS0.closure_3]: any
;; CHECK-NEXT:   %9 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_4(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %9: object, [%VS0.closure_4]: any
;; CHECK-NEXT:   %11 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:   %12 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:   %13 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:   %14 = CreateThisInst (:any) %11: any, %11: any, empty: any
;; CHECK-NEXT:   %15 = CallInst (:any) %11: any, empty: any, false: boolean, empty: any, %11: any, %14: any, 8: number
;; CHECK-NEXT:   %16 = GetConstructedObjectInst (:object) %14: any, %15: any
;; CHECK-NEXT:   %17 = CreateThisInst (:any) %12: any, %12: any, empty: any
;; CHECK-NEXT:   %18 = CallInst (:any) %12: any, empty: any, false: boolean, empty: any, %12: any, %17: any, %16: object
;; CHECK-NEXT:   %19 = GetConstructedObjectInst (:object) %17: any, %18: any
;; CHECK-NEXT:   %20 = CreateThisInst (:any) %13: any, %13: any, empty: any
;; CHECK-NEXT:   %21 = CallInst (:any) %13: any, empty: any, false: boolean, empty: any, %13: any, %20: any, %16: object
;; CHECK-NEXT:   %22 = GetConstructedObjectInst (:object) %20: any, %21: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %19: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %22: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %25 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:   %26 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_add_i32(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func:ii:i": string, %26: object, "__wasm_type__": string
;; CHECK-NEXT:         StorePropertyStrictInst %26: object, %25: object, "add_i32": string
;; CHECK-NEXT:   %29 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_void_func(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func::": string, %29: object, "__wasm_type__": string
;; CHECK-NEXT:         StorePropertyStrictInst %29: object, %25: object, "void_func": string
;; CHECK-NEXT:   %32 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_add_f64(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func:dd:d": string, %32: object, "__wasm_type__": string
;; CHECK-NEXT:         StorePropertyStrictInst %32: object, %25: object, "add_f64": string
;; CHECK-NEXT:   %35 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_mixed(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func:id:d": string, %35: object, "__wasm_type__": string
;; CHECK-NEXT:         StorePropertyStrictInst %35: object, %25: object, "mixed": string
;; CHECK-NEXT:   %38 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_id_i64(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func:l:l": string, %38: object, "__wasm_type__": string
;; CHECK-NEXT:         StorePropertyStrictInst %38: object, %25: object, "id_i64": string
;; CHECK-NEXT:         ReturnInst %25: object
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
;; CHECK-NEXT:   %12 = LoadPropertyInst (:any) %2: any, 1: number
;; CHECK-NEXT:   %13 = CallBuiltinInst (:bigint) [HermesBuiltin.wasmI64ToBigInt]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %11: any, %12: any
;; CHECK-NEXT:         ReturnInst %13: bigint
;; CHECK-NEXT: function_end
