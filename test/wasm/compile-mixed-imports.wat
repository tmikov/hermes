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

;; CHECK:scope %VS0 [HEAP8: any, HEAPU8: any, HEAP16: any, HEAPU16: any, HEAP32: any, HEAPU32: any, HEAPF32: any, HEAPF64: any, wasm_type_id_0: any, wasm_type_id_1: any, wasm_type_id_2: any, wasm_type_id_3: any, table_0_funcs: any, table_0_types: any, table_0_exported: any, table_0_obj: any, global_0: any, import_func_0: any, import_func_1: any, import_global_val_0: any, imported_mem_max: any, imported_mem_buf: any, mem_obj: any, retBufI: any, retBufF: any, closure_0: any, exported_func_0: any, closure_1: any, exported_func_1: any, closure_2: any, closure_3: any, exported_func_3: any, closure_4: any, exported_func_4: any]

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
;; CHECK-NEXT:  %36 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkGlobal]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %31: any, 0: number, false: boolean
;; CHECK-NEXT:  %37 = BinaryStrictlyEqualInst (:any) %36: any, null: null
;; CHECK-NEXT:        CondBranchInst %37: any, %BB18, %BB14
;; CHECK-NEXT:%BB14:
;; CHECK-NEXT:  %39 = BinaryStrictlyEqualInst (:any) %36: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %39: any, %BB16, %BB15
;; CHECK-NEXT:%BB15:
;; CHECK-NEXT:  %41 = PhiInst (:any) %31: any, %BB18, %36: any, %BB14
;; CHECK-NEXT:        StoreFrameInst %0: environment, %41: any, [%VS0.import_global_val_0]: any
;; CHECK-NEXT:  %43 = LoadPropertyInst (:any) %1: any, "env": string
;; CHECK-NEXT:  %44 = BinaryStrictlyEqualInst (:any) %43: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %44: any, %BB19, %BB20
;; CHECK-NEXT:%BB16:
;; CHECK-NEXT:  %46 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import config.max_size is a WebAssembly.Global that does not match the declared immutable i32 global import": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB17:
;; CHECK-NEXT:  %48 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import config.max_size must be a Number to satisfy an i32 global import": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB18:
;; CHECK-NEXT:  %50 = TypeOfInst (:string) %31: any
;; CHECK-NEXT:  %51 = BinaryStrictlyEqualInst (:any) %50: string, "number": string
;; CHECK-NEXT:        CondBranchInst %51: any, %BB15, %BB17
;; CHECK-NEXT:%BB19:
;; CHECK-NEXT:  %53 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace env": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB20:
;; CHECK-NEXT:  %55 = LoadPropertyInst (:any) %43: any, "memory": string
;; CHECK-NEXT:  %56 = BinaryStrictlyEqualInst (:any) %55: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %56: any, %BB21, %BB22
;; CHECK-NEXT:%BB21:
;; CHECK-NEXT:  %58 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.memory": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB22:
;; CHECK-NEXT:  %60 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkMemory]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %55: any
;; CHECK-NEXT:  %61 = BinaryStrictlyEqualInst (:any) %60: any, null: null
;; CHECK-NEXT:        CondBranchInst %61: any, %BB23, %BB26
;; CHECK-NEXT:%BB23:
;; CHECK-NEXT:  %63 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.memory is not a WebAssembly.Memory": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB24:
;; CHECK-NEXT:  %65 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.memory does not satisfy the declared memory limits": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB25:
;; CHECK-NEXT:        StoreFrameInst %0: environment, %55: any, [%VS0.mem_obj]: any
;; CHECK-NEXT:        StoreFrameInst %0: environment, %74: any, [%VS0.imported_mem_max]: any
;; CHECK-NEXT:        StoreFrameInst %0: environment, %75: any, [%VS0.imported_mem_buf]: any
;; CHECK-NEXT:  %70 = LoadPropertyInst (:any) %1: any, "math": string
;; CHECK-NEXT:  %71 = BinaryStrictlyEqualInst (:any) %70: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %71: any, %BB29, %BB30
;; CHECK-NEXT:%BB26:
;; CHECK-NEXT:  %73 = LoadPropertyInst (:any) %60: any, 0: number
;; CHECK-NEXT:  %74 = LoadPropertyInst (:any) %60: any, 1: number
;; CHECK-NEXT:  %75 = LoadPropertyInst (:any) %60: any, 2: number
;; CHECK-NEXT:  %76 = BinaryGreaterThanOrEqualInst (:any) %73: any, 1: number
;; CHECK-NEXT:        CondBranchInst %76: any, %BB27, %BB24
;; CHECK-NEXT:%BB27:
;; CHECK-NEXT:  %78 = BinaryStrictlyEqualInst (:any) %74: any, -1: number
;; CHECK-NEXT:        CondBranchInst %78: any, %BB24, %BB28
;; CHECK-NEXT:%BB28:
;; CHECK-NEXT:  %80 = BinaryLessThanOrEqualInst (:any) %74: any, 10: number
;; CHECK-NEXT:        CondBranchInst %80: any, %BB25, %BB24
;; CHECK-NEXT:%BB29:
;; CHECK-NEXT:  %82 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace math": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB30:
;; CHECK-NEXT:  %84 = LoadPropertyInst (:any) %70: any, "square": string
;; CHECK-NEXT:  %85 = BinaryStrictlyEqualInst (:any) %84: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %85: any, %BB31, %BB32
;; CHECK-NEXT:%BB31:
;; CHECK-NEXT:  %87 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import math.square": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB32:
;; CHECK-NEXT:  %89 = LoadPropertyInst (:any) %84: any, "__wasm_type__": string
;; CHECK-NEXT:  %90 = BinaryStrictlyEqualInst (:any) %89: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %90: any, %BB33, %BB34
;; CHECK-NEXT:%BB33:
;; CHECK-NEXT:  %92 = TypeOfInst (:string) %84: any
;; CHECK-NEXT:  %93 = BinaryStrictlyEqualInst (:any) %92: string, "function": string
;; CHECK-NEXT:        CondBranchInst %93: any, %BB35, %BB36
;; CHECK-NEXT:%BB34:
;; CHECK-NEXT:  %95 = BinaryStrictlyNotEqualInst (:any) %89: any, "func:i:i": string
;; CHECK-NEXT:        CondBranchInst %95: any, %BB36, %BB37
;; CHECK-NEXT:%BB35:
;; CHECK-NEXT:        StoreFrameInst %0: environment, %84: any, [%VS0.import_func_1]: any
;; CHECK-NEXT:  %98 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_0(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %98: object, [%VS0.closure_0]: any
;; CHECK-NEXT:  %100 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_1(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %100: object, [%VS0.closure_1]: any
;; CHECK-NEXT:  %102 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_2(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %102: object, [%VS0.closure_2]: any
;; CHECK-NEXT:  %104 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_3(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %104: object, [%VS0.closure_3]: any
;; CHECK-NEXT:  %106 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_4(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %106: object, [%VS0.closure_4]: any
;; CHECK-NEXT:  %108 = LoadFrameInst (:any) %0: environment, [%VS0.imported_mem_buf]: any
;; CHECK-NEXT:  %109 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int8Array": string
;; CHECK-NEXT:  %110 = CreateThisInst (:any) %109: any, %109: any, empty: any
;; CHECK-NEXT:  %111 = CallInst (:any) %109: any, empty: any, false: boolean, empty: any, %109: any, %110: any, %108: any
;; CHECK-NEXT:  %112 = GetConstructedObjectInst (:object) %110: any, %111: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %112: object, [%VS0.HEAP8]: any
;; CHECK-NEXT:  %114 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint8Array": string
;; CHECK-NEXT:  %115 = CreateThisInst (:any) %114: any, %114: any, empty: any
;; CHECK-NEXT:  %116 = CallInst (:any) %114: any, empty: any, false: boolean, empty: any, %114: any, %115: any, %108: any
;; CHECK-NEXT:  %117 = GetConstructedObjectInst (:object) %115: any, %116: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %117: object, [%VS0.HEAPU8]: any
;; CHECK-NEXT:  %119 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int16Array": string
;; CHECK-NEXT:  %120 = CreateThisInst (:any) %119: any, %119: any, empty: any
;; CHECK-NEXT:  %121 = CallInst (:any) %119: any, empty: any, false: boolean, empty: any, %119: any, %120: any, %108: any
;; CHECK-NEXT:  %122 = GetConstructedObjectInst (:object) %120: any, %121: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %122: object, [%VS0.HEAP16]: any
;; CHECK-NEXT:  %124 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint16Array": string
;; CHECK-NEXT:  %125 = CreateThisInst (:any) %124: any, %124: any, empty: any
;; CHECK-NEXT:  %126 = CallInst (:any) %124: any, empty: any, false: boolean, empty: any, %124: any, %125: any, %108: any
;; CHECK-NEXT:  %127 = GetConstructedObjectInst (:object) %125: any, %126: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %127: object, [%VS0.HEAPU16]: any
;; CHECK-NEXT:  %129 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int32Array": string
;; CHECK-NEXT:  %130 = CreateThisInst (:any) %129: any, %129: any, empty: any
;; CHECK-NEXT:  %131 = CallInst (:any) %129: any, empty: any, false: boolean, empty: any, %129: any, %130: any, %108: any
;; CHECK-NEXT:  %132 = GetConstructedObjectInst (:object) %130: any, %131: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %132: object, [%VS0.HEAP32]: any
;; CHECK-NEXT:  %134 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:  %135 = CreateThisInst (:any) %134: any, %134: any, empty: any
;; CHECK-NEXT:  %136 = CallInst (:any) %134: any, empty: any, false: boolean, empty: any, %134: any, %135: any, %108: any
;; CHECK-NEXT:  %137 = GetConstructedObjectInst (:object) %135: any, %136: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %137: object, [%VS0.HEAPU32]: any
;; CHECK-NEXT:  %139 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float32Array": string
;; CHECK-NEXT:  %140 = CreateThisInst (:any) %139: any, %139: any, empty: any
;; CHECK-NEXT:  %141 = CallInst (:any) %139: any, empty: any, false: boolean, empty: any, %139: any, %140: any, %108: any
;; CHECK-NEXT:  %142 = GetConstructedObjectInst (:object) %140: any, %141: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %142: object, [%VS0.HEAPF32]: any
;; CHECK-NEXT:  %144 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:  %145 = CreateThisInst (:any) %144: any, %144: any, empty: any
;; CHECK-NEXT:  %146 = CallInst (:any) %144: any, empty: any, false: boolean, empty: any, %144: any, %145: any, %108: any
;; CHECK-NEXT:  %147 = GetConstructedObjectInst (:object) %145: any, %146: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %147: object, [%VS0.HEAPF64]: any
;; CHECK-NEXT:  %149 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:  %150 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:  %151 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:  %152 = CreateThisInst (:any) %149: any, %149: any, empty: any
;; CHECK-NEXT:  %153 = CallInst (:any) %149: any, empty: any, false: boolean, empty: any, %149: any, %152: any, 8: number
;; CHECK-NEXT:  %154 = GetConstructedObjectInst (:object) %152: any, %153: any
;; CHECK-NEXT:  %155 = CreateThisInst (:any) %150: any, %150: any, empty: any
;; CHECK-NEXT:  %156 = CallInst (:any) %150: any, empty: any, false: boolean, empty: any, %150: any, %155: any, %154: object
;; CHECK-NEXT:  %157 = GetConstructedObjectInst (:object) %155: any, %156: any
;; CHECK-NEXT:  %158 = CreateThisInst (:any) %151: any, %151: any, empty: any
;; CHECK-NEXT:  %159 = CallInst (:any) %151: any, empty: any, false: boolean, empty: any, %151: any, %158: any, %154: object
;; CHECK-NEXT:  %160 = GetConstructedObjectInst (:object) %158: any, %159: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %157: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %160: object, [%VS0.retBufF]: any
;; CHECK-NEXT:  %163 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %163: any, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:  %165 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:i": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %165: any, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:  %167 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %167: any, [%VS0.wasm_type_id_2]: any
;; CHECK-NEXT:  %169 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::i": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %169: any, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:  %171 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_0(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func:i:": string, %171: object, "__wasm_type__": string
;; CHECK-NEXT:  %173 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:  %174 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:  %175 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %171: object, %173: any, %174: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %171: object, [%VS0.exported_func_0]: any
;; CHECK-NEXT:  %177 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_1(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func:i:i": string, %177: object, "__wasm_type__": string
;; CHECK-NEXT:  %179 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:  %180 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:  %181 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %177: object, %179: any, %180: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %177: object, [%VS0.exported_func_1]: any
;; CHECK-NEXT:  %183 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_run(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func::i": string, %183: object, "__wasm_type__": string
;; CHECK-NEXT:  %185 = LoadFrameInst (:any) %0: environment, [%VS0.closure_3]: any
;; CHECK-NEXT:  %186 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:  %187 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %183: object, %185: any, %186: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %183: object, [%VS0.exported_func_3]: any
;; CHECK-NEXT:  %189 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_helper(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func:i:i": string, %189: object, "__wasm_type__": string
;; CHECK-NEXT:  %191 = LoadFrameInst (:any) %0: environment, [%VS0.closure_4]: any
;; CHECK-NEXT:  %192 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:  %193 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %189: object, %191: any, %192: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %189: object, [%VS0.exported_func_4]: any
;; CHECK-NEXT:  %195 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "anyfunc": string, %195: object, "element": string
;; CHECK-NEXT:         StorePropertyStrictInst 4: number, %195: object, "initial": string
;; CHECK-NEXT:  %198 = TryLoadGlobalPropertyInst (:any) globalObject: object, "WebAssembly": string
;; CHECK-NEXT:  %199 = LoadPropertyInst (:any) %198: any, "Table": string
;; CHECK-NEXT:  %200 = CreateThisInst (:any) %199: any, %199: any, empty: any
;; CHECK-NEXT:  %201 = CallInst (:any) %199: any, empty: any, false: boolean, empty: any, %199: any, %200: any, %195: object
;; CHECK-NEXT:  %202 = GetConstructedObjectInst (:object) %200: any, %201: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %202: object, [%VS0.table_0_obj]: any
;; CHECK-NEXT:  %204 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkTable]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %202: object, true: boolean
;; CHECK-NEXT:  %205 = BinaryStrictlyEqualInst (:any) %204: any, null: null
;; CHECK-NEXT:         CondBranchInst %205: any, %BB38, %BB39
;; CHECK-NEXT:%BB36:
;; CHECK-NEXT:  %207 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import math.square is not a function": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT:%BB37:
;; CHECK-NEXT:  %209 = TypeOfInst (:string) %84: any
;; CHECK-NEXT:  %210 = BinaryStrictlyEqualInst (:any) %209: string, "function": string
;; CHECK-NEXT:         CondBranchInst %210: any, %BB35, %BB36
;; CHECK-NEXT:%BB38:
;; CHECK-NEXT:  %212 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "WebAssembly.Table did not construct a table for this module's table 0": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT:%BB39:
;; CHECK-NEXT:  %214 = LoadPropertyInst (:any) %204: any, 0: number
;; CHECK-NEXT:  %215 = LoadPropertyInst (:any) %204: any, 1: number
;; CHECK-NEXT:  %216 = LoadPropertyInst (:any) %204: any, 2: number
;; CHECK-NEXT:  %217 = LoadPropertyInst (:any) %214: any, "length": string
;; CHECK-NEXT:  %218 = LoadPropertyInst (:any) %204: any, 3: number
;; CHECK-NEXT:  %219 = BinaryStrictlyEqualInst (:any) %217: any, 4: number
;; CHECK-NEXT:         CondBranchInst %219: any, %BB41, %BB40
;; CHECK-NEXT:%BB40:
;; CHECK-NEXT:  %221 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "WebAssembly.Table did not construct a table with this module's declared limits for table 0": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT:%BB41:
;; CHECK-NEXT:  %223 = BinaryStrictlyEqualInst (:any) %218: any, -1: number
;; CHECK-NEXT:         CondBranchInst %223: any, %BB42, %BB40
;; CHECK-NEXT:%BB42:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %214: any, [%VS0.table_0_funcs]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %215: any, [%VS0.table_0_types]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %216: any, [%VS0.table_0_exported]: any
;; CHECK-NEXT:  %228 = LoadFrameInst (:any) %0: environment, [%VS0.import_global_val_0]: any
;; CHECK-NEXT:  %229 = AsInt32Inst (:number) %228: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %229: number, [%VS0.global_0]: any
;; CHECK-NEXT:  %231 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:  %232 = CallInst (:any) %231: any, %wasm_func_2(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:  %233 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:  %234 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_3]: any
;; CHECK-NEXT:         StorePropertyStrictInst %234: any, %233: object, "run": string
;; CHECK-NEXT:  %236 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_4]: any
;; CHECK-NEXT:         StorePropertyStrictInst %236: any, %233: object, "helper": string
;; CHECK-NEXT:         ReturnInst %233: object
;; CHECK-NEXT:function_end

;; CHECK:function wasm_funcref_0(p0: any): any
;; CHECK-NEXT:%BB0:
;; CHECK-NEXT:  %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:  %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:  %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:  %3 = AsInt32Inst (:number) %2: any
;; CHECK-NEXT:  %4 = CallInst (:any) %1: any, %wasm_func_0(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %3: number
;; CHECK-NEXT:       ReturnInst undefined: undefined
;; CHECK-NEXT:function_end

;; CHECK:function wasm_funcref_1(p0: any): any
;; CHECK-NEXT:%BB0:
;; CHECK-NEXT:  %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:  %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:  %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:  %3 = AsInt32Inst (:number) %2: any
;; CHECK-NEXT:  %4 = CallInst (:any) %1: any, %wasm_func_1(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %3: number
;; CHECK-NEXT:       ReturnInst %4: any
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
