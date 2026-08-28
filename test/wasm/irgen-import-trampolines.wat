;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for import trampoline functions (I.2).
;; Verifies argument marshaling and return value conversion for
;; various imported function signatures.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Import 1: void function with one i32 param.
  (import "env" "log" (func $log (param i32)))

  ;; Import 2: i32 function with two i32 params.
  (import "env" "add" (func $add (param i32 i32) (result i32)))

  ;; Import 3: void function with no params.
  (import "env" "init" (func $init))

  ;; Import 4: f64 function with two f64 params.
  (import "env" "f64_add" (func $f64_add (param f64 f64) (result f64)))

  ;; Import 5: i64 function with one i64 param (BigInt conversion).
  (import "env" "i64_id" (func $i64_id (param i64) (result i64)))

  ;; A defined function that calls all imports.
  (func (export "test")
    i32.const 42
    call $log

    i32.const 3
    i32.const 4
    call $add
    drop

    call $init

    f64.const 1.5
    f64.const 2.5
    call $f64_add
    drop

    i64.const 100
    call $i64_id
    drop
  )
)

;; Import trampoline 1: $log(i32) -> void.
;; Loads the imported JS function, passes the i32 param, returns undefined.

;; Import trampoline 2: $add(i32, i32) -> i32.
;; Two params, AsInt32Inst on return value.

;; Import trampoline 3: $init() -> void.
;; No params, returns undefined.

;; Import trampoline 4: $f64_add(f64, f64) -> f64.
;; Float params pass through, result returned directly.

;; Import trampoline 5: $i64_id(i64) -> i64.
;; i64 param splits into two JS params (lo, hi). Trampoline converts to BigInt.
;; i64 return: BigInt converted back to split (lo, hi).

;; CHECK: scope %VS0 [wasm_type_id_0: any, wasm_type_id_1: any, wasm_type_id_2: any, wasm_type_id_3: any, wasm_type_id_4: any, import_func_0: any, import_func_1: any, import_func_2: any, import_func_3: any, import_func_4: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any, closure_2: any, closure_3: any, closure_4: any, closure_5: any]
;; CHECK-EMPTY:
;; CHECK-NEXT: function global(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:   %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %__wasm_instantiate__(): functionCode
;; CHECK-NEXT:   %2 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK-NEXT:   %3 = CreateThisInst (:any) %2: any, %2: any, empty: any
;; CHECK-NEXT:   %4 = CallInst (:any) %2: any, empty: any, false: boolean, empty: any, %2: any, %3: any, 1: number
;; CHECK-NEXT:   %5 = GetConstructedObjectInst (:object) %3: any, %4: any
;; CHECK-NEXT:   %6 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:        StorePropertyStrictInst "test": string, %6: object, "name": string
;; CHECK-NEXT:        StorePropertyStrictInst "function": string, %6: object, "kind": string
;; CHECK-NEXT:        StorePropertyStrictInst %6: object, %5: object, 0: number
;; CHECK-NEXT:   %10 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK-NEXT:   %11 = CreateThisInst (:any) %10: any, %10: any, empty: any
;; CHECK-NEXT:   %12 = CallInst (:any) %10: any, empty: any, false: boolean, empty: any, %10: any, %11: any, 5: number
;; CHECK-NEXT:   %13 = GetConstructedObjectInst (:object) %11: any, %12: any
;; CHECK-NEXT:   %14 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "env": string, %14: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "log": string, %14: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %14: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %14: object, %13: object, 0: number
;; CHECK-NEXT:   %19 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "env": string, %19: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "add": string, %19: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %19: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %19: object, %13: object, 1: number
;; CHECK-NEXT:   %24 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "env": string, %24: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "init": string, %24: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %24: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %24: object, %13: object, 2: number
;; CHECK-NEXT:   %29 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "env": string, %29: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "f64_add": string, %29: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %29: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %29: object, %13: object, 3: number
;; CHECK-NEXT:   %34 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "env": string, %34: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "i64_id": string, %34: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %34: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %34: object, %13: object, 4: number
;; CHECK-NEXT:   %39 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst %1: object, %39: object, "instantiate": string
;; CHECK-NEXT:         StorePropertyStrictInst %5: object, %39: object, "exportDescs": string
;; CHECK-NEXT:         StorePropertyStrictInst %13: object, %39: object, "importDescs": string
;; CHECK-NEXT:         ReturnInst %39: object
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_0(p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.import_func_0]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:   %3 = CallInst (:any) %1: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: any
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_1(p0: any, p1: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.import_func_1]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:   %3 = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:   %4 = CallInst (:any) %1: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: any, %3: any
;; CHECK-NEXT:   %5 = AsInt32Inst (:number) %4: any
;; CHECK-NEXT:        ReturnInst %5: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_2(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.import_func_2]: any
;; CHECK-NEXT:   %2 = CallInst (:any) %1: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_3(p0: any, p1: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.import_func_3]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:   %3 = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:   %4 = CallInst (:any) %1: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: any, %3: any
;; CHECK-NEXT:        ReturnInst %4: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_4(retbuf_I: any, retbuf_F: any, p0_lo: any, p0_hi: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.import_func_4]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %retbuf_I: any
;; CHECK-NEXT:   %3 = LoadParamInst (:any) %retbuf_F: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p0_lo: any
;; CHECK-NEXT:   %5 = LoadParamInst (:any) %p0_hi: any
;; CHECK-NEXT:   %6 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64ToBigInt]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %4: any, %5: any
;; CHECK-NEXT:   %7 = CallInst (:any) %1: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %6: any
;; CHECK-NEXT:   %8 = CallBuiltinInst (:any) [HermesBuiltin.wasmBigIntToI64]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: any, %7: any
;; CHECK-NEXT:        ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_5(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %3 = CallInst (:any) %2: any, %wasm_func_0(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, 42: number
;; CHECK-NEXT:   %4 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %5 = CallInst (:any) %4: any, %wasm_func_1(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, 3: number, 4: number
;; CHECK-NEXT:   %6 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:   %7 = CallInst (:any) %6: any, %wasm_func_2(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:   %8 = LoadFrameInst (:any) %0: environment, [%VS0.closure_3]: any
;; CHECK-NEXT:   %9 = CallInst (:any) %8: any, %wasm_func_3(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, 1.5: number, 2.5: number
;; CHECK-NEXT:   %10 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %11 = LoadFrameInst (:any) %0: environment, [%VS0.retBufF]: any
;; CHECK-NEXT:   %12 = LoadFrameInst (:any) %0: environment, [%VS0.closure_4]: any
;; CHECK-NEXT:   %13 = CallInst (:any) %12: any, %wasm_func_4(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %10: any, %11: any, 100: number, 0: number
;; CHECK-NEXT:   %14 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %15 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %16 = AsInt32Inst (:number) %14: any
;; CHECK-NEXT:   %17 = AsInt32Inst (:number) %15: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:         ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function __wasm_instantiate__(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:   %1 = TryLoadGlobalPropertyInst (:any) globalObject: object, "__wasm_imports__": string
;; CHECK-NEXT:   %2 = LoadPropertyInst (:any) %1: any, "env": string
;; CHECK-NEXT:   %3 = BinaryStrictlyEqualInst (:any) %2: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %3: any, %BB1, %BB2
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %5 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import module": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT: %BB2:
;; CHECK-NEXT:   %7 = LoadPropertyInst (:any) %2: any, "log": string
;; CHECK-NEXT:   %8 = BinaryStrictlyEqualInst (:any) %7: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %8: any, %BB3, %BB4
;; CHECK-NEXT: %BB3:
;; CHECK-NEXT:   %10 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB4:
;; CHECK-NEXT:   %12 = LoadPropertyInst (:any) %7: any, "__wasm_type__": string
;; CHECK-NEXT:   %13 = BinaryStrictlyEqualInst (:any) %12: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %13: any, %BB5, %BB6
;; CHECK-NEXT: %BB5:
;; CHECK-NEXT:   %15 = TypeOfInst (:string) %7: any
;; CHECK-NEXT:   %16 = BinaryStrictlyEqualInst (:any) %15: string, "function": string
;; CHECK-NEXT:         CondBranchInst %16: any, %BB7, %BB8
;; CHECK-NEXT: %BB6:
;; CHECK-NEXT:   %18 = BinaryStrictlyNotEqualInst (:any) %12: any, "func:i:": string
;; CHECK-NEXT:         CondBranchInst %18: any, %BB8, %BB7
;; CHECK-NEXT: %BB7:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %7: any, [%VS0.import_func_0]: any
;; CHECK-NEXT:   %21 = LoadPropertyInst (:any) %2: any, "add": string
;; CHECK-NEXT:   %22 = BinaryStrictlyEqualInst (:any) %21: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %22: any, %BB9, %BB10
;; CHECK-NEXT: %BB8:
;; CHECK-NEXT:   %24 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "incompatible import type": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB9:
;; CHECK-NEXT:   %26 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB10:
;; CHECK-NEXT:   %28 = LoadPropertyInst (:any) %21: any, "__wasm_type__": string
;; CHECK-NEXT:   %29 = BinaryStrictlyEqualInst (:any) %28: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %29: any, %BB11, %BB12
;; CHECK-NEXT: %BB11:
;; CHECK-NEXT:   %31 = TypeOfInst (:string) %21: any
;; CHECK-NEXT:   %32 = BinaryStrictlyEqualInst (:any) %31: string, "function": string
;; CHECK-NEXT:         CondBranchInst %32: any, %BB13, %BB14
;; CHECK-NEXT: %BB12:
;; CHECK-NEXT:   %34 = BinaryStrictlyNotEqualInst (:any) %28: any, "func:ii:i": string
;; CHECK-NEXT:         CondBranchInst %34: any, %BB14, %BB13
;; CHECK-NEXT: %BB13:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %21: any, [%VS0.import_func_1]: any
;; CHECK-NEXT:   %37 = LoadPropertyInst (:any) %2: any, "init": string
;; CHECK-NEXT:   %38 = BinaryStrictlyEqualInst (:any) %37: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %38: any, %BB15, %BB16
;; CHECK-NEXT: %BB14:
;; CHECK-NEXT:   %40 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "incompatible import type": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB15:
;; CHECK-NEXT:   %42 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB16:
;; CHECK-NEXT:   %44 = LoadPropertyInst (:any) %37: any, "__wasm_type__": string
;; CHECK-NEXT:   %45 = BinaryStrictlyEqualInst (:any) %44: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %45: any, %BB17, %BB18
;; CHECK-NEXT: %BB17:
;; CHECK-NEXT:   %47 = TypeOfInst (:string) %37: any
;; CHECK-NEXT:   %48 = BinaryStrictlyEqualInst (:any) %47: string, "function": string
;; CHECK-NEXT:         CondBranchInst %48: any, %BB19, %BB20
;; CHECK-NEXT: %BB18:
;; CHECK-NEXT:   %50 = BinaryStrictlyNotEqualInst (:any) %44: any, "func::": string
;; CHECK-NEXT:         CondBranchInst %50: any, %BB20, %BB19
;; CHECK-NEXT: %BB19:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %37: any, [%VS0.import_func_2]: any
;; CHECK-NEXT:   %53 = LoadPropertyInst (:any) %2: any, "f64_add": string
;; CHECK-NEXT:   %54 = BinaryStrictlyEqualInst (:any) %53: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %54: any, %BB21, %BB22
;; CHECK-NEXT: %BB20:
;; CHECK-NEXT:   %56 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "incompatible import type": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB21:
;; CHECK-NEXT:   %58 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB22:
;; CHECK-NEXT:   %60 = LoadPropertyInst (:any) %53: any, "__wasm_type__": string
;; CHECK-NEXT:   %61 = BinaryStrictlyEqualInst (:any) %60: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %61: any, %BB23, %BB24
;; CHECK-NEXT: %BB23:
;; CHECK-NEXT:   %63 = TypeOfInst (:string) %53: any
;; CHECK-NEXT:   %64 = BinaryStrictlyEqualInst (:any) %63: string, "function": string
;; CHECK-NEXT:         CondBranchInst %64: any, %BB25, %BB26
;; CHECK-NEXT: %BB24:
;; CHECK-NEXT:   %66 = BinaryStrictlyNotEqualInst (:any) %60: any, "func:dd:d": string
;; CHECK-NEXT:         CondBranchInst %66: any, %BB26, %BB25
;; CHECK-NEXT: %BB25:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %53: any, [%VS0.import_func_3]: any
;; CHECK-NEXT:   %69 = LoadPropertyInst (:any) %2: any, "i64_id": string
;; CHECK-NEXT:   %70 = BinaryStrictlyEqualInst (:any) %69: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %70: any, %BB27, %BB28
;; CHECK-NEXT: %BB26:
;; CHECK-NEXT:   %72 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "incompatible import type": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB27:
;; CHECK-NEXT:   %74 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB28:
;; CHECK-NEXT:   %76 = LoadPropertyInst (:any) %69: any, "__wasm_type__": string
;; CHECK-NEXT:   %77 = BinaryStrictlyEqualInst (:any) %76: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %77: any, %BB29, %BB30
;; CHECK-NEXT: %BB29:
;; CHECK-NEXT:   %79 = TypeOfInst (:string) %69: any
;; CHECK-NEXT:   %80 = BinaryStrictlyEqualInst (:any) %79: string, "function": string
;; CHECK-NEXT:         CondBranchInst %80: any, %BB31, %BB32
;; CHECK-NEXT: %BB30:
;; CHECK-NEXT:   %82 = BinaryStrictlyNotEqualInst (:any) %76: any, "func:l:l": string
;; CHECK-NEXT:         CondBranchInst %82: any, %BB32, %BB31
;; CHECK-NEXT: %BB31:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %69: any, [%VS0.import_func_4]: any
;; CHECK-NEXT:   %85 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_0(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %85: object, [%VS0.closure_0]: any
;; CHECK-NEXT:   %87 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_1(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %87: object, [%VS0.closure_1]: any
;; CHECK-NEXT:   %89 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_2(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %89: object, [%VS0.closure_2]: any
;; CHECK-NEXT:   %91 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_3(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %91: object, [%VS0.closure_3]: any
;; CHECK-NEXT:   %93 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_4(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %93: object, [%VS0.closure_4]: any
;; CHECK-NEXT:   %95 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_5(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %95: object, [%VS0.closure_5]: any
;; CHECK-NEXT:   %97 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:   %98 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:   %99 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:   %100 = CreateThisInst (:any) %97: any, %97: any, empty: any
;; CHECK-NEXT:   %101 = CallInst (:any) %97: any, empty: any, false: boolean, empty: any, %97: any, %100: any, 8: number
;; CHECK-NEXT:   %102 = GetConstructedObjectInst (:object) %100: any, %101: any
;; CHECK-NEXT:   %103 = CreateThisInst (:any) %98: any, %98: any, empty: any
;; CHECK-NEXT:   %104 = CallInst (:any) %98: any, empty: any, false: boolean, empty: any, %98: any, %103: any, %102: object
;; CHECK-NEXT:   %105 = GetConstructedObjectInst (:object) %103: any, %104: any
;; CHECK-NEXT:   %106 = CreateThisInst (:any) %99: any, %99: any, empty: any
;; CHECK-NEXT:   %107 = CallInst (:any) %99: any, empty: any, false: boolean, empty: any, %99: any, %106: any, %102: object
;; CHECK-NEXT:   %108 = GetConstructedObjectInst (:object) %106: any, %107: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %105: object, [%VS0.retBufI]: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %108: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %111 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:   %112 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_test(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func::": string, %112: object, "__wasm_type__": string
;; CHECK-NEXT:          StorePropertyStrictInst %112: object, %111: object, "test": string
;; CHECK-NEXT:          ReturnInst %111: object
;; CHECK-NEXT: %BB32:
;; CHECK-NEXT:   %116 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "incompatible import type": string
;; CHECK-NEXT:          UnreachableInst
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_export_test(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_5]: any
;; CHECK-NEXT:   %2 = CallInst (:any) %1: any, %wasm_func_5(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
