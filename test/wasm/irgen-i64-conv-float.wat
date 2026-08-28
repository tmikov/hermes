;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test i64→float conversions and reinterpret (G.4c).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; f64.convert_i64_s: signed i64 → f64
  (func $f64_convert_i64_s (result f64)
    i64.const 42
    f64.convert_i64_s)


  ;; f64.convert_i64_u: unsigned i64 → f64
  (func $f64_convert_i64_u (result f64)
    i64.const -1  ;; = 0xFFFFFFFF_FFFFFFFF unsigned
    f64.convert_i64_u)


  ;; f32.convert_i64_s: signed i64 → f32
  (func $f32_convert_i64_s (result f32)
    i64.const 100
    f32.convert_i64_s)


  ;; f32.convert_i64_u: unsigned i64 → f32
  (func $f32_convert_i64_u (result f32)
    i64.const 200
    f32.convert_i64_u)


  ;; i64.reinterpret_f64: bitcast f64 to i64
  (func $i64_reinterpret_f64 (result i64)
    f64.const 1.0
    i64.reinterpret_f64)

;; Constant-folded: f64.const 1.0 has bits 0x3FF0000000000000 (lo=0, hi=1072693248).

  ;; f64.reinterpret_i64: bitcast i64 to f64
  (func $f64_reinterpret_i64 (result f64)
    i64.const 4607182418800017408  ;; 0x3FF0000000000000 = 1.0 as f64 bits
    f64.reinterpret_i64)

)

;; CHECK: scope %VS0 [wasm_type_id_0: any, wasm_type_id_1: any, wasm_type_id_2: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any, closure_2: any, closure_3: any, closure_4: any, closure_5: any]
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
;; CHECK-NEXT: function wasm_func_0(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:any) [HermesBuiltin.wasmF64ConvertI64S]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, 42: number, 0: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %4 = PhiInst (:any) %2: any, %BB0
;; CHECK-NEXT:        ReturnInst %4: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_1(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:any) [HermesBuiltin.wasmF64ConvertI64U]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, -1: number, -1: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %4 = PhiInst (:any) %2: any, %BB0
;; CHECK-NEXT:        ReturnInst %4: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_2(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:any) [HermesBuiltin.wasmF32ConvertI64S]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, 100: number, 0: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %4 = PhiInst (:any) %2: any, %BB0
;; CHECK-NEXT:        ReturnInst %4: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_3(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:any) [HermesBuiltin.wasmF32ConvertI64U]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, 200: number, 0: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %4 = PhiInst (:any) %2: any, %BB0
;; CHECK-NEXT:        ReturnInst %4: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_4(retbuf_I: any, retbuf_F: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %retbuf_I: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %retbuf_F: any
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %4 = PhiInst (:number) 0: number, %BB0
;; CHECK-NEXT:   %5 = PhiInst (:number) 1072693248: number, %BB0
;; CHECK-NEXT:        StorePropertyStrictInst %4: number, %1: any, 0: number
;; CHECK-NEXT:        StorePropertyStrictInst %5: number, %1: any, 1: number
;; CHECK-NEXT:        ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_5(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:any) [HermesBuiltin.wasmF64ReinterpretI64]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, 0: number, 1072693248: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %4 = PhiInst (:any) %2: any, %BB0
;; CHECK-NEXT:        ReturnInst %4: any
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
;; CHECK-NEXT:   %13 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:   %14 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:   %15 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:   %16 = CreateThisInst (:any) %13: any, %13: any, empty: any
;; CHECK-NEXT:   %17 = CallInst (:any) %13: any, empty: any, false: boolean, empty: any, %13: any, %16: any, 8: number
;; CHECK-NEXT:   %18 = GetConstructedObjectInst (:object) %16: any, %17: any
;; CHECK-NEXT:   %19 = CreateThisInst (:any) %14: any, %14: any, empty: any
;; CHECK-NEXT:   %20 = CallInst (:any) %14: any, empty: any, false: boolean, empty: any, %14: any, %19: any, %18: object
;; CHECK-NEXT:   %21 = GetConstructedObjectInst (:object) %19: any, %20: any
;; CHECK-NEXT:   %22 = CreateThisInst (:any) %15: any, %15: any, empty: any
;; CHECK-NEXT:   %23 = CallInst (:any) %15: any, empty: any, false: boolean, empty: any, %15: any, %22: any, %18: object
;; CHECK-NEXT:   %24 = GetConstructedObjectInst (:object) %22: any, %23: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %21: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %24: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %27 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         ReturnInst %27: object
;; CHECK-NEXT: function_end
