;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test module with mixed imports (functions, table, memory, global)
;; from different modules, and a start function.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheckOrRegen %s

(module
  ;; Import a function from "env".
  (import "env" "log" (func $log (param i32)))

  ;; Import a global from "config".
  (import "config" "max_size" (global $max_size i32))

  ;; Import a memory from "env".
  (import "env" "memory" (memory 1 10))

  ;; Import a function from a different module "math".
  (import "math" "square" (func $square (param i32) (result i32)))

  ;; Table declared in this module (not imported).
  (table 4 funcref)

  ;; The start function is the first defined function (func index 2).
  (func $init
    i32.const 0
    call $log
  )
  (start $init)
;; Import trampoline for $log (void return).

;; Import trampoline for $square (i32 return).

;; $init: calls $log(0).

  ;; Exported functions.
  (func (export "run") (result i32)
    global.get $max_size
  )
;; "run": global.get loads the imported global.

  (func (export "helper") (param i32) (result i32)
    local.get 0
    call $square
  )
;; "helper": loads param, calls imported $square, returns result.
)

;; Auto-generated content below. Please do not modify manually.

;; CHECK:scope %VS0 [HEAP8: any, HEAPU8: any, HEAP16: any, HEAPU16: any, HEAP32: any, HEAPU32: any, HEAPF32: any, HEAPF64: any, wasm_type_id_0: any, wasm_type_id_1: any, wasm_type_id_2: any, wasm_type_id_3: any, table_0_funcs: any, table_0_types: any, table_0_obj: any, global_0: any, import_func_0: any, import_func_1: any, import_global_val_0: any, imported_mem_max: any, mem_obj: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any, closure_2: any, closure_3: any, closure_4: any]

;; CHECK:function global(): object
;; CHECK-NEXT:%BB0:
;; CHECK-NEXT:  %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:  %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %__wasm_instantiate__(): functionCode
;; CHECK-NEXT:  %2 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK-NEXT:  %3 = CreateThisInst (:any) %2: any, %2: any, empty: any
;; CHECK-NEXT:  %4 = CallInst (:any) %2: any, empty: any, false: boolean, empty: any, %2: any, %3: any, 2: number
;; CHECK-NEXT:  %5 = GetConstructedObjectInst (:object) %3: any, %4: any
;; CHECK-NEXT:  %6 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:       StorePropertyStrictInst "run": string, %6: object, "name": string
;; CHECK-NEXT:       StorePropertyStrictInst "function": string, %6: object, "kind": string
;; CHECK-NEXT:       StorePropertyStrictInst %6: object, %5: object, 0: number
;; CHECK-NEXT:  %10 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:        StorePropertyStrictInst "helper": string, %10: object, "name": string
;; CHECK-NEXT:        StorePropertyStrictInst "function": string, %10: object, "kind": string
;; CHECK-NEXT:        StorePropertyStrictInst %10: object, %5: object, 1: number
;; CHECK-NEXT:  %14 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK-NEXT:  %15 = CreateThisInst (:any) %14: any, %14: any, empty: any
;; CHECK-NEXT:  %16 = CallInst (:any) %14: any, empty: any, false: boolean, empty: any, %14: any, %15: any, 4: number
;; CHECK-NEXT:  %17 = GetConstructedObjectInst (:object) %15: any, %16: any
;; CHECK-NEXT:  %18 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:        StorePropertyStrictInst "env": string, %18: object, "module": string
;; CHECK-NEXT:        StorePropertyStrictInst "log": string, %18: object, "name": string
;; CHECK-NEXT:        StorePropertyStrictInst "function": string, %18: object, "kind": string
;; CHECK-NEXT:        StorePropertyStrictInst %18: object, %17: object, 0: number
;; CHECK-NEXT:  %23 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:        StorePropertyStrictInst "config": string, %23: object, "module": string
;; CHECK-NEXT:        StorePropertyStrictInst "max_size": string, %23: object, "name": string
;; CHECK-NEXT:        StorePropertyStrictInst "global": string, %23: object, "kind": string
;; CHECK-NEXT:        StorePropertyStrictInst %23: object, %17: object, 1: number
;; CHECK-NEXT:  %28 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:        StorePropertyStrictInst "env": string, %28: object, "module": string
;; CHECK-NEXT:        StorePropertyStrictInst "memory": string, %28: object, "name": string
;; CHECK-NEXT:        StorePropertyStrictInst "memory": string, %28: object, "kind": string
;; CHECK-NEXT:        StorePropertyStrictInst %28: object, %17: object, 2: number
;; CHECK-NEXT:  %33 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:        StorePropertyStrictInst "math": string, %33: object, "module": string
;; CHECK-NEXT:        StorePropertyStrictInst "square": string, %33: object, "name": string
;; CHECK-NEXT:        StorePropertyStrictInst "function": string, %33: object, "kind": string
;; CHECK-NEXT:        StorePropertyStrictInst %33: object, %17: object, 3: number
;; CHECK-NEXT:  %38 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:        StorePropertyStrictInst %1: object, %38: object, "instantiate": string
;; CHECK-NEXT:        StorePropertyStrictInst %5: object, %38: object, "exportDescs": string
;; CHECK-NEXT:        StorePropertyStrictInst %17: object, %38: object, "importDescs": string
;; CHECK-NEXT:        ReturnInst %38: object
;; CHECK-NEXT:function_end

;; CHECK:function wasm_func_0(p0: number): undefined
;; CHECK-NEXT:%BB0:
;; CHECK-NEXT:  %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:  %1 = LoadFrameInst (:any) %0: environment, [%VS0.import_func_0]: any
;; CHECK-NEXT:  %2 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:  %3 = CallInst (:any) %1: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: number
;; CHECK-NEXT:       ReturnInst undefined: undefined
;; CHECK-NEXT:function_end

;; CHECK:function wasm_func_1(p0: number): number
;; CHECK-NEXT:%BB0:
;; CHECK-NEXT:  %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:  %1 = LoadFrameInst (:any) %0: environment, [%VS0.import_func_1]: any
;; CHECK-NEXT:  %2 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:  %3 = CallInst (:any) %1: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: number
;; CHECK-NEXT:  %4 = AsInt32Inst (:number) %3: any
;; CHECK-NEXT:       ReturnInst %4: number
;; CHECK-NEXT:function_end

;; CHECK:function wasm_func_2(): undefined
;; CHECK-NEXT:%BB0:
;; CHECK-NEXT:  %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:  %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:  %2 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:  %3 = CallInst (:undefined) %2: any, %wasm_func_0(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, 0: number
;; CHECK-NEXT:       BranchInst %BB1
;; CHECK-NEXT:%BB1:
;; CHECK-NEXT:       ReturnInst undefined: undefined
;; CHECK-NEXT:function_end

;; CHECK:function wasm_func_3(): number
;; CHECK-NEXT:%BB0:
;; CHECK-NEXT:  %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:  %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:  %2 = LoadFrameInst (:any) %0: environment, [%VS0.global_0]: any
;; CHECK-NEXT:       BranchInst %BB1
;; CHECK-NEXT:%BB1:
;; CHECK-NEXT:  %4 = PhiInst (:any) %2: any, %BB0
;; CHECK-NEXT:       ReturnInst %4: any
;; CHECK-NEXT:function_end

;; CHECK:function wasm_func_4(p0: number): number
;; CHECK-NEXT:%BB0:
;; CHECK-NEXT:  %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:  %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:  %2 = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:  %3 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:       StoreStackInst %3: number, %2: number
;; CHECK-NEXT:  %5 = LoadStackInst (:number) %2: number
;; CHECK-NEXT:  %6 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:  %7 = CallInst (:number) %6: any, %wasm_func_1(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %5: number
;; CHECK-NEXT:       BranchInst %BB1
;; CHECK-NEXT:%BB1:
;; CHECK-NEXT:  %9 = PhiInst (:number) %7: number, %BB0
;; CHECK-NEXT:        ReturnInst %9: number
;; CHECK-NEXT:function_end

;; CHECK:function __wasm_instantiate__(imports: any): object
;; CHECK-NEXT:%BB0:
;; CHECK-NEXT:  %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:  %1 = LoadParamInst (:any) %imports: any
;; CHECK-NEXT:  %2 = LoadPropertyInst (:any) %1: any, "env": string
;; CHECK-NEXT:  %3 = BinaryStrictlyEqualInst (:any) %2: any, undefined: undefined
;; CHECK-NEXT:       CondBranchInst %3: any, %BB1, %BB2
;; CHECK-NEXT:%BB1:
;; CHECK-NEXT:  %5 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace env": string
;; CHECK-NEXT:       UnreachableInst
;; CHECK-NEXT:%BB2:
;; CHECK-NEXT:  %7 = LoadPropertyInst (:any) %2: any, "log": string
;; CHECK-NEXT:  %8 = BinaryStrictlyEqualInst (:any) %7: any, undefined: undefined
;; CHECK-NEXT:       CondBranchInst %8: any, %BB3, %BB4
;; CHECK-NEXT:%BB3:
;; CHECK-NEXT:  %10 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.log": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB4:
;; CHECK-NEXT:  %12 = LoadPropertyInst (:any) %7: any, "__wasm_type__": string
;; CHECK-NEXT:  %13 = BinaryStrictlyEqualInst (:any) %12: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %13: any, %BB5, %BB6
;; CHECK-NEXT:%BB5:
;; CHECK-NEXT:  %15 = TypeOfInst (:string) %7: any
;; CHECK-NEXT:  %16 = BinaryStrictlyEqualInst (:any) %15: string, "function": string
;; CHECK-NEXT:        CondBranchInst %16: any, %BB7, %BB8
;; CHECK-NEXT:%BB6:
;; CHECK-NEXT:  %18 = BinaryStrictlyNotEqualInst (:any) %12: any, "func:i:": string
;; CHECK-NEXT:        CondBranchInst %18: any, %BB8, %BB9
;; CHECK-NEXT:%BB7:
;; CHECK-NEXT:        StoreFrameInst %0: environment, %7: any, [%VS0.import_func_0]: any
;; CHECK-NEXT:  %21 = LoadPropertyInst (:any) %1: any, "config": string
;; CHECK-NEXT:  %22 = BinaryStrictlyEqualInst (:any) %21: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %22: any, %BB10, %BB11
;; CHECK-NEXT:%BB8:
;; CHECK-NEXT:  %24 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.log is not a function": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB9:
;; CHECK-NEXT:  %26 = TypeOfInst (:string) %7: any
;; CHECK-NEXT:  %27 = BinaryStrictlyEqualInst (:any) %26: string, "function": string
;; CHECK-NEXT:        CondBranchInst %27: any, %BB7, %BB8
;; CHECK-NEXT:%BB10:
;; CHECK-NEXT:  %29 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace config": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB11:
;; CHECK-NEXT:  %31 = LoadPropertyInst (:any) %21: any, "max_size": string
;; CHECK-NEXT:  %32 = BinaryStrictlyEqualInst (:any) %31: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %32: any, %BB12, %BB13
;; CHECK-NEXT:%BB12:
;; CHECK-NEXT:  %34 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import config.max_size": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB13:
;; CHECK-NEXT:  %36 = LoadPropertyInst (:any) %31: any, "__wasm_type__": string
;; CHECK-NEXT:  %37 = BinaryStrictlyEqualInst (:any) %36: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %37: any, %BB18, %BB14
;; CHECK-NEXT:%BB14:
;; CHECK-NEXT:  %39 = BinaryStrictlyNotEqualInst (:any) %36: any, "global:i32:const": string
;; CHECK-NEXT:        CondBranchInst %39: any, %BB16, %BB19
;; CHECK-NEXT:%BB15:
;; CHECK-NEXT:  %41 = PhiInst (:any) %31: any, %BB18, %53: any, %BB19
;; CHECK-NEXT:        StoreFrameInst %0: environment, %41: any, [%VS0.import_global_val_0]: any
;; CHECK-NEXT:  %43 = LoadPropertyInst (:any) %1: any, "env": string
;; CHECK-NEXT:  %44 = BinaryStrictlyEqualInst (:any) %43: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %44: any, %BB20, %BB21
;; CHECK-NEXT:%BB16:
;; CHECK-NEXT:  %46 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import config.max_size is not a valid global import": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB17:
;; CHECK-NEXT:  %48 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import config.max_size must be a Number to satisfy an i32 global import": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB18:
;; CHECK-NEXT:  %50 = TypeOfInst (:string) %31: any
;; CHECK-NEXT:  %51 = BinaryStrictlyEqualInst (:any) %50: string, "number": string
;; CHECK-NEXT:        CondBranchInst %51: any, %BB15, %BB17
;; CHECK-NEXT:%BB19:
;; CHECK-NEXT:  %53 = LoadPropertyInst (:any) %31: any, "value": string
;; CHECK-NEXT:        BranchInst %BB15
;; CHECK-NEXT:%BB20:
;; CHECK-NEXT:  %55 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace env": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB21:
;; CHECK-NEXT:  %57 = LoadPropertyInst (:any) %43: any, "memory": string
;; CHECK-NEXT:  %58 = BinaryStrictlyEqualInst (:any) %57: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %58: any, %BB22, %BB23
;; CHECK-NEXT:%BB22:
;; CHECK-NEXT:  %60 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.memory": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB23:
;; CHECK-NEXT:  %62 = TryLoadGlobalPropertyInst (:any) globalObject: object, "WebAssembly": string
;; CHECK-NEXT:  %63 = LoadPropertyInst (:any) %62: any, "Memory": string
;; CHECK-NEXT:  %64 = BinaryInstanceOfInst (:any) %57: any, %63: any
;; CHECK-NEXT:        CondBranchInst %64: any, %BB27, %BB24
;; CHECK-NEXT:%BB24:
;; CHECK-NEXT:  %66 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.memory is not a WebAssembly.Memory": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB25:
;; CHECK-NEXT:  %68 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.memory does not satisfy the declared memory limits": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB26:
;; CHECK-NEXT:        StoreFrameInst %0: environment, %57: any, [%VS0.mem_obj]: any
;; CHECK-NEXT:        StoreFrameInst %0: environment, %78: number, [%VS0.imported_mem_max]: any
;; CHECK-NEXT:  %72 = LoadPropertyInst (:any) %1: any, "math": string
;; CHECK-NEXT:  %73 = BinaryStrictlyEqualInst (:any) %72: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %73: any, %BB30, %BB31
;; CHECK-NEXT:%BB27:
;; CHECK-NEXT:  %75 = LoadPropertyInst (:any) %57: any, "__wasm_min__": string
;; CHECK-NEXT:  %76 = AsNumberInst (:number) %75: any
;; CHECK-NEXT:  %77 = LoadPropertyInst (:any) %57: any, "__wasm_max__": string
;; CHECK-NEXT:  %78 = AsNumberInst (:number) %77: any
;; CHECK-NEXT:  %79 = BinaryGreaterThanOrEqualInst (:any) %76: number, 1: number
;; CHECK-NEXT:        CondBranchInst %79: any, %BB28, %BB25
;; CHECK-NEXT:%BB28:
;; CHECK-NEXT:  %81 = BinaryStrictlyEqualInst (:any) %78: number, -1: number
;; CHECK-NEXT:        CondBranchInst %81: any, %BB25, %BB29
;; CHECK-NEXT:%BB29:
;; CHECK-NEXT:  %83 = BinaryLessThanOrEqualInst (:any) %78: number, 10: number
;; CHECK-NEXT:        CondBranchInst %83: any, %BB26, %BB25
;; CHECK-NEXT:%BB30:
;; CHECK-NEXT:  %85 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace math": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB31:
;; CHECK-NEXT:  %87 = LoadPropertyInst (:any) %72: any, "square": string
;; CHECK-NEXT:  %88 = BinaryStrictlyEqualInst (:any) %87: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %88: any, %BB32, %BB33
;; CHECK-NEXT:%BB32:
;; CHECK-NEXT:  %90 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import math.square": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB33:
;; CHECK-NEXT:  %92 = LoadPropertyInst (:any) %87: any, "__wasm_type__": string
;; CHECK-NEXT:  %93 = BinaryStrictlyEqualInst (:any) %92: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %93: any, %BB34, %BB35
;; CHECK-NEXT:%BB34:
;; CHECK-NEXT:  %95 = TypeOfInst (:string) %87: any
;; CHECK-NEXT:  %96 = BinaryStrictlyEqualInst (:any) %95: string, "function": string
;; CHECK-NEXT:        CondBranchInst %96: any, %BB36, %BB37
;; CHECK-NEXT:%BB35:
;; CHECK-NEXT:  %98 = BinaryStrictlyNotEqualInst (:any) %92: any, "func:i:i": string
;; CHECK-NEXT:        CondBranchInst %98: any, %BB37, %BB38
;; CHECK-NEXT:%BB36:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %87: any, [%VS0.import_func_1]: any
;; CHECK-NEXT:  %101 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_0(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %101: object, [%VS0.closure_0]: any
;; CHECK-NEXT:  %103 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_1(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %103: object, [%VS0.closure_1]: any
;; CHECK-NEXT:  %105 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_2(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %105: object, [%VS0.closure_2]: any
;; CHECK-NEXT:  %107 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_3(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %107: object, [%VS0.closure_3]: any
;; CHECK-NEXT:  %109 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_4(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %109: object, [%VS0.closure_4]: any
;; CHECK-NEXT:  %111 = LoadFrameInst (:any) %0: environment, [%VS0.mem_obj]: any
;; CHECK-NEXT:  %112 = LoadPropertyInst (:any) %111: any, "buffer": string
;; CHECK-NEXT:  %113 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int8Array": string
;; CHECK-NEXT:  %114 = CreateThisInst (:any) %113: any, %113: any, empty: any
;; CHECK-NEXT:  %115 = CallInst (:any) %113: any, empty: any, false: boolean, empty: any, %113: any, %114: any, %112: any
;; CHECK-NEXT:  %116 = GetConstructedObjectInst (:object) %114: any, %115: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %116: object, [%VS0.HEAP8]: any
;; CHECK-NEXT:  %118 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint8Array": string
;; CHECK-NEXT:  %119 = CreateThisInst (:any) %118: any, %118: any, empty: any
;; CHECK-NEXT:  %120 = CallInst (:any) %118: any, empty: any, false: boolean, empty: any, %118: any, %119: any, %112: any
;; CHECK-NEXT:  %121 = GetConstructedObjectInst (:object) %119: any, %120: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %121: object, [%VS0.HEAPU8]: any
;; CHECK-NEXT:  %123 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int16Array": string
;; CHECK-NEXT:  %124 = CreateThisInst (:any) %123: any, %123: any, empty: any
;; CHECK-NEXT:  %125 = CallInst (:any) %123: any, empty: any, false: boolean, empty: any, %123: any, %124: any, %112: any
;; CHECK-NEXT:  %126 = GetConstructedObjectInst (:object) %124: any, %125: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %126: object, [%VS0.HEAP16]: any
;; CHECK-NEXT:  %128 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint16Array": string
;; CHECK-NEXT:  %129 = CreateThisInst (:any) %128: any, %128: any, empty: any
;; CHECK-NEXT:  %130 = CallInst (:any) %128: any, empty: any, false: boolean, empty: any, %128: any, %129: any, %112: any
;; CHECK-NEXT:  %131 = GetConstructedObjectInst (:object) %129: any, %130: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %131: object, [%VS0.HEAPU16]: any
;; CHECK-NEXT:  %133 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int32Array": string
;; CHECK-NEXT:  %134 = CreateThisInst (:any) %133: any, %133: any, empty: any
;; CHECK-NEXT:  %135 = CallInst (:any) %133: any, empty: any, false: boolean, empty: any, %133: any, %134: any, %112: any
;; CHECK-NEXT:  %136 = GetConstructedObjectInst (:object) %134: any, %135: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %136: object, [%VS0.HEAP32]: any
;; CHECK-NEXT:  %138 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:  %139 = CreateThisInst (:any) %138: any, %138: any, empty: any
;; CHECK-NEXT:  %140 = CallInst (:any) %138: any, empty: any, false: boolean, empty: any, %138: any, %139: any, %112: any
;; CHECK-NEXT:  %141 = GetConstructedObjectInst (:object) %139: any, %140: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %141: object, [%VS0.HEAPU32]: any
;; CHECK-NEXT:  %143 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float32Array": string
;; CHECK-NEXT:  %144 = CreateThisInst (:any) %143: any, %143: any, empty: any
;; CHECK-NEXT:  %145 = CallInst (:any) %143: any, empty: any, false: boolean, empty: any, %143: any, %144: any, %112: any
;; CHECK-NEXT:  %146 = GetConstructedObjectInst (:object) %144: any, %145: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %146: object, [%VS0.HEAPF32]: any
;; CHECK-NEXT:  %148 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:  %149 = CreateThisInst (:any) %148: any, %148: any, empty: any
;; CHECK-NEXT:  %150 = CallInst (:any) %148: any, empty: any, false: boolean, empty: any, %148: any, %149: any, %112: any
;; CHECK-NEXT:  %151 = GetConstructedObjectInst (:object) %149: any, %150: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %151: object, [%VS0.HEAPF64]: any
;; CHECK-NEXT:  %153 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:  %154 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:  %155 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:  %156 = CreateThisInst (:any) %153: any, %153: any, empty: any
;; CHECK-NEXT:  %157 = CallInst (:any) %153: any, empty: any, false: boolean, empty: any, %153: any, %156: any, 8: number
;; CHECK-NEXT:  %158 = GetConstructedObjectInst (:object) %156: any, %157: any
;; CHECK-NEXT:  %159 = CreateThisInst (:any) %154: any, %154: any, empty: any
;; CHECK-NEXT:  %160 = CallInst (:any) %154: any, empty: any, false: boolean, empty: any, %154: any, %159: any, %158: object
;; CHECK-NEXT:  %161 = GetConstructedObjectInst (:object) %159: any, %160: any
;; CHECK-NEXT:  %162 = CreateThisInst (:any) %155: any, %155: any, empty: any
;; CHECK-NEXT:  %163 = CallInst (:any) %155: any, empty: any, false: boolean, empty: any, %155: any, %162: any, %158: object
;; CHECK-NEXT:  %164 = GetConstructedObjectInst (:object) %162: any, %163: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %161: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %164: object, [%VS0.retBufF]: any
;; CHECK-NEXT:  %167 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %167: any, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:  %169 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:i": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %169: any, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:  %171 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %171: any, [%VS0.wasm_type_id_2]: any
;; CHECK-NEXT:  %173 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::i": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %173: any, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:  %175 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "anyfunc": string, %175: object, "element": string
;; CHECK-NEXT:         StorePropertyStrictInst 4: number, %175: object, "initial": string
;; CHECK-NEXT:  %178 = TryLoadGlobalPropertyInst (:any) globalObject: object, "WebAssembly": string
;; CHECK-NEXT:  %179 = LoadPropertyInst (:any) %178: any, "Table": string
;; CHECK-NEXT:  %180 = CreateThisInst (:any) %179: any, %179: any, empty: any
;; CHECK-NEXT:  %181 = CallInst (:any) %179: any, empty: any, false: boolean, empty: any, %179: any, %180: any, %175: object
;; CHECK-NEXT:  %182 = GetConstructedObjectInst (:object) %180: any, %181: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %182: object, [%VS0.table_0_obj]: any
;; CHECK-NEXT:  %184 = LoadPropertyInst (:any) %182: object, "__wasm_funcs__": string
;; CHECK-NEXT:  %185 = LoadPropertyInst (:any) %182: object, "__wasm_types__": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %184: any, [%VS0.table_0_funcs]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %185: any, [%VS0.table_0_types]: any
;; CHECK-NEXT:  %188 = LoadFrameInst (:any) %0: environment, [%VS0.import_global_val_0]: any
;; CHECK-NEXT:  %189 = AsInt32Inst (:number) %188: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %189: number, [%VS0.global_0]: any
;; CHECK-NEXT:  %191 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:  %192 = CallInst (:any) %191: any, %wasm_func_2(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:  %193 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:  %194 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_run(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func::i": string, %194: object, "__wasm_type__": string
;; CHECK-NEXT:         StorePropertyStrictInst %194: object, %193: object, "run": string
;; CHECK-NEXT:  %197 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_helper(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func:i:i": string, %197: object, "__wasm_type__": string
;; CHECK-NEXT:         StorePropertyStrictInst %197: object, %193: object, "helper": string
;; CHECK-NEXT:         ReturnInst %193: object
;; CHECK-NEXT:%BB37:
;; CHECK-NEXT:  %201 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import math.square is not a function": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT:%BB38:
;; CHECK-NEXT:  %203 = TypeOfInst (:string) %87: any
;; CHECK-NEXT:  %204 = BinaryStrictlyEqualInst (:any) %203: string, "function": string
;; CHECK-NEXT:         CondBranchInst %204: any, %BB36, %BB37
;; CHECK-NEXT:function_end

;; CHECK:function wasm_export_run(): any
;; CHECK-NEXT:%BB0:
;; CHECK-NEXT:  %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:  %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_3]: any
;; CHECK-NEXT:  %2 = CallInst (:any) %1: any, %wasm_func_3(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:       ReturnInst %2: any
;; CHECK-NEXT:function_end

;; CHECK:function wasm_export_helper(p0: any): any
;; CHECK-NEXT:%BB0:
;; CHECK-NEXT:  %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:  %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_4]: any
;; CHECK-NEXT:  %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:  %3 = AsInt32Inst (:number) %2: any
;; CHECK-NEXT:  %4 = CallInst (:any) %1: any, %wasm_func_4(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %3: number
;; CHECK-NEXT:       ReturnInst %4: any
;; CHECK-NEXT:function_end
