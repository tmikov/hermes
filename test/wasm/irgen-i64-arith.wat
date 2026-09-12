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

;; CHECK: scope %VS0 [wasm_type_id_0: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any, closure_2: any, closure_3: any, closure_4: any, closure_5: any, closure_6: any, closure_7: any, closure_8: any, closure_9: any, intrinsics: any]
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
;; CHECK-NEXT: function wasm_func_0(): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:number) [HermesBuiltin.wasmI64Add]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, 100: number, 0: number, 200: number, 0: number
;; CHECK-NEXT:   %3 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %4 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %5 = AsInt32Inst (:number) %3: any
;; CHECK-NEXT:   %6 = AsInt32Inst (:number) %4: any
;; CHECK-NEXT:   %7 = BinaryOrInst (:number) %5: number, %6: number
;; CHECK-NEXT:   %8 = FEqualInst (:boolean) %7: number, 0: number
;; CHECK-NEXT:   %9 = AsInt32Inst (:number) %8: boolean
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %11 = PhiInst (:number) %9: number, %BB0
;; CHECK-NEXT:         ReturnInst %11: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_1(): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = BinaryAndInst (:number) 65280: number, 4095: number
;; CHECK-NEXT:   %3 = BinaryAndInst (:number) 0: number, 0: number
;; CHECK-NEXT:   %4 = BinaryOrInst (:number) %2: number, %3: number
;; CHECK-NEXT:   %5 = FEqualInst (:boolean) %4: number, 0: number
;; CHECK-NEXT:   %6 = AsInt32Inst (:number) %5: boolean
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %8 = PhiInst (:number) %6: number, %BB0
;; CHECK-NEXT:        ReturnInst %8: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_2(): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = BinaryOrInst (:number) 65280: number, 255: number
;; CHECK-NEXT:   %3 = BinaryOrInst (:number) 0: number, 0: number
;; CHECK-NEXT:   %4 = BinaryOrInst (:number) %2: number, %3: number
;; CHECK-NEXT:   %5 = FEqualInst (:boolean) %4: number, 0: number
;; CHECK-NEXT:   %6 = AsInt32Inst (:number) %5: boolean
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %8 = PhiInst (:number) %6: number, %BB0
;; CHECK-NEXT:        ReturnInst %8: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_3(): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = BinaryXorInst (:number) 255: number, 15: number
;; CHECK-NEXT:   %3 = BinaryXorInst (:number) 0: number, 0: number
;; CHECK-NEXT:   %4 = BinaryOrInst (:number) %2: number, %3: number
;; CHECK-NEXT:   %5 = FEqualInst (:boolean) %4: number, 0: number
;; CHECK-NEXT:   %6 = AsInt32Inst (:number) %5: boolean
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %8 = PhiInst (:number) %6: number, %BB0
;; CHECK-NEXT:        ReturnInst %8: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_4(): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:number) [HermesBuiltin.wasmI64Shl]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, 1: number, 0: number, 32: number, 0: number
;; CHECK-NEXT:   %3 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %4 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %5 = AsInt32Inst (:number) %3: any
;; CHECK-NEXT:   %6 = AsInt32Inst (:number) %4: any
;; CHECK-NEXT:   %7 = BinaryOrInst (:number) %5: number, %6: number
;; CHECK-NEXT:   %8 = FEqualInst (:boolean) %7: number, 0: number
;; CHECK-NEXT:   %9 = AsInt32Inst (:number) %8: boolean
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %11 = PhiInst (:number) %9: number, %BB0
;; CHECK-NEXT:         ReturnInst %11: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_5(): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:number) [HermesBuiltin.wasmI64Clz]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, 1: number, 0: number
;; CHECK-NEXT:   %3 = AsInt32Inst (:number) %2: number
;; CHECK-NEXT:   %4 = AsInt32Inst (:number) 0: number
;; CHECK-NEXT:   %5 = BinaryOrInst (:number) %3: number, %4: number
;; CHECK-NEXT:   %6 = FEqualInst (:boolean) %5: number, 0: number
;; CHECK-NEXT:   %7 = AsInt32Inst (:number) %6: boolean
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %9 = PhiInst (:number) %7: number, %BB0
;; CHECK-NEXT:         ReturnInst %9: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_6(): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = AsInt32Inst (:number) 42: number
;; CHECK-NEXT:   %3 = AsInt32Inst (:number) 42: number
;; CHECK-NEXT:   %4 = AsInt32Inst (:number) 0: number
;; CHECK-NEXT:   %5 = AsInt32Inst (:number) 0: number
;; CHECK-NEXT:   %6 = BinaryXorInst (:number) %2: number, %3: number
;; CHECK-NEXT:   %7 = BinaryXorInst (:number) %4: number, %5: number
;; CHECK-NEXT:   %8 = BinaryOrInst (:number) %6: number, %7: number
;; CHECK-NEXT:   %9 = FEqualInst (:boolean) %8: number, 0: number
;; CHECK-NEXT:   %10 = AsInt32Inst (:number) %9: boolean
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %12 = PhiInst (:number) %10: number, %BB0
;; CHECK-NEXT:         ReturnInst %12: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_7(): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = AsInt32Inst (:number) 0: number
;; CHECK-NEXT:   %3 = AsInt32Inst (:number) 0: number
;; CHECK-NEXT:   %4 = BinaryOrInst (:number) %2: number, %3: number
;; CHECK-NEXT:   %5 = FEqualInst (:boolean) %4: number, 0: number
;; CHECK-NEXT:   %6 = AsInt32Inst (:number) %5: boolean
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %8 = PhiInst (:number) %6: number, %BB0
;; CHECK-NEXT:        ReturnInst %8: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_8(): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:number) [HermesBuiltin.wasmI64Sub]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, 500: number, 0: number, 200: number, 0: number
;; CHECK-NEXT:   %3 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %4 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %5 = AsInt32Inst (:number) %3: any
;; CHECK-NEXT:   %6 = AsInt32Inst (:number) %4: any
;; CHECK-NEXT:   %7 = AsInt32Inst (:number) 300: number
;; CHECK-NEXT:   %8 = AsInt32Inst (:number) 0: number
;; CHECK-NEXT:   %9 = BinaryXorInst (:number) %5: number, %7: number
;; CHECK-NEXT:   %10 = BinaryXorInst (:number) %6: number, %8: number
;; CHECK-NEXT:   %11 = BinaryOrInst (:number) %9: number, %10: number
;; CHECK-NEXT:   %12 = FEqualInst (:boolean) %11: number, 0: number
;; CHECK-NEXT:   %13 = AsInt32Inst (:number) %12: boolean
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %15 = PhiInst (:number) %13: number, %BB0
;; CHECK-NEXT:         ReturnInst %15: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_9(): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = CallBuiltinInst (:number) [HermesBuiltin.wasmI64Mul]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %1: any, 6: number, 0: number, 7: number, 0: number
;; CHECK-NEXT:   %3 = LoadPropertyInst (:any) %1: any, 0: number
;; CHECK-NEXT:   %4 = LoadPropertyInst (:any) %1: any, 1: number
;; CHECK-NEXT:   %5 = AsInt32Inst (:number) %3: any
;; CHECK-NEXT:   %6 = AsInt32Inst (:number) %4: any
;; CHECK-NEXT:   %7 = AsInt32Inst (:number) 42: number
;; CHECK-NEXT:   %8 = AsInt32Inst (:number) 0: number
;; CHECK-NEXT:   %9 = BinaryXorInst (:number) %5: number, %7: number
;; CHECK-NEXT:   %10 = BinaryXorInst (:number) %6: number, %8: number
;; CHECK-NEXT:   %11 = BinaryOrInst (:number) %9: number, %10: number
;; CHECK-NEXT:   %12 = FEqualInst (:boolean) %11: number, 0: number
;; CHECK-NEXT:   %13 = AsInt32Inst (:number) %12: boolean
;; CHECK-NEXT:         BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %15 = PhiInst (:number) %13: number, %BB0
;; CHECK-NEXT:         ReturnInst %15: number
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
;; CHECK-NEXT:   %20 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_8(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %20: object, [%VS0.closure_8]: any
;; CHECK-NEXT:   %22 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_9(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %22: object, [%VS0.closure_9]: any
;; CHECK-NEXT:   %24 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %25 = LoadPropertyInst (:any) %24: any, "ArrayBuffer": string
;; CHECK-NEXT:   %26 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %27 = LoadPropertyInst (:any) %26: any, "Uint32Array": string
;; CHECK-NEXT:   %28 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %29 = LoadPropertyInst (:any) %28: any, "Float64Array": string
;; CHECK-NEXT:   %30 = CreateThisInst (:any) %25: any, %25: any, empty: any
;; CHECK-NEXT:   %31 = CallInst (:any) %25: any, empty: any, false: boolean, empty: any, %25: any, %30: any, 8: number
;; CHECK-NEXT:   %32 = GetConstructedObjectInst (:object) %30: any, %31: any
;; CHECK-NEXT:   %33 = CreateThisInst (:any) %27: any, %27: any, empty: any
;; CHECK-NEXT:   %34 = CallInst (:any) %27: any, empty: any, false: boolean, empty: any, %27: any, %33: any, %32: object
;; CHECK-NEXT:   %35 = GetConstructedObjectInst (:object) %33: any, %34: any
;; CHECK-NEXT:   %36 = CreateThisInst (:any) %29: any, %29: any, empty: any
;; CHECK-NEXT:   %37 = CallInst (:any) %29: any, empty: any, false: boolean, empty: any, %29: any, %36: any, %32: object
;; CHECK-NEXT:   %38 = GetConstructedObjectInst (:object) %36: any, %37: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %35: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %38: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %41 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         ReturnInst %41: object
;; CHECK-NEXT: function_end
