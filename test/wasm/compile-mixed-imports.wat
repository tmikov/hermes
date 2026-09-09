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
;; CHECK-NEXT:        CondBranchInst %39: any, %BB16, %BB19
;; CHECK-NEXT:%BB15:
;; CHECK-NEXT:  %41 = PhiInst (:any) %31: any, %BB18, %53: any, %BB19
;; CHECK-NEXT:        StoreFrameInst %0: environment, %41: any, [%VS0.import_global_val_0]: any
;; CHECK-NEXT:  %43 = LoadPropertyInst (:any) %1: any, "env": string
;; CHECK-NEXT:  %44 = BinaryStrictlyEqualInst (:any) %43: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %44: any, %BB20, %BB21
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
;; CHECK-NEXT:  %53 = CallBuiltinInst (:any) [HermesBuiltin.wasmGlobalGet]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %36: any
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
;; CHECK-NEXT:  %62 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkMemory]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %57: any
;; CHECK-NEXT:  %63 = BinaryStrictlyEqualInst (:any) %62: any, null: null
;; CHECK-NEXT:        CondBranchInst %63: any, %BB24, %BB27
;; CHECK-NEXT:%BB24:
;; CHECK-NEXT:  %65 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.memory is not a WebAssembly.Memory": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB25:
;; CHECK-NEXT:  %67 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.memory does not satisfy the declared memory limits": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB26:
;; CHECK-NEXT:        StoreFrameInst %0: environment, %57: any, [%VS0.mem_obj]: any
;; CHECK-NEXT:        StoreFrameInst %0: environment, %76: any, [%VS0.imported_mem_max]: any
;; CHECK-NEXT:        StoreFrameInst %0: environment, %77: any, [%VS0.imported_mem_buf]: any
;; CHECK-NEXT:  %72 = LoadPropertyInst (:any) %1: any, "math": string
;; CHECK-NEXT:  %73 = BinaryStrictlyEqualInst (:any) %72: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %73: any, %BB30, %BB31
;; CHECK-NEXT:%BB27:
;; CHECK-NEXT:  %75 = LoadPropertyInst (:any) %62: any, 0: number
;; CHECK-NEXT:  %76 = LoadPropertyInst (:any) %62: any, 1: number
;; CHECK-NEXT:  %77 = LoadPropertyInst (:any) %62: any, 2: number
;; CHECK-NEXT:  %78 = BinaryGreaterThanOrEqualInst (:any) %75: any, 1: number
;; CHECK-NEXT:        CondBranchInst %78: any, %BB28, %BB25
;; CHECK-NEXT:%BB28:
;; CHECK-NEXT:  %80 = BinaryStrictlyEqualInst (:any) %76: any, -1: number
;; CHECK-NEXT:        CondBranchInst %80: any, %BB25, %BB29
;; CHECK-NEXT:%BB29:
;; CHECK-NEXT:  %82 = BinaryLessThanOrEqualInst (:any) %76: any, 10: number
;; CHECK-NEXT:        CondBranchInst %82: any, %BB26, %BB25
;; CHECK-NEXT:%BB30:
;; CHECK-NEXT:  %84 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace math": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB31:
;; CHECK-NEXT:  %86 = LoadPropertyInst (:any) %72: any, "square": string
;; CHECK-NEXT:  %87 = BinaryStrictlyEqualInst (:any) %86: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %87: any, %BB32, %BB33
;; CHECK-NEXT:%BB32:
;; CHECK-NEXT:  %89 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import math.square": string
;; CHECK-NEXT:        UnreachableInst
;; CHECK-NEXT:%BB33:
;; CHECK-NEXT:  %91 = LoadPropertyInst (:any) %86: any, "__wasm_type__": string
;; CHECK-NEXT:  %92 = BinaryStrictlyEqualInst (:any) %91: any, undefined: undefined
;; CHECK-NEXT:        CondBranchInst %92: any, %BB34, %BB35
;; CHECK-NEXT:%BB34:
;; CHECK-NEXT:  %94 = TypeOfInst (:string) %86: any
;; CHECK-NEXT:  %95 = BinaryStrictlyEqualInst (:any) %94: string, "function": string
;; CHECK-NEXT:        CondBranchInst %95: any, %BB36, %BB37
;; CHECK-NEXT:%BB35:
;; CHECK-NEXT:  %97 = BinaryStrictlyNotEqualInst (:any) %91: any, "func:i:i": string
;; CHECK-NEXT:        CondBranchInst %97: any, %BB37, %BB38
;; CHECK-NEXT:%BB36:
;; CHECK-NEXT:        StoreFrameInst %0: environment, %86: any, [%VS0.import_func_1]: any
;; CHECK-NEXT:  %100 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_0(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %100: object, [%VS0.closure_0]: any
;; CHECK-NEXT:  %102 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_1(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %102: object, [%VS0.closure_1]: any
;; CHECK-NEXT:  %104 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_2(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %104: object, [%VS0.closure_2]: any
;; CHECK-NEXT:  %106 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_3(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %106: object, [%VS0.closure_3]: any
;; CHECK-NEXT:  %108 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_4(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %108: object, [%VS0.closure_4]: any
;; CHECK-NEXT:  %110 = LoadFrameInst (:any) %0: environment, [%VS0.imported_mem_buf]: any
;; CHECK-NEXT:  %111 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int8Array": string
;; CHECK-NEXT:  %112 = CreateThisInst (:any) %111: any, %111: any, empty: any
;; CHECK-NEXT:  %113 = CallInst (:any) %111: any, empty: any, false: boolean, empty: any, %111: any, %112: any, %110: any
;; CHECK-NEXT:  %114 = GetConstructedObjectInst (:object) %112: any, %113: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %114: object, [%VS0.HEAP8]: any
;; CHECK-NEXT:  %116 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint8Array": string
;; CHECK-NEXT:  %117 = CreateThisInst (:any) %116: any, %116: any, empty: any
;; CHECK-NEXT:  %118 = CallInst (:any) %116: any, empty: any, false: boolean, empty: any, %116: any, %117: any, %110: any
;; CHECK-NEXT:  %119 = GetConstructedObjectInst (:object) %117: any, %118: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %119: object, [%VS0.HEAPU8]: any
;; CHECK-NEXT:  %121 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int16Array": string
;; CHECK-NEXT:  %122 = CreateThisInst (:any) %121: any, %121: any, empty: any
;; CHECK-NEXT:  %123 = CallInst (:any) %121: any, empty: any, false: boolean, empty: any, %121: any, %122: any, %110: any
;; CHECK-NEXT:  %124 = GetConstructedObjectInst (:object) %122: any, %123: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %124: object, [%VS0.HEAP16]: any
;; CHECK-NEXT:  %126 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint16Array": string
;; CHECK-NEXT:  %127 = CreateThisInst (:any) %126: any, %126: any, empty: any
;; CHECK-NEXT:  %128 = CallInst (:any) %126: any, empty: any, false: boolean, empty: any, %126: any, %127: any, %110: any
;; CHECK-NEXT:  %129 = GetConstructedObjectInst (:object) %127: any, %128: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %129: object, [%VS0.HEAPU16]: any
;; CHECK-NEXT:  %131 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int32Array": string
;; CHECK-NEXT:  %132 = CreateThisInst (:any) %131: any, %131: any, empty: any
;; CHECK-NEXT:  %133 = CallInst (:any) %131: any, empty: any, false: boolean, empty: any, %131: any, %132: any, %110: any
;; CHECK-NEXT:  %134 = GetConstructedObjectInst (:object) %132: any, %133: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %134: object, [%VS0.HEAP32]: any
;; CHECK-NEXT:  %136 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:  %137 = CreateThisInst (:any) %136: any, %136: any, empty: any
;; CHECK-NEXT:  %138 = CallInst (:any) %136: any, empty: any, false: boolean, empty: any, %136: any, %137: any, %110: any
;; CHECK-NEXT:  %139 = GetConstructedObjectInst (:object) %137: any, %138: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %139: object, [%VS0.HEAPU32]: any
;; CHECK-NEXT:  %141 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float32Array": string
;; CHECK-NEXT:  %142 = CreateThisInst (:any) %141: any, %141: any, empty: any
;; CHECK-NEXT:  %143 = CallInst (:any) %141: any, empty: any, false: boolean, empty: any, %141: any, %142: any, %110: any
;; CHECK-NEXT:  %144 = GetConstructedObjectInst (:object) %142: any, %143: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %144: object, [%VS0.HEAPF32]: any
;; CHECK-NEXT:  %146 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:  %147 = CreateThisInst (:any) %146: any, %146: any, empty: any
;; CHECK-NEXT:  %148 = CallInst (:any) %146: any, empty: any, false: boolean, empty: any, %146: any, %147: any, %110: any
;; CHECK-NEXT:  %149 = GetConstructedObjectInst (:object) %147: any, %148: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %149: object, [%VS0.HEAPF64]: any
;; CHECK-NEXT:  %151 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:  %152 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:  %153 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:  %154 = CreateThisInst (:any) %151: any, %151: any, empty: any
;; CHECK-NEXT:  %155 = CallInst (:any) %151: any, empty: any, false: boolean, empty: any, %151: any, %154: any, 8: number
;; CHECK-NEXT:  %156 = GetConstructedObjectInst (:object) %154: any, %155: any
;; CHECK-NEXT:  %157 = CreateThisInst (:any) %152: any, %152: any, empty: any
;; CHECK-NEXT:  %158 = CallInst (:any) %152: any, empty: any, false: boolean, empty: any, %152: any, %157: any, %156: object
;; CHECK-NEXT:  %159 = GetConstructedObjectInst (:object) %157: any, %158: any
;; CHECK-NEXT:  %160 = CreateThisInst (:any) %153: any, %153: any, empty: any
;; CHECK-NEXT:  %161 = CallInst (:any) %153: any, empty: any, false: boolean, empty: any, %153: any, %160: any, %156: object
;; CHECK-NEXT:  %162 = GetConstructedObjectInst (:object) %160: any, %161: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %159: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %162: object, [%VS0.retBufF]: any
;; CHECK-NEXT:  %165 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %165: any, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:  %167 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:i": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %167: any, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:  %169 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %169: any, [%VS0.wasm_type_id_2]: any
;; CHECK-NEXT:  %171 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::i": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %171: any, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:  %173 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_0(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func:i:": string, %173: object, "__wasm_type__": string
;; CHECK-NEXT:  %175 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:  %176 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:  %177 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %173: object, %175: any, %176: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %173: object, [%VS0.exported_func_0]: any
;; CHECK-NEXT:  %179 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_1(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func:i:i": string, %179: object, "__wasm_type__": string
;; CHECK-NEXT:  %181 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:  %182 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:  %183 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %179: object, %181: any, %182: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %179: object, [%VS0.exported_func_1]: any
;; CHECK-NEXT:  %185 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_run(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func::i": string, %185: object, "__wasm_type__": string
;; CHECK-NEXT:  %187 = LoadFrameInst (:any) %0: environment, [%VS0.closure_3]: any
;; CHECK-NEXT:  %188 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:  %189 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %185: object, %187: any, %188: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %185: object, [%VS0.exported_func_3]: any
;; CHECK-NEXT:  %191 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_helper(): functionCode
;; CHECK-NEXT:         StorePropertyStrictInst "func:i:i": string, %191: object, "__wasm_type__": string
;; CHECK-NEXT:  %193 = LoadFrameInst (:any) %0: environment, [%VS0.closure_4]: any
;; CHECK-NEXT:  %194 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:  %195 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %191: object, %193: any, %194: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %191: object, [%VS0.exported_func_4]: any
;; CHECK-NEXT:  %197 = LoadFrameInst (:any) %0: environment, [%VS0.import_global_val_0]: any
;; CHECK-NEXT:  %198 = AsInt32Inst (:number) %197: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %198: number, [%VS0.global_0]: any
;; CHECK-NEXT:  %200 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "anyfunc": string, %200: object, "element": string
;; CHECK-NEXT:         StorePropertyStrictInst 4: number, %200: object, "initial": string
;; CHECK-NEXT:  %203 = TryLoadGlobalPropertyInst (:any) globalObject: object, "WebAssembly": string
;; CHECK-NEXT:  %204 = LoadPropertyInst (:any) %203: any, "Table": string
;; CHECK-NEXT:  %205 = CreateThisInst (:any) %204: any, %204: any, empty: any
;; CHECK-NEXT:  %206 = CallInst (:any) %204: any, empty: any, false: boolean, empty: any, %204: any, %205: any, %200: object
;; CHECK-NEXT:  %207 = GetConstructedObjectInst (:object) %205: any, %206: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %207: object, [%VS0.table_0_obj]: any
;; CHECK-NEXT:  %209 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkTable]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %207: object, true: boolean
;; CHECK-NEXT:  %210 = BinaryStrictlyEqualInst (:any) %209: any, null: null
;; CHECK-NEXT:         CondBranchInst %210: any, %BB39, %BB40
;; CHECK-NEXT:%BB37:
;; CHECK-NEXT:  %212 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import math.square is not a function": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT:%BB38:
;; CHECK-NEXT:  %214 = TypeOfInst (:string) %86: any
;; CHECK-NEXT:  %215 = BinaryStrictlyEqualInst (:any) %214: string, "function": string
;; CHECK-NEXT:         CondBranchInst %215: any, %BB36, %BB37
;; CHECK-NEXT:%BB39:
;; CHECK-NEXT:  %217 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "WebAssembly.Table did not construct a table for this module's table 0": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT:%BB40:
;; CHECK-NEXT:  %219 = LoadPropertyInst (:any) %209: any, 0: number
;; CHECK-NEXT:  %220 = LoadPropertyInst (:any) %209: any, 1: number
;; CHECK-NEXT:  %221 = LoadPropertyInst (:any) %209: any, 2: number
;; CHECK-NEXT:  %222 = LoadPropertyInst (:any) %219: any, "length": string
;; CHECK-NEXT:  %223 = LoadPropertyInst (:any) %209: any, 3: number
;; CHECK-NEXT:  %224 = BinaryStrictlyEqualInst (:any) %222: any, 4: number
;; CHECK-NEXT:         CondBranchInst %224: any, %BB42, %BB41
;; CHECK-NEXT:%BB41:
;; CHECK-NEXT:  %226 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "WebAssembly.Table did not construct a table with this module's declared limits for table 0": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT:%BB42:
;; CHECK-NEXT:  %228 = BinaryStrictlyEqualInst (:any) %223: any, -1: number
;; CHECK-NEXT:         CondBranchInst %228: any, %BB43, %BB41
;; CHECK-NEXT:%BB43:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %219: any, [%VS0.table_0_funcs]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %220: any, [%VS0.table_0_types]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %221: any, [%VS0.table_0_exported]: any
;; CHECK-NEXT:  %233 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:  %234 = CallInst (:any) %233: any, %wasm_func_2(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:  %235 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:  %236 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_3]: any
;; CHECK-NEXT:         StorePropertyStrictInst %236: any, %235: object, "run": string
;; CHECK-NEXT:  %238 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_4]: any
;; CHECK-NEXT:         StorePropertyStrictInst %238: any, %235: object, "helper": string
;; CHECK-NEXT:         ReturnInst %235: object
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
