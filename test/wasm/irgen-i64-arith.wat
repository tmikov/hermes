;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for i64 arithmetic operations (G.3).
;; i64 values are represented as two i32 stack slots [lo, hi].
;; Binary ops use CallBuiltinInst + HiResult pattern; and/or/xor are inline.
;; NOTE: Uses only i64 constants, not i64 params (G.5 needed for i64 locals).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; i64.add uses CallBuiltinInst
  (func $add (result i32)
    i64.const 100
    i64.const 200
    i64.add
    i64.eqz)


  ;; i64.and uses inline BinaryAndInst on both lo and hi
  (func $and (result i32)
    i64.const 0xFF00
    i64.const 0x0FFF
    i64.and
    i64.eqz)


  ;; i64.or uses inline BinaryOrInst on both lo and hi
  (func $or (result i32)
    i64.const 0xFF00
    i64.const 0x00FF
    i64.or
    i64.eqz)


  ;; i64.xor uses inline BinaryXorInst on both lo and hi
  (func $xor (result i32)
    i64.const 0xFF
    i64.const 0x0F
    i64.xor
    i64.eqz)


  ;; i64.shl uses CallBuiltinInst
  (func $shl (result i32)
    i64.const 1
    i64.const 32
    i64.shl
    i64.eqz)


  ;; i64.clz returns i64 (but result is always in [0,64])
  (func $clz (result i32)
    i64.const 1
    i64.clz
    i64.eqz)


  ;; i64.eq returns i32 (not i64)
  (func $eq (result i32)
    i64.const 42
    i64.const 42
    i64.eq)


  ;; i64.eqz returns i32
  (func $eqz (result i32)
    i64.const 0
    i64.eqz)


  ;; i64.sub
  (func $sub (result i32)
    i64.const 500
    i64.const 200
    i64.sub
    i64.const 300
    i64.eq)


  ;; i64.mul
  (func $mul (result i32)
    i64.const 6
    i64.const 7
    i64.mul
    i64.const 42
    i64.eq)

)

;; CHECK: scope %VS0 [wasm_type_id_0: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any, closure_2: any, closure_3: any, closure_4: any, closure_5: any, closure_6: any, closure_7: any, closure_8: any, closure_9: any]
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
;; CHECK-NEXT:   %2 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Add]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, 100: number, 0: number, 200: number, 0: number
;; CHECK-NEXT:   %3 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %4 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %5 = AsInt32Inst (:number) %3: any
;; CHECK-NEXT:   %6 = AsInt32Inst (:number) %4: any
;; CHECK-NEXT:   %7 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Eqz]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %5: number, %6: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %9 = PhiInst (:any) %7: any, %BB0
;; CHECK-NEXT:         ReturnInst %9: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_1(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = BinaryAndInst (:any) 65280: number, 4095: number
;; CHECK-NEXT:   %3 = BinaryAndInst (:any) 0: number, 0: number
;; CHECK-NEXT:   %4 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Eqz]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: any, %3: any
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %6 = PhiInst (:any) %4: any, %BB0
;; CHECK-NEXT:        ReturnInst %6: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_2(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = BinaryOrInst (:any) 65280: number, 255: number
;; CHECK-NEXT:   %3 = BinaryOrInst (:any) 0: number, 0: number
;; CHECK-NEXT:   %4 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Eqz]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: any, %3: any
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %6 = PhiInst (:any) %4: any, %BB0
;; CHECK-NEXT:        ReturnInst %6: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_3(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = BinaryXorInst (:any) 255: number, 15: number
;; CHECK-NEXT:   %3 = BinaryXorInst (:any) 0: number, 0: number
;; CHECK-NEXT:   %4 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Eqz]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: any, %3: any
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %6 = PhiInst (:any) %4: any, %BB0
;; CHECK-NEXT:        ReturnInst %6: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_4(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Shl]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, 1: number, 0: number, 32: number, 0: number
;; CHECK-NEXT:   %3 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %4 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %5 = AsInt32Inst (:number) %3: any
;; CHECK-NEXT:   %6 = AsInt32Inst (:number) %4: any
;; CHECK-NEXT:   %7 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Eqz]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %5: number, %6: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %9 = PhiInst (:any) %7: any, %BB0
;; CHECK-NEXT:         ReturnInst %9: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_5(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Clz]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, 1: number, 0: number
;; CHECK-NEXT:   %3 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Eqz]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: any, 0: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %5 = PhiInst (:any) %3: any, %BB0
;; CHECK-NEXT:        ReturnInst %5: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_6(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Eq]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, 42: number, 0: number, 42: number, 0: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %4 = PhiInst (:any) %2: any, %BB0
;; CHECK-NEXT:        ReturnInst %4: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_7(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Eqz]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, 0: number, 0: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %4 = PhiInst (:any) %2: any, %BB0
;; CHECK-NEXT:        ReturnInst %4: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_8(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Sub]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, 500: number, 0: number, 200: number, 0: number
;; CHECK-NEXT:   %3 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %4 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %5 = AsInt32Inst (:number) %3: any
;; CHECK-NEXT:   %6 = AsInt32Inst (:number) %4: any
;; CHECK-NEXT:   %7 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Eq]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %5: number, %6: number, 300: number, 0: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %9 = PhiInst (:any) %7: any, %BB0
;; CHECK-NEXT:         ReturnInst %9: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_9(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Mul]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, 6: number, 0: number, 7: number, 0: number
;; CHECK-NEXT:   %3 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %4 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %5 = AsInt32Inst (:number) %3: any
;; CHECK-NEXT:   %6 = AsInt32Inst (:number) %4: any
;; CHECK-NEXT:   %7 = CallBuiltinInst (:any) [HermesBuiltin.wasmI64Eq]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %5: number, %6: number, 42: number, 0: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %9 = PhiInst (:any) %7: any, %BB0
;; CHECK-NEXT:         ReturnInst %9: any
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
;; CHECK-NEXT:   %17 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_8(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %17: object, [%VS0.closure_8]: any
;; CHECK-NEXT:   %19 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_9(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %19: object, [%VS0.closure_9]: any
;; CHECK-NEXT:   %21 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:   %22 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:   %23 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:   %24 = CreateThisInst (:any) %21: any, %21: any, empty: any
;; CHECK-NEXT:   %25 = CallInst (:any) %21: any, empty: any, false: boolean, empty: any, %21: any, %24: any, 8: number
;; CHECK-NEXT:   %26 = GetConstructedObjectInst (:object) %24: any, %25: any
;; CHECK-NEXT:   %27 = CreateThisInst (:any) %22: any, %22: any, empty: any
;; CHECK-NEXT:   %28 = CallInst (:any) %22: any, empty: any, false: boolean, empty: any, %22: any, %27: any, %26: object
;; CHECK-NEXT:   %29 = GetConstructedObjectInst (:object) %27: any, %28: any
;; CHECK-NEXT:   %30 = CreateThisInst (:any) %23: any, %23: any, empty: any
;; CHECK-NEXT:   %31 = CallInst (:any) %23: any, empty: any, false: boolean, empty: any, %23: any, %30: any, %26: object
;; CHECK-NEXT:   %32 = GetConstructedObjectInst (:object) %30: any, %31: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %29: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %32: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %35 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         ReturnInst %35: object
;; CHECK-NEXT: function_end
