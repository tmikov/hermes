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

;; CHECK: scope %VS0 [wasm_type_id_0: any, wasm_type_id_1: any, wasm_type_id_2: any, wasm_type_id_3: any, wasm_type_id_4: any, import_func_0: any, import_func_1: any, import_func_2: any, import_func_3: any, import_func_4: any, retBufI: any, retBufF: any, closure_0: any, exported_func_0: any, closure_1: any, exported_func_1: any, closure_2: any, exported_func_2: any, closure_3: any, exported_func_3: any, closure_4: any, exported_func_4: any, closure_5: any, exported_func_5: any, intrinsics: any]
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
;; CHECK-NEXT:        StorePropertyStrictInst "test": string, %8: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %8: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %8: object, %7: object, 0: number
;; CHECK-NEXT:   %12 = LoadPropertyInst (:any) %3: any, "Array": string
;; CHECK-NEXT:   %13 = CreateThisInst (:any) %12: any, %12: any, empty: any
;; CHECK-NEXT:   %14 = CallInst (:any) %12: any, empty: any, false: boolean, empty: any, %12: any, %13: any, 5: number
;; CHECK-NEXT:   %15 = GetConstructedObjectInst (:object) %13: any, %14: any
;; CHECK-NEXT:   %16 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "env": string, %16: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "log": string, %16: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %16: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %16: object, %15: object, 0: number
;; CHECK-NEXT:   %21 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "env": string, %21: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "add": string, %21: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %21: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %21: object, %15: object, 1: number
;; CHECK-NEXT:   %26 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "env": string, %26: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "init": string, %26: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %26: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %26: object, %15: object, 2: number
;; CHECK-NEXT:   %31 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "env": string, %31: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "f64_add": string, %31: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %31: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %31: object, %15: object, 3: number
;; CHECK-NEXT:   %36 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "env": string, %36: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "i64_id": string, %36: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %36: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %36: object, %15: object, 4: number
;; CHECK-NEXT:   %41 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst %1: object, %41: object, "instantiate": string
;; CHECK-NEXT:         StorePropertyStrictInst %7: object, %41: object, "exportDescs": string
;; CHECK-NEXT:         StorePropertyStrictInst %15: object, %41: object, "importDescs": string
;; CHECK-NEXT:         ReturnInst %41: object
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_0(p0: number): undefined
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.import_func_0]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:   %3 = CallInst (:any) %1: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: number
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_1(p0: number, p1: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.import_func_1]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:   %3 = LoadParamInst (:number) %p1: number
;; CHECK-NEXT:   %4 = CallInst (:any) %1: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: number, %3: number
;; CHECK-NEXT:   %5 = AsInt32Inst (:number) %4: any
;; CHECK-NEXT:        ReturnInst %5: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_2(): undefined
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.import_func_2]: any
;; CHECK-NEXT:   %2 = CallInst (:any) %1: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_3(p0: number, p1: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.import_func_3]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:   %3 = LoadParamInst (:number) %p1: number
;; CHECK-NEXT:   %4 = CallInst (:any) %1: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: number, %3: number
;; CHECK-NEXT:   %5 = AsNumberInst (:number) %4: any
;; CHECK-NEXT:        ReturnInst %5: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_4(retbuf_I: object, retbuf_F: object, p0_lo: number, p0_hi: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.import_func_4]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT:   %3 = LoadParamInst (:object) %retbuf_F: object
;; CHECK-NEXT:   %4 = LoadParamInst (:number) %p0_lo: number
;; CHECK-NEXT:   %5 = LoadParamInst (:number) %p0_hi: number
;; CHECK-NEXT:   %6 = CallBuiltinInst (:bigint) [HermesBuiltin.wasmI64ToBigInt]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %4: number, %5: number
;; CHECK-NEXT:   %7 = CallInst (:any) %1: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %6: bigint
;; CHECK-NEXT:   %8 = CallBuiltinInst (:any) [HermesBuiltin.wasmBigIntToI64]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: object, %7: any
;; CHECK-NEXT:        ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_5(): undefined
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %3 = CallInst (:undefined) %2: any, %wasm_func_0(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, 42: number
;; CHECK-NEXT:   %4 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %5 = CallInst (:number) %4: any, %wasm_func_1(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, 3: number, 4: number
;; CHECK-NEXT:   %6 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:   %7 = CallInst (:undefined) %6: any, %wasm_func_2(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:   %8 = LoadFrameInst (:any) %0: environment, [%VS0.closure_3]: any
;; CHECK-NEXT:   %9 = CallInst (:number) %8: any, %wasm_func_3(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, 1.5: number, 2.5: number
;; CHECK-NEXT:   %10 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %11 = LoadFrameInst (:any) %0: environment, [%VS0.retBufF]: any
;; CHECK-NEXT:   %12 = LoadFrameInst (:any) %0: environment, [%VS0.closure_4]: any
;; CHECK-NEXT:   %13 = CallInst (:number) %12: any, %wasm_func_4(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %10: any, %11: any, 100: number, 0: number
;; CHECK-NEXT:   %14 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %15 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %16 = AsInt32Inst (:number) %14: any
;; CHECK-NEXT:   %17 = AsInt32Inst (:number) %15: any
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:         ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function __wasm_instantiate__(imports: any): object
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:   %1 = TryLoadGlobalPropertyInst (:any) globalObject: object, "HermesInternal": string
;; CHECK-NEXT:   %2 = LoadPropertyInst (:any) %1: any, "intrinsics": string
;; CHECK-NEXT:        StoreFrameInst %0: environment, %2: any, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %imports: any
;; CHECK-NEXT:   %5 = LoadPropertyInst (:any) %4: any, "env": string
;; CHECK-NEXT:   %6 = BinaryStrictlyEqualInst (:any) %5: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %6: any, %BB1, %BB2
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %8 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace env": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT: %BB2:
;; CHECK-NEXT:   %10 = LoadPropertyInst (:any) %5: any, "log": string
;; CHECK-NEXT:   %11 = BinaryStrictlyEqualInst (:any) %10: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %11: any, %BB3, %BB4
;; CHECK-NEXT: %BB3:
;; CHECK-NEXT:   %13 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.log": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB4:
;; CHECK-NEXT:   %15 = CallBuiltinInst (:any) [HermesBuiltin.wasmFuncTypeId]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %10: any
;; CHECK-NEXT:   %16 = BinaryStrictlyEqualInst (:any) %15: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %16: any, %BB5, %BB6
;; CHECK-NEXT: %BB5:
;; CHECK-NEXT:   %18 = TypeOfInst (:string) %10: any
;; CHECK-NEXT:   %19 = BinaryStrictlyEqualInst (:any) %18: string, "function": string
;; CHECK-NEXT:         CondBranchInst %19: any, %BB7, %BB8
;; CHECK-NEXT: %BB6:
;; CHECK-NEXT:   %21 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:": string
;; CHECK-NEXT:   %22 = BinaryStrictlyNotEqualInst (:any) %15: any, %21: any
;; CHECK-NEXT:         CondBranchInst %22: any, %BB8, %BB9
;; CHECK-NEXT: %BB7:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %10: any, [%VS0.import_func_0]: any
;; CHECK-NEXT:   %25 = LoadPropertyInst (:any) %5: any, "add": string
;; CHECK-NEXT:   %26 = BinaryStrictlyEqualInst (:any) %25: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %26: any, %BB10, %BB11
;; CHECK-NEXT: %BB8:
;; CHECK-NEXT:   %28 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.log is not a function": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB9:
;; CHECK-NEXT:   %30 = TypeOfInst (:string) %10: any
;; CHECK-NEXT:   %31 = BinaryStrictlyEqualInst (:any) %30: string, "function": string
;; CHECK-NEXT:         CondBranchInst %31: any, %BB7, %BB8
;; CHECK-NEXT: %BB10:
;; CHECK-NEXT:   %33 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.add": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB11:
;; CHECK-NEXT:   %35 = CallBuiltinInst (:any) [HermesBuiltin.wasmFuncTypeId]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %25: any
;; CHECK-NEXT:   %36 = BinaryStrictlyEqualInst (:any) %35: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %36: any, %BB12, %BB13
;; CHECK-NEXT: %BB12:
;; CHECK-NEXT:   %38 = TypeOfInst (:string) %25: any
;; CHECK-NEXT:   %39 = BinaryStrictlyEqualInst (:any) %38: string, "function": string
;; CHECK-NEXT:         CondBranchInst %39: any, %BB14, %BB15
;; CHECK-NEXT: %BB13:
;; CHECK-NEXT:   %41 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:ii:i": string
;; CHECK-NEXT:   %42 = BinaryStrictlyNotEqualInst (:any) %35: any, %41: any
;; CHECK-NEXT:         CondBranchInst %42: any, %BB15, %BB16
;; CHECK-NEXT: %BB14:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %25: any, [%VS0.import_func_1]: any
;; CHECK-NEXT:   %45 = LoadPropertyInst (:any) %5: any, "init": string
;; CHECK-NEXT:   %46 = BinaryStrictlyEqualInst (:any) %45: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %46: any, %BB17, %BB18
;; CHECK-NEXT: %BB15:
;; CHECK-NEXT:   %48 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.add is not a function": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB16:
;; CHECK-NEXT:   %50 = TypeOfInst (:string) %25: any
;; CHECK-NEXT:   %51 = BinaryStrictlyEqualInst (:any) %50: string, "function": string
;; CHECK-NEXT:         CondBranchInst %51: any, %BB14, %BB15
;; CHECK-NEXT: %BB17:
;; CHECK-NEXT:   %53 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.init": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB18:
;; CHECK-NEXT:   %55 = CallBuiltinInst (:any) [HermesBuiltin.wasmFuncTypeId]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %45: any
;; CHECK-NEXT:   %56 = BinaryStrictlyEqualInst (:any) %55: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %56: any, %BB19, %BB20
;; CHECK-NEXT: %BB19:
;; CHECK-NEXT:   %58 = TypeOfInst (:string) %45: any
;; CHECK-NEXT:   %59 = BinaryStrictlyEqualInst (:any) %58: string, "function": string
;; CHECK-NEXT:         CondBranchInst %59: any, %BB21, %BB22
;; CHECK-NEXT: %BB20:
;; CHECK-NEXT:   %61 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::": string
;; CHECK-NEXT:   %62 = BinaryStrictlyNotEqualInst (:any) %55: any, %61: any
;; CHECK-NEXT:         CondBranchInst %62: any, %BB22, %BB23
;; CHECK-NEXT: %BB21:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %45: any, [%VS0.import_func_2]: any
;; CHECK-NEXT:   %65 = LoadPropertyInst (:any) %5: any, "f64_add": string
;; CHECK-NEXT:   %66 = BinaryStrictlyEqualInst (:any) %65: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %66: any, %BB24, %BB25
;; CHECK-NEXT: %BB22:
;; CHECK-NEXT:   %68 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.init is not a function": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB23:
;; CHECK-NEXT:   %70 = TypeOfInst (:string) %45: any
;; CHECK-NEXT:   %71 = BinaryStrictlyEqualInst (:any) %70: string, "function": string
;; CHECK-NEXT:         CondBranchInst %71: any, %BB21, %BB22
;; CHECK-NEXT: %BB24:
;; CHECK-NEXT:   %73 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.f64_add": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB25:
;; CHECK-NEXT:   %75 = CallBuiltinInst (:any) [HermesBuiltin.wasmFuncTypeId]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %65: any
;; CHECK-NEXT:   %76 = BinaryStrictlyEqualInst (:any) %75: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %76: any, %BB26, %BB27
;; CHECK-NEXT: %BB26:
;; CHECK-NEXT:   %78 = TypeOfInst (:string) %65: any
;; CHECK-NEXT:   %79 = BinaryStrictlyEqualInst (:any) %78: string, "function": string
;; CHECK-NEXT:         CondBranchInst %79: any, %BB28, %BB29
;; CHECK-NEXT: %BB27:
;; CHECK-NEXT:   %81 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:dd:d": string
;; CHECK-NEXT:   %82 = BinaryStrictlyNotEqualInst (:any) %75: any, %81: any
;; CHECK-NEXT:         CondBranchInst %82: any, %BB29, %BB30
;; CHECK-NEXT: %BB28:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %65: any, [%VS0.import_func_3]: any
;; CHECK-NEXT:   %85 = LoadPropertyInst (:any) %5: any, "i64_id": string
;; CHECK-NEXT:   %86 = BinaryStrictlyEqualInst (:any) %85: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %86: any, %BB31, %BB32
;; CHECK-NEXT: %BB29:
;; CHECK-NEXT:   %88 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.f64_add is not a function": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB30:
;; CHECK-NEXT:   %90 = TypeOfInst (:string) %65: any
;; CHECK-NEXT:   %91 = BinaryStrictlyEqualInst (:any) %90: string, "function": string
;; CHECK-NEXT:         CondBranchInst %91: any, %BB28, %BB29
;; CHECK-NEXT: %BB31:
;; CHECK-NEXT:   %93 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.i64_id": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB32:
;; CHECK-NEXT:   %95 = CallBuiltinInst (:any) [HermesBuiltin.wasmFuncTypeId]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %85: any
;; CHECK-NEXT:   %96 = BinaryStrictlyEqualInst (:any) %95: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %96: any, %BB33, %BB34
;; CHECK-NEXT: %BB33:
;; CHECK-NEXT:   %98 = TypeOfInst (:string) %85: any
;; CHECK-NEXT:   %99 = BinaryStrictlyEqualInst (:any) %98: string, "function": string
;; CHECK-NEXT:          CondBranchInst %99: any, %BB35, %BB36
;; CHECK-NEXT: %BB34:
;; CHECK-NEXT:   %101 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:l:l": string
;; CHECK-NEXT:   %102 = BinaryStrictlyNotEqualInst (:any) %95: any, %101: any
;; CHECK-NEXT:          CondBranchInst %102: any, %BB36, %BB37
;; CHECK-NEXT: %BB35:
;; CHECK-NEXT:          StoreFrameInst %0: environment, %85: any, [%VS0.import_func_4]: any
;; CHECK-NEXT:   %105 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_0(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %105: object, [%VS0.closure_0]: any
;; CHECK-NEXT:   %107 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_1(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %107: object, [%VS0.closure_1]: any
;; CHECK-NEXT:   %109 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_2(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %109: object, [%VS0.closure_2]: any
;; CHECK-NEXT:   %111 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_3(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %111: object, [%VS0.closure_3]: any
;; CHECK-NEXT:   %113 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_4(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %113: object, [%VS0.closure_4]: any
;; CHECK-NEXT:   %115 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_5(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %115: object, [%VS0.closure_5]: any
;; CHECK-NEXT:   %117 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %118 = LoadPropertyInst (:any) %117: any, "ArrayBuffer": string
;; CHECK-NEXT:   %119 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %120 = LoadPropertyInst (:any) %119: any, "Uint32Array": string
;; CHECK-NEXT:   %121 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %122 = LoadPropertyInst (:any) %121: any, "Float64Array": string
;; CHECK-NEXT:   %123 = CreateThisInst (:any) %118: any, %118: any, empty: any
;; CHECK-NEXT:   %124 = CallInst (:any) %118: any, empty: any, false: boolean, empty: any, %118: any, %123: any, 8: number
;; CHECK-NEXT:   %125 = GetConstructedObjectInst (:object) %123: any, %124: any
;; CHECK-NEXT:   %126 = CreateThisInst (:any) %120: any, %120: any, empty: any
;; CHECK-NEXT:   %127 = CallInst (:any) %120: any, empty: any, false: boolean, empty: any, %120: any, %126: any, %125: object
;; CHECK-NEXT:   %128 = GetConstructedObjectInst (:object) %126: any, %127: any
;; CHECK-NEXT:   %129 = CreateThisInst (:any) %122: any, %122: any, empty: any
;; CHECK-NEXT:   %130 = CallInst (:any) %122: any, empty: any, false: boolean, empty: any, %122: any, %129: any, %125: object
;; CHECK-NEXT:   %131 = GetConstructedObjectInst (:object) %129: any, %130: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %128: object, [%VS0.retBufI]: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %131: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %134 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %134: any, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %136 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:ii:i": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %136: any, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:   %138 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %138: any, [%VS0.wasm_type_id_2]: any
;; CHECK-NEXT:   %140 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:dd:d": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %140: any, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:   %142 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:l:l": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %142: any, [%VS0.wasm_type_id_4]: any
;; CHECK-NEXT:   %144 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_0(): functionCode
;; CHECK-NEXT:   %145 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %146 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %147 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %144: object, %145: any, %146: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %144: object, [%VS0.exported_func_0]: any
;; CHECK-NEXT:   %149 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_1(): functionCode
;; CHECK-NEXT:   %150 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %151 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:   %152 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %149: object, %150: any, %151: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %149: object, [%VS0.exported_func_1]: any
;; CHECK-NEXT:   %154 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_2(): functionCode
;; CHECK-NEXT:   %155 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:   %156 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_2]: any
;; CHECK-NEXT:   %157 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %154: object, %155: any, %156: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %154: object, [%VS0.exported_func_2]: any
;; CHECK-NEXT:   %159 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_3(): functionCode
;; CHECK-NEXT:   %160 = LoadFrameInst (:any) %0: environment, [%VS0.closure_3]: any
;; CHECK-NEXT:   %161 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:   %162 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %159: object, %160: any, %161: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %159: object, [%VS0.exported_func_3]: any
;; CHECK-NEXT:   %164 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_4(): functionCode
;; CHECK-NEXT:   %165 = LoadFrameInst (:any) %0: environment, [%VS0.closure_4]: any
;; CHECK-NEXT:   %166 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_4]: any
;; CHECK-NEXT:   %167 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %164: object, %165: any, %166: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %164: object, [%VS0.exported_func_4]: any
;; CHECK-NEXT:   %169 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_test(): functionCode
;; CHECK-NEXT:   %170 = LoadFrameInst (:any) %0: environment, [%VS0.closure_5]: any
;; CHECK-NEXT:   %171 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_2]: any
;; CHECK-NEXT:   %172 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %169: object, %170: any, %171: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %169: object, [%VS0.exported_func_5]: any
;; CHECK-NEXT:   %174 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:   %175 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_5]: any
;; CHECK-NEXT:          StorePropertyStrictInst %175: any, %174: object, "test": string
;; CHECK-NEXT:          ReturnInst %174: object
;; CHECK-NEXT: %BB36:
;; CHECK-NEXT:   %178 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.i64_id is not a function": string
;; CHECK-NEXT:          UnreachableInst
;; CHECK-NEXT: %BB37:
;; CHECK-NEXT:   %180 = TypeOfInst (:string) %85: any
;; CHECK-NEXT:   %181 = BinaryStrictlyEqualInst (:any) %180: string, "function": string
;; CHECK-NEXT:          CondBranchInst %181: any, %BB35, %BB36
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_funcref_0(p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:   %3 = AsInt32Inst (:number) %2: any
;; CHECK-NEXT:   %4 = CallInst (:any) %1: any, %wasm_func_0(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %3: number
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_funcref_1(p0: any, p1: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:   %3 = AsInt32Inst (:number) %2: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:   %5 = AsInt32Inst (:number) %4: any
;; CHECK-NEXT:   %6 = CallInst (:any) %1: any, %wasm_func_1(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %3: number, %5: number
;; CHECK-NEXT:        ReturnInst %6: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_funcref_2(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:   %2 = CallInst (:any) %1: any, %wasm_func_2(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_funcref_3(p0: any, p1: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_3]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:   %3 = AsNumberInst (:number) %2: any
;; CHECK-NEXT:   %4 = LoadParamInst (:any) %p1: any
;; CHECK-NEXT:   %5 = AsNumberInst (:number) %4: any
;; CHECK-NEXT:   %6 = CallInst (:any) %1: any, %wasm_func_3(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %3: number, %5: number
;; CHECK-NEXT:        ReturnInst %6: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_funcref_4(p0: any): any
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
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_export_test(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_5]: any
;; CHECK-NEXT:   %2 = CallInst (:any) %1: any, %wasm_func_5(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
