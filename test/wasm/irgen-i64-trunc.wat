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

;; CHECK: scope %VS0 [wasm_type_id_0: any, wasm_type_id_1: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any, closure_2: any, closure_3: any, closure_4: any, closure_5: any, closure_6: any, closure_7: any]
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
;; CHECK-NEXT: function wasm_func_0(retbuf_I: any, retbuf_F: any, p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %retbuf_I: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %retbuf_F: any
;; CHECK-NEXT:   %3 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %4: any, %3: any
;; CHECK-NEXT:   %6 = LoadStackInst (:any) %3: any
;; CHECK-NEXT:   %7 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64TruncF64S]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, %6: any
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: any, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: any, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_1(retbuf_I: any, retbuf_F: any, p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %retbuf_I: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %retbuf_F: any
;; CHECK-NEXT:   %3 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %4: any, %3: any
;; CHECK-NEXT:   %6 = LoadStackInst (:any) %3: any
;; CHECK-NEXT:   %7 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64TruncF64U]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, %6: any
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: any, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: any, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_2(retbuf_I: any, retbuf_F: any, p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %retbuf_I: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %retbuf_F: any
;; CHECK-NEXT:   %3 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %4: any, %3: any
;; CHECK-NEXT:   %6 = LoadStackInst (:any) %3: any
;; CHECK-NEXT:   %7 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64TruncF64S]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, %6: any
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: any, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: any, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_3(retbuf_I: any, retbuf_F: any, p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %retbuf_I: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %retbuf_F: any
;; CHECK-NEXT:   %3 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %4: any, %3: any
;; CHECK-NEXT:   %6 = LoadStackInst (:any) %3: any
;; CHECK-NEXT:   %7 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64TruncF64U]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, %6: any
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: any, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: any, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_4(retbuf_I: any, retbuf_F: any, p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %retbuf_I: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %retbuf_F: any
;; CHECK-NEXT:   %3 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %4: any, %3: any
;; CHECK-NEXT:   %6 = LoadStackInst (:any) %3: any
;; CHECK-NEXT:   %7 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64TruncSatF64S]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, %6: any
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: any, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: any, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_5(retbuf_I: any, retbuf_F: any, p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %retbuf_I: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %retbuf_F: any
;; CHECK-NEXT:   %3 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %4: any, %3: any
;; CHECK-NEXT:   %6 = LoadStackInst (:any) %3: any
;; CHECK-NEXT:   %7 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64TruncSatF64U]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, %6: any
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: any, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: any, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_6(retbuf_I: any, retbuf_F: any, p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %retbuf_I: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %retbuf_F: any
;; CHECK-NEXT:   %3 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %4: any, %3: any
;; CHECK-NEXT:   %6 = LoadStackInst (:any) %3: any
;; CHECK-NEXT:   %7 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64TruncSatF64S]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, %6: any
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: any, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: any, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_7(retbuf_I: any, retbuf_F: any, p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %retbuf_I: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %retbuf_F: any
;; CHECK-NEXT:   %3 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %4: any, %3: any
;; CHECK-NEXT:   %6 = LoadStackInst (:any) %3: any
;; CHECK-NEXT:   %7 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64TruncSatF64U]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, %6: any
;; CHECK-NEXT:   %8 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %9 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %8: any
;; CHECK-NEXT:   %11 = AsInt32Inst (:number) %9: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %13 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:   %14 = PhiInst (:number) %11: number, %BB0
;; CHECK-NEXT:         StorePropertyStrictInst %13: number, %1: any, 0: number
;; CHECK-NEXT:         StorePropertyStrictInst %14: number, %1: any, 1: number
;; CHECK-NEXT:         ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function __wasm_instantiate__(): any
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
;; CHECK-NEXT:   %11 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_5(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %11: object, [%VS0.closure_5]: any
;; CHECK-NEXT:   %13 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_6(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %13: object, [%VS0.closure_6]: any
;; CHECK-NEXT:   %15 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_7(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %15: object, [%VS0.closure_7]: any
;; CHECK-NEXT:   %17 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:   %18 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:   %19 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:   %20 = CreateThisInst (:any) %17: any, %17: any, empty: any
;; CHECK-NEXT:   %21 = CallInst (:any) %17: any, empty: any, false: boolean, empty: any, %17: any, %20: any, 8: number
;; CHECK-NEXT:   %22 = GetConstructedObjectInst (:object) %20: any, %21: any
;; CHECK-NEXT:   %23 = CreateThisInst (:any) %18: any, %18: any, empty: any
;; CHECK-NEXT:   %24 = CallInst (:any) %18: any, empty: any, false: boolean, empty: any, %18: any, %23: any, %22: object
;; CHECK-NEXT:   %25 = GetConstructedObjectInst (:object) %23: any, %24: any
;; CHECK-NEXT:   %26 = CreateThisInst (:any) %19: any, %19: any, empty: any
;; CHECK-NEXT:   %27 = CallInst (:any) %19: any, empty: any, false: boolean, empty: any, %19: any, %26: any, %22: object
;; CHECK-NEXT:   %28 = GetConstructedObjectInst (:object) %26: any, %27: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %25: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %28: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %31 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         ReturnInst %31: object
;; CHECK-NEXT: function_end
