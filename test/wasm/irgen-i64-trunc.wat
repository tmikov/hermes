;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test i64 truncation operations (G.4b): float→i64 trapping and saturating.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; i64.trunc_f64_s: trapping truncation from f64 to signed i64
  (func $trunc_f64_s (param f64) (result i64)
    local.get 0
    i64.trunc_f64_s)


  ;; i64.trunc_f64_u: trapping truncation from f64 to unsigned i64
  (func $trunc_f64_u (param f64) (result i64)
    local.get 0
    i64.trunc_f64_u)


  ;; i64.trunc_f32_s: same as f64 in Phase 1
  (func $trunc_f32_s (param f32) (result i64)
    local.get 0
    i64.trunc_f32_s)


  ;; i64.trunc_f32_u: same as f64 in Phase 1
  (func $trunc_f32_u (param f32) (result i64)
    local.get 0
    i64.trunc_f32_u)


  ;; i64.trunc_sat_f64_s: saturating truncation from f64 to signed i64
  (func $trunc_sat_f64_s (param f64) (result i64)
    local.get 0
    i64.trunc_sat_f64_s)


  ;; i64.trunc_sat_f64_u: saturating truncation from f64 to unsigned i64
  (func $trunc_sat_f64_u (param f64) (result i64)
    local.get 0
    i64.trunc_sat_f64_u)


  ;; i64.trunc_sat_f32_s: same as f64 sat in Phase 1
  (func $trunc_sat_f32_s (param f32) (result i64)
    local.get 0
    i64.trunc_sat_f32_s)


  ;; i64.trunc_sat_f32_u: same as f64 sat in Phase 1
  (func $trunc_sat_f32_u (param f32) (result i64)
    local.get 0
    i64.trunc_sat_f32_u)

)

;; CHECK: scope %VS0 [wasm_type_id_0: any, wasm_type_id_1: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any, closure_2: any, closure_3: any, closure_4: any, closure_5: any, closure_6: any, closure_7: any, intrinsics: any]
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
;; CHECK-NEXT: function wasm_func_0(retbuf_I: object, retbuf_F: object, p0: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT:   %2 = LoadParamInst (:object) %retbuf_F: object
;; CHECK-NEXT:   %3 = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:        StoreStackInst %4: number, %3: number
;; CHECK-NEXT:   %6 = LoadStackInst (:number) %3: number
;; CHECK-NEXT:   %7 = CallBuiltinInst (:number) [HermesBuiltin.wasmI64TruncF64S]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: object, %6: number
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: object, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: object, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: object, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: object, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_1(retbuf_I: object, retbuf_F: object, p0: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT:   %2 = LoadParamInst (:object) %retbuf_F: object
;; CHECK-NEXT:   %3 = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:        StoreStackInst %4: number, %3: number
;; CHECK-NEXT:   %6 = LoadStackInst (:number) %3: number
;; CHECK-NEXT:   %7 = CallBuiltinInst (:number) [HermesBuiltin.wasmI64TruncF64U]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: object, %6: number
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: object, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: object, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: object, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: object, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_2(retbuf_I: object, retbuf_F: object, p0: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT:   %2 = LoadParamInst (:object) %retbuf_F: object
;; CHECK-NEXT:   %3 = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:        StoreStackInst %4: number, %3: number
;; CHECK-NEXT:   %6 = LoadStackInst (:number) %3: number
;; CHECK-NEXT:   %7 = CallBuiltinInst (:number) [HermesBuiltin.wasmI64TruncF64S]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: object, %6: number
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: object, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: object, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: object, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: object, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_3(retbuf_I: object, retbuf_F: object, p0: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT:   %2 = LoadParamInst (:object) %retbuf_F: object
;; CHECK-NEXT:   %3 = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:        StoreStackInst %4: number, %3: number
;; CHECK-NEXT:   %6 = LoadStackInst (:number) %3: number
;; CHECK-NEXT:   %7 = CallBuiltinInst (:number) [HermesBuiltin.wasmI64TruncF64U]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: object, %6: number
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: object, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: object, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: object, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: object, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_4(retbuf_I: object, retbuf_F: object, p0: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT:   %2 = LoadParamInst (:object) %retbuf_F: object
;; CHECK-NEXT:   %3 = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:        StoreStackInst %4: number, %3: number
;; CHECK-NEXT:   %6 = LoadStackInst (:number) %3: number
;; CHECK-NEXT:   %7 = CallBuiltinInst (:number) [HermesBuiltin.wasmI64TruncSatF64S]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: object, %6: number
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: object, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: object, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: object, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: object, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_5(retbuf_I: object, retbuf_F: object, p0: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT:   %2 = LoadParamInst (:object) %retbuf_F: object
;; CHECK-NEXT:   %3 = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:        StoreStackInst %4: number, %3: number
;; CHECK-NEXT:   %6 = LoadStackInst (:number) %3: number
;; CHECK-NEXT:   %7 = CallBuiltinInst (:number) [HermesBuiltin.wasmI64TruncSatF64U]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: object, %6: number
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: object, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: object, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: object, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: object, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_6(retbuf_I: object, retbuf_F: object, p0: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT:   %2 = LoadParamInst (:object) %retbuf_F: object
;; CHECK-NEXT:   %3 = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:        StoreStackInst %4: number, %3: number
;; CHECK-NEXT:   %6 = LoadStackInst (:number) %3: number
;; CHECK-NEXT:   %7 = CallBuiltinInst (:number) [HermesBuiltin.wasmI64TruncSatF64S]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: object, %6: number
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: object, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: object, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: object, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: object, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_7(retbuf_I: object, retbuf_F: object, p0: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT:   %2 = LoadParamInst (:object) %retbuf_F: object
;; CHECK-NEXT:   %3 = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:        StoreStackInst %4: number, %3: number
;; CHECK-NEXT:   %6 = LoadStackInst (:number) %3: number
;; CHECK-NEXT:   %7 = CallBuiltinInst (:number) [HermesBuiltin.wasmI64TruncSatF64U]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: object, %6: number
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: object, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: object, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: object, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: object, 1: number
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
;; CHECK-NEXT:   %14 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_5(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %14: object, [%VS0.closure_5]: any
;; CHECK-NEXT:   %16 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_6(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %16: object, [%VS0.closure_6]: any
;; CHECK-NEXT:   %18 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_7(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %18: object, [%VS0.closure_7]: any
;; CHECK-NEXT:   %20 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %21 = LoadPropertyInst (:any) %20: any, "ArrayBuffer": string
;; CHECK-NEXT:   %22 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %23 = LoadPropertyInst (:any) %22: any, "Uint32Array": string
;; CHECK-NEXT:   %24 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %25 = LoadPropertyInst (:any) %24: any, "Float64Array": string
;; CHECK-NEXT:   %26 = CreateThisInst (:any) %21: any, %21: any, empty: any
;; CHECK-NEXT:   %27 = CallInst (:any) %21: any, empty: any, false: boolean, empty: any, %21: any, %26: any, 8: number
;; CHECK-NEXT:   %28 = GetConstructedObjectInst (:object) %26: any, %27: any
;; CHECK-NEXT:   %29 = CreateThisInst (:any) %23: any, %23: any, empty: any
;; CHECK-NEXT:   %30 = CallInst (:any) %23: any, empty: any, false: boolean, empty: any, %23: any, %29: any, %28: object
;; CHECK-NEXT:   %31 = GetConstructedObjectInst (:object) %29: any, %30: any
;; CHECK-NEXT:   %32 = CreateThisInst (:any) %25: any, %25: any, empty: any
;; CHECK-NEXT:   %33 = CallInst (:any) %25: any, empty: any, false: boolean, empty: any, %25: any, %32: any, %28: object
;; CHECK-NEXT:   %34 = GetConstructedObjectInst (:object) %32: any, %33: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %31: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %34: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %37 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         ReturnInst %37: object
;; CHECK-NEXT: function_end
