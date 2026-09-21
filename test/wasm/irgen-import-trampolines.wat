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

;; CHECK: scope %VS0 [wasm_type_id_0: any, wasm_type_id_1: any, wasm_type_id_2: any, wasm_type_id_3: any, wasm_type_id_4: any, import_func_0: any, import_func_1: any, import_func_2: any, import_func_3: any, import_func_4: any, retBufI: any, retBufF: any, closure_0: any, exported_func_0: any, closure_1: any, exported_func_1: any, closure_2: any, exported_func_2: any, closure_3: any, exported_func_3: any, closure_4: any, exported_func_4: any, closure_5: any, exported_func_5: any]
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
;; CHECK-NEXT:   %1 = LoadParamInst (:any) %imports: any
;; CHECK-NEXT:   %2 = LoadPropertyInst (:any) %1: any, "env": string
;; CHECK-NEXT:   %3 = BinaryStrictlyEqualInst (:any) %2: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %3: any, %BB1, %BB2
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %5 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace env": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT: %BB2:
;; CHECK-NEXT:   %7 = LoadPropertyInst (:any) %2: any, "log": string
;; CHECK-NEXT:   %8 = BinaryStrictlyEqualInst (:any) %7: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %8: any, %BB3, %BB4
;; CHECK-NEXT: %BB3:
;; CHECK-NEXT:   %10 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.log": string
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
;; CHECK-NEXT:         CondBranchInst %18: any, %BB8, %BB9
;; CHECK-NEXT: %BB7:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %7: any, [%VS0.import_func_0]: any
;; CHECK-NEXT:   %21 = LoadPropertyInst (:any) %2: any, "add": string
;; CHECK-NEXT:   %22 = BinaryStrictlyEqualInst (:any) %21: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %22: any, %BB10, %BB11
;; CHECK-NEXT: %BB8:
;; CHECK-NEXT:   %24 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.log is not a function": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB9:
;; CHECK-NEXT:   %26 = TypeOfInst (:string) %7: any
;; CHECK-NEXT:   %27 = BinaryStrictlyEqualInst (:any) %26: string, "function": string
;; CHECK-NEXT:         CondBranchInst %27: any, %BB7, %BB8
;; CHECK-NEXT: %BB10:
;; CHECK-NEXT:   %29 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.add": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB11:
;; CHECK-NEXT:   %31 = LoadPropertyInst (:any) %21: any, "__wasm_type__": string
;; CHECK-NEXT:   %32 = BinaryStrictlyEqualInst (:any) %31: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %32: any, %BB12, %BB13
;; CHECK-NEXT: %BB12:
;; CHECK-NEXT:   %34 = TypeOfInst (:string) %21: any
;; CHECK-NEXT:   %35 = BinaryStrictlyEqualInst (:any) %34: string, "function": string
;; CHECK-NEXT:         CondBranchInst %35: any, %BB14, %BB15
;; CHECK-NEXT: %BB13:
;; CHECK-NEXT:   %37 = BinaryStrictlyNotEqualInst (:any) %31: any, "func:ii:i": string
;; CHECK-NEXT:         CondBranchInst %37: any, %BB15, %BB16
;; CHECK-NEXT: %BB14:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %21: any, [%VS0.import_func_1]: any
;; CHECK-NEXT:   %40 = LoadPropertyInst (:any) %2: any, "init": string
;; CHECK-NEXT:   %41 = BinaryStrictlyEqualInst (:any) %40: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %41: any, %BB17, %BB18
;; CHECK-NEXT: %BB15:
;; CHECK-NEXT:   %43 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.add is not a function": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB16:
;; CHECK-NEXT:   %45 = TypeOfInst (:string) %21: any
;; CHECK-NEXT:   %46 = BinaryStrictlyEqualInst (:any) %45: string, "function": string
;; CHECK-NEXT:         CondBranchInst %46: any, %BB14, %BB15
;; CHECK-NEXT: %BB17:
;; CHECK-NEXT:   %48 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.init": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB18:
;; CHECK-NEXT:   %50 = LoadPropertyInst (:any) %40: any, "__wasm_type__": string
;; CHECK-NEXT:   %51 = BinaryStrictlyEqualInst (:any) %50: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %51: any, %BB19, %BB20
;; CHECK-NEXT: %BB19:
;; CHECK-NEXT:   %53 = TypeOfInst (:string) %40: any
;; CHECK-NEXT:   %54 = BinaryStrictlyEqualInst (:any) %53: string, "function": string
;; CHECK-NEXT:         CondBranchInst %54: any, %BB21, %BB22
;; CHECK-NEXT: %BB20:
;; CHECK-NEXT:   %56 = BinaryStrictlyNotEqualInst (:any) %50: any, "func::": string
;; CHECK-NEXT:         CondBranchInst %56: any, %BB22, %BB23
;; CHECK-NEXT: %BB21:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %40: any, [%VS0.import_func_2]: any
;; CHECK-NEXT:   %59 = LoadPropertyInst (:any) %2: any, "f64_add": string
;; CHECK-NEXT:   %60 = BinaryStrictlyEqualInst (:any) %59: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %60: any, %BB24, %BB25
;; CHECK-NEXT: %BB22:
;; CHECK-NEXT:   %62 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.init is not a function": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB23:
;; CHECK-NEXT:   %64 = TypeOfInst (:string) %40: any
;; CHECK-NEXT:   %65 = BinaryStrictlyEqualInst (:any) %64: string, "function": string
;; CHECK-NEXT:         CondBranchInst %65: any, %BB21, %BB22
;; CHECK-NEXT: %BB24:
;; CHECK-NEXT:   %67 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.f64_add": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB25:
;; CHECK-NEXT:   %69 = LoadPropertyInst (:any) %59: any, "__wasm_type__": string
;; CHECK-NEXT:   %70 = BinaryStrictlyEqualInst (:any) %69: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %70: any, %BB26, %BB27
;; CHECK-NEXT: %BB26:
;; CHECK-NEXT:   %72 = TypeOfInst (:string) %59: any
;; CHECK-NEXT:   %73 = BinaryStrictlyEqualInst (:any) %72: string, "function": string
;; CHECK-NEXT:         CondBranchInst %73: any, %BB28, %BB29
;; CHECK-NEXT: %BB27:
;; CHECK-NEXT:   %75 = BinaryStrictlyNotEqualInst (:any) %69: any, "func:dd:d": string
;; CHECK-NEXT:         CondBranchInst %75: any, %BB29, %BB30
;; CHECK-NEXT: %BB28:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %59: any, [%VS0.import_func_3]: any
;; CHECK-NEXT:   %78 = LoadPropertyInst (:any) %2: any, "i64_id": string
;; CHECK-NEXT:   %79 = BinaryStrictlyEqualInst (:any) %78: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %79: any, %BB31, %BB32
;; CHECK-NEXT: %BB29:
;; CHECK-NEXT:   %81 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.f64_add is not a function": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB30:
;; CHECK-NEXT:   %83 = TypeOfInst (:string) %59: any
;; CHECK-NEXT:   %84 = BinaryStrictlyEqualInst (:any) %83: string, "function": string
;; CHECK-NEXT:         CondBranchInst %84: any, %BB28, %BB29
;; CHECK-NEXT: %BB31:
;; CHECK-NEXT:   %86 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.i64_id": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB32:
;; CHECK-NEXT:   %88 = LoadPropertyInst (:any) %78: any, "__wasm_type__": string
;; CHECK-NEXT:   %89 = BinaryStrictlyEqualInst (:any) %88: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %89: any, %BB33, %BB34
;; CHECK-NEXT: %BB33:
;; CHECK-NEXT:   %91 = TypeOfInst (:string) %78: any
;; CHECK-NEXT:   %92 = BinaryStrictlyEqualInst (:any) %91: string, "function": string
;; CHECK-NEXT:         CondBranchInst %92: any, %BB35, %BB36
;; CHECK-NEXT: %BB34:
;; CHECK-NEXT:   %94 = BinaryStrictlyNotEqualInst (:any) %88: any, "func:l:l": string
;; CHECK-NEXT:         CondBranchInst %94: any, %BB36, %BB37
;; CHECK-NEXT: %BB35:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %78: any, [%VS0.import_func_4]: any
;; CHECK-NEXT:   %97 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_0(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %97: object, [%VS0.closure_0]: any
;; CHECK-NEXT:   %99 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_1(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %99: object, [%VS0.closure_1]: any
;; CHECK-NEXT:   %101 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_2(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %101: object, [%VS0.closure_2]: any
;; CHECK-NEXT:   %103 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_3(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %103: object, [%VS0.closure_3]: any
;; CHECK-NEXT:   %105 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_4(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %105: object, [%VS0.closure_4]: any
;; CHECK-NEXT:   %107 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_5(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %107: object, [%VS0.closure_5]: any
;; CHECK-NEXT:   %109 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:   %110 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:   %111 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:   %112 = CreateThisInst (:any) %109: any, %109: any, empty: any
;; CHECK-NEXT:   %113 = CallInst (:any) %109: any, empty: any, false: boolean, empty: any, %109: any, %112: any, 8: number
;; CHECK-NEXT:   %114 = GetConstructedObjectInst (:object) %112: any, %113: any
;; CHECK-NEXT:   %115 = CreateThisInst (:any) %110: any, %110: any, empty: any
;; CHECK-NEXT:   %116 = CallInst (:any) %110: any, empty: any, false: boolean, empty: any, %110: any, %115: any, %114: object
;; CHECK-NEXT:   %117 = GetConstructedObjectInst (:object) %115: any, %116: any
;; CHECK-NEXT:   %118 = CreateThisInst (:any) %111: any, %111: any, empty: any
;; CHECK-NEXT:   %119 = CallInst (:any) %111: any, empty: any, false: boolean, empty: any, %111: any, %118: any, %114: object
;; CHECK-NEXT:   %120 = GetConstructedObjectInst (:object) %118: any, %119: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %117: object, [%VS0.retBufI]: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %120: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %123 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %123: any, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %125 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:ii:i": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %125: any, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:   %127 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %127: any, [%VS0.wasm_type_id_2]: any
;; CHECK-NEXT:   %129 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:dd:d": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %129: any, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:   %131 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:l:l": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %131: any, [%VS0.wasm_type_id_4]: any
;; CHECK-NEXT:   %133 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_0(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func:i:": string, %133: object, "__wasm_type__": string
;; CHECK-NEXT:   %135 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %136 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %137 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %133: object, %135: any, %136: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %133: object, [%VS0.exported_func_0]: any
;; CHECK-NEXT:   %139 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_1(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func:ii:i": string, %139: object, "__wasm_type__": string
;; CHECK-NEXT:   %141 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %142 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:   %143 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %139: object, %141: any, %142: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %139: object, [%VS0.exported_func_1]: any
;; CHECK-NEXT:   %145 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_2(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func::": string, %145: object, "__wasm_type__": string
;; CHECK-NEXT:   %147 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:   %148 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_2]: any
;; CHECK-NEXT:   %149 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %145: object, %147: any, %148: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %145: object, [%VS0.exported_func_2]: any
;; CHECK-NEXT:   %151 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_3(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func:dd:d": string, %151: object, "__wasm_type__": string
;; CHECK-NEXT:   %153 = LoadFrameInst (:any) %0: environment, [%VS0.closure_3]: any
;; CHECK-NEXT:   %154 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:   %155 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %151: object, %153: any, %154: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %151: object, [%VS0.exported_func_3]: any
;; CHECK-NEXT:   %157 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_4(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func:l:l": string, %157: object, "__wasm_type__": string
;; CHECK-NEXT:   %159 = LoadFrameInst (:any) %0: environment, [%VS0.closure_4]: any
;; CHECK-NEXT:   %160 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_4]: any
;; CHECK-NEXT:   %161 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %157: object, %159: any, %160: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %157: object, [%VS0.exported_func_4]: any
;; CHECK-NEXT:   %163 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_test(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func::": string, %163: object, "__wasm_type__": string
;; CHECK-NEXT:   %165 = LoadFrameInst (:any) %0: environment, [%VS0.closure_5]: any
;; CHECK-NEXT:   %166 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_2]: any
;; CHECK-NEXT:   %167 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %163: object, %165: any, %166: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %163: object, [%VS0.exported_func_5]: any
;; CHECK-NEXT:   %169 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:   %170 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_5]: any
;; CHECK-NEXT:          StorePropertyStrictInst %170: any, %169: object, "test": string
;; CHECK-NEXT:          ReturnInst %169: object
;; CHECK-NEXT: %BB36:
;; CHECK-NEXT:   %173 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.i64_id is not a function": string
;; CHECK-NEXT:          UnreachableInst
;; CHECK-NEXT: %BB37:
;; CHECK-NEXT:   %175 = TypeOfInst (:string) %78: any
;; CHECK-NEXT:   %176 = BinaryStrictlyEqualInst (:any) %175: string, "function": string
;; CHECK-NEXT:          CondBranchInst %176: any, %BB35, %BB36
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
