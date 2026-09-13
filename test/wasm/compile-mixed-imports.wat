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

;; CHECK: scope %VS0 [HEAP8: any, HEAPU8: any, HEAP16: any, HEAPU16: any, HEAP32: any, HEAPU32: any, HEAPF32: any, HEAPF64: any, wasm_type_id_0: any, wasm_type_id_1: any, wasm_type_id_2: any, wasm_type_id_3: any, table_0_funcs: any, table_0_types: any, table_0_exported: any, table_0_obj: any, global_0: any, import_func_0: any, import_func_1: any, import_global_val_0: any, imported_mem_max: any, imported_mem_buf: any, mem_obj: any, retBufI: any, retBufF: any, closure_0: any, exported_func_0: any, closure_1: any, exported_func_1: any, closure_2: any, closure_3: any, exported_func_3: any, closure_4: any, exported_func_4: any, intrinsics: any]
;; CHECK-EMPTY:
;; CHECK-NEXT: function global(): object
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:   %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %__wasm_instantiate__(): functionCode
;; CHECK-NEXT:   %2 = TryLoadGlobalPropertyInst (:any) globalObject: object, "HermesInternal": string
;; CHECK-NEXT:   %3 = LoadPropertyInst (:any) %2: any, "intrinsics": string
;; CHECK-NEXT:   %4 = LoadPropertyInst (:any) %3: any, "Array": string
;; CHECK-NEXT:   %5 = CreateThisInst (:any) %4: any, %4: any, empty: any
;; CHECK-NEXT:   %6 = CallInst (:any) %4: any, empty: any, false: boolean, empty: any, %4: any, %5: any, 2: number
;; CHECK-NEXT:   %7 = GetConstructedObjectInst (:object) %5: any, %6: any
;; CHECK-NEXT:   %8 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:        StorePropertyStrictInst "run": string, %8: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %8: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %8: object, %7: object, 0: number
;; CHECK-NEXT:   %12 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "helper": string, %12: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %12: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %12: object, %7: object, 1: number
;; CHECK-NEXT:   %16 = LoadPropertyInst (:any) %3: any, "Array": string
;; CHECK-NEXT:   %17 = CreateThisInst (:any) %16: any, %16: any, empty: any
;; CHECK-NEXT:   %18 = CallInst (:any) %16: any, empty: any, false: boolean, empty: any, %16: any, %17: any, 4: number
;; CHECK-NEXT:   %19 = GetConstructedObjectInst (:object) %17: any, %18: any
;; CHECK-NEXT:   %20 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "env": string, %20: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "log": string, %20: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %20: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %20: object, %19: object, 0: number
;; CHECK-NEXT:   %25 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "config": string, %25: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "max_size": string, %25: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "global": string, %25: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %25: object, %19: object, 1: number
;; CHECK-NEXT:   %30 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "env": string, %30: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "memory": string, %30: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "memory": string, %30: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %30: object, %19: object, 2: number
;; CHECK-NEXT:   %35 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "math": string, %35: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "square": string, %35: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %35: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %35: object, %19: object, 3: number
;; CHECK-NEXT:   %40 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst %1: object, %40: object, "instantiate": string
;; CHECK-NEXT:         StorePropertyStrictInst %7: object, %40: object, "exportDescs": string
;; CHECK-NEXT:         StorePropertyStrictInst %19: object, %40: object, "importDescs": string
;; CHECK-NEXT:         ReturnInst %40: object
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
;; CHECK-NEXT: function wasm_func_1(p0: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.import_func_1]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:   %3 = CallInst (:any) %1: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: number
;; CHECK-NEXT:   %4 = AsInt32Inst (:number) %3: any
;; CHECK-NEXT:        ReturnInst %4: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_2(): undefined
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %3 = CallInst (:undefined) %2: any, %wasm_func_0(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, 0: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_3(): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = LoadFrameInst (:any) %0: environment, [%VS0.global_0]: any
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %4 = PhiInst (:any) %2: any, %BB0
;; CHECK-NEXT:        ReturnInst %4: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_4(p0: number): number
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT:   %3 = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:        StoreStackInst %3: number, %2: number
;; CHECK-NEXT:   %5 = LoadStackInst (:number) %2: number
;; CHECK-NEXT:   %6 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %7 = CallInst (:number) %6: any, %wasm_func_1(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %5: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %9 = PhiInst (:number) %7: number, %BB0
;; CHECK-NEXT:         ReturnInst %9: number
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
;; CHECK-NEXT:   %25 = LoadPropertyInst (:any) %4: any, "config": string
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
;; CHECK-NEXT:   %33 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace config": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB11:
;; CHECK-NEXT:   %35 = LoadPropertyInst (:any) %25: any, "max_size": string
;; CHECK-NEXT:   %36 = BinaryStrictlyEqualInst (:any) %35: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %36: any, %BB12, %BB13
;; CHECK-NEXT: %BB12:
;; CHECK-NEXT:   %38 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import config.max_size": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB13:
;; CHECK-NEXT:   %40 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkGlobal]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %35: any, 0: number, false: boolean
;; CHECK-NEXT:   %41 = BinaryStrictlyEqualInst (:any) %40: any, null: null
;; CHECK-NEXT:         CondBranchInst %41: any, %BB18, %BB14
;; CHECK-NEXT: %BB14:
;; CHECK-NEXT:   %43 = BinaryStrictlyEqualInst (:any) %40: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %43: any, %BB16, %BB19
;; CHECK-NEXT: %BB15:
;; CHECK-NEXT:   %45 = PhiInst (:any) %35: any, %BB18, %57: any, %BB19
;; CHECK-NEXT:         StoreFrameInst %0: environment, %45: any, [%VS0.import_global_val_0]: any
;; CHECK-NEXT:   %47 = LoadPropertyInst (:any) %4: any, "env": string
;; CHECK-NEXT:   %48 = BinaryStrictlyEqualInst (:any) %47: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %48: any, %BB20, %BB21
;; CHECK-NEXT: %BB16:
;; CHECK-NEXT:   %50 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import config.max_size is a WebAssembly.Global that does not match the declared immutable i32 global import": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB17:
;; CHECK-NEXT:   %52 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import config.max_size must be a Number to satisfy an i32 global import": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB18:
;; CHECK-NEXT:   %54 = TypeOfInst (:string) %35: any
;; CHECK-NEXT:   %55 = BinaryStrictlyEqualInst (:any) %54: string, "number": string
;; CHECK-NEXT:         CondBranchInst %55: any, %BB15, %BB17
;; CHECK-NEXT: %BB19:
;; CHECK-NEXT:   %57 = CallBuiltinInst (:any) [HermesBuiltin.wasmGlobalGet]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %40: any
;; CHECK-NEXT:         BranchInst %BB15
;; CHECK-NEXT: %BB20:
;; CHECK-NEXT:   %59 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace env": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB21:
;; CHECK-NEXT:   %61 = LoadPropertyInst (:any) %47: any, "memory": string
;; CHECK-NEXT:   %62 = BinaryStrictlyEqualInst (:any) %61: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %62: any, %BB22, %BB23
;; CHECK-NEXT: %BB22:
;; CHECK-NEXT:   %64 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.memory": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB23:
;; CHECK-NEXT:   %66 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkMemory]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %61: any
;; CHECK-NEXT:   %67 = BinaryStrictlyEqualInst (:any) %66: any, null: null
;; CHECK-NEXT:         CondBranchInst %67: any, %BB24, %BB27
;; CHECK-NEXT: %BB24:
;; CHECK-NEXT:   %69 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.memory is not a WebAssembly.Memory": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB25:
;; CHECK-NEXT:   %71 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.memory does not satisfy the declared memory limits": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB26:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %61: any, [%VS0.mem_obj]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %80: any, [%VS0.imported_mem_max]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %81: any, [%VS0.imported_mem_buf]: any
;; CHECK-NEXT:   %76 = LoadPropertyInst (:any) %4: any, "math": string
;; CHECK-NEXT:   %77 = BinaryStrictlyEqualInst (:any) %76: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %77: any, %BB30, %BB31
;; CHECK-NEXT: %BB27:
;; CHECK-NEXT:   %79 = LoadPropertyInst (:any) %66: any, 0: number
;; CHECK-NEXT:   %80 = LoadPropertyInst (:any) %66: any, 1: number
;; CHECK-NEXT:   %81 = LoadPropertyInst (:any) %66: any, 2: number
;; CHECK-NEXT:   %82 = BinaryGreaterThanOrEqualInst (:any) %79: any, 1: number
;; CHECK-NEXT:         CondBranchInst %82: any, %BB28, %BB25
;; CHECK-NEXT: %BB28:
;; CHECK-NEXT:   %84 = BinaryStrictlyEqualInst (:any) %80: any, -1: number
;; CHECK-NEXT:         CondBranchInst %84: any, %BB25, %BB29
;; CHECK-NEXT: %BB29:
;; CHECK-NEXT:   %86 = BinaryLessThanOrEqualInst (:any) %80: any, 10: number
;; CHECK-NEXT:         CondBranchInst %86: any, %BB26, %BB25
;; CHECK-NEXT: %BB30:
;; CHECK-NEXT:   %88 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace math": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB31:
;; CHECK-NEXT:   %90 = LoadPropertyInst (:any) %76: any, "square": string
;; CHECK-NEXT:   %91 = BinaryStrictlyEqualInst (:any) %90: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %91: any, %BB32, %BB33
;; CHECK-NEXT: %BB32:
;; CHECK-NEXT:   %93 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import math.square": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB33:
;; CHECK-NEXT:   %95 = CallBuiltinInst (:any) [HermesBuiltin.wasmFuncTypeId]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %90: any
;; CHECK-NEXT:   %96 = BinaryStrictlyEqualInst (:any) %95: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %96: any, %BB34, %BB35
;; CHECK-NEXT: %BB34:
;; CHECK-NEXT:   %98 = TypeOfInst (:string) %90: any
;; CHECK-NEXT:   %99 = BinaryStrictlyEqualInst (:any) %98: string, "function": string
;; CHECK-NEXT:          CondBranchInst %99: any, %BB36, %BB37
;; CHECK-NEXT: %BB35:
;; CHECK-NEXT:   %101 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:i": string
;; CHECK-NEXT:   %102 = BinaryStrictlyNotEqualInst (:any) %95: any, %101: any
;; CHECK-NEXT:          CondBranchInst %102: any, %BB37, %BB38
;; CHECK-NEXT: %BB36:
;; CHECK-NEXT:          StoreFrameInst %0: environment, %90: any, [%VS0.import_func_1]: any
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
;; CHECK-NEXT:   %115 = LoadFrameInst (:any) %0: environment, [%VS0.imported_mem_buf]: any
;; CHECK-NEXT:   %116 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %117 = LoadPropertyInst (:any) %116: any, "Int8Array": string
;; CHECK-NEXT:   %118 = CreateThisInst (:any) %117: any, %117: any, empty: any
;; CHECK-NEXT:   %119 = CallInst (:any) %117: any, empty: any, false: boolean, empty: any, %117: any, %118: any, %115: any
;; CHECK-NEXT:   %120 = GetConstructedObjectInst (:object) %118: any, %119: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %120: object, [%VS0.HEAP8]: any
;; CHECK-NEXT:   %122 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %123 = LoadPropertyInst (:any) %122: any, "Uint8Array": string
;; CHECK-NEXT:   %124 = CreateThisInst (:any) %123: any, %123: any, empty: any
;; CHECK-NEXT:   %125 = CallInst (:any) %123: any, empty: any, false: boolean, empty: any, %123: any, %124: any, %115: any
;; CHECK-NEXT:   %126 = GetConstructedObjectInst (:object) %124: any, %125: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %126: object, [%VS0.HEAPU8]: any
;; CHECK-NEXT:   %128 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %129 = LoadPropertyInst (:any) %128: any, "Int16Array": string
;; CHECK-NEXT:   %130 = CreateThisInst (:any) %129: any, %129: any, empty: any
;; CHECK-NEXT:   %131 = CallInst (:any) %129: any, empty: any, false: boolean, empty: any, %129: any, %130: any, %115: any
;; CHECK-NEXT:   %132 = GetConstructedObjectInst (:object) %130: any, %131: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %132: object, [%VS0.HEAP16]: any
;; CHECK-NEXT:   %134 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %135 = LoadPropertyInst (:any) %134: any, "Uint16Array": string
;; CHECK-NEXT:   %136 = CreateThisInst (:any) %135: any, %135: any, empty: any
;; CHECK-NEXT:   %137 = CallInst (:any) %135: any, empty: any, false: boolean, empty: any, %135: any, %136: any, %115: any
;; CHECK-NEXT:   %138 = GetConstructedObjectInst (:object) %136: any, %137: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %138: object, [%VS0.HEAPU16]: any
;; CHECK-NEXT:   %140 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %141 = LoadPropertyInst (:any) %140: any, "Int32Array": string
;; CHECK-NEXT:   %142 = CreateThisInst (:any) %141: any, %141: any, empty: any
;; CHECK-NEXT:   %143 = CallInst (:any) %141: any, empty: any, false: boolean, empty: any, %141: any, %142: any, %115: any
;; CHECK-NEXT:   %144 = GetConstructedObjectInst (:object) %142: any, %143: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %144: object, [%VS0.HEAP32]: any
;; CHECK-NEXT:   %146 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %147 = LoadPropertyInst (:any) %146: any, "Uint32Array": string
;; CHECK-NEXT:   %148 = CreateThisInst (:any) %147: any, %147: any, empty: any
;; CHECK-NEXT:   %149 = CallInst (:any) %147: any, empty: any, false: boolean, empty: any, %147: any, %148: any, %115: any
;; CHECK-NEXT:   %150 = GetConstructedObjectInst (:object) %148: any, %149: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %150: object, [%VS0.HEAPU32]: any
;; CHECK-NEXT:   %152 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %153 = LoadPropertyInst (:any) %152: any, "Float32Array": string
;; CHECK-NEXT:   %154 = CreateThisInst (:any) %153: any, %153: any, empty: any
;; CHECK-NEXT:   %155 = CallInst (:any) %153: any, empty: any, false: boolean, empty: any, %153: any, %154: any, %115: any
;; CHECK-NEXT:   %156 = GetConstructedObjectInst (:object) %154: any, %155: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %156: object, [%VS0.HEAPF32]: any
;; CHECK-NEXT:   %158 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %159 = LoadPropertyInst (:any) %158: any, "Float64Array": string
;; CHECK-NEXT:   %160 = CreateThisInst (:any) %159: any, %159: any, empty: any
;; CHECK-NEXT:   %161 = CallInst (:any) %159: any, empty: any, false: boolean, empty: any, %159: any, %160: any, %115: any
;; CHECK-NEXT:   %162 = GetConstructedObjectInst (:object) %160: any, %161: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %162: object, [%VS0.HEAPF64]: any
;; CHECK-NEXT:   %164 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %165 = LoadPropertyInst (:any) %164: any, "ArrayBuffer": string
;; CHECK-NEXT:   %166 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %167 = LoadPropertyInst (:any) %166: any, "Uint32Array": string
;; CHECK-NEXT:   %168 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %169 = LoadPropertyInst (:any) %168: any, "Float64Array": string
;; CHECK-NEXT:   %170 = CreateThisInst (:any) %165: any, %165: any, empty: any
;; CHECK-NEXT:   %171 = CallInst (:any) %165: any, empty: any, false: boolean, empty: any, %165: any, %170: any, 8: number
;; CHECK-NEXT:   %172 = GetConstructedObjectInst (:object) %170: any, %171: any
;; CHECK-NEXT:   %173 = CreateThisInst (:any) %167: any, %167: any, empty: any
;; CHECK-NEXT:   %174 = CallInst (:any) %167: any, empty: any, false: boolean, empty: any, %167: any, %173: any, %172: object
;; CHECK-NEXT:   %175 = GetConstructedObjectInst (:object) %173: any, %174: any
;; CHECK-NEXT:   %176 = CreateThisInst (:any) %169: any, %169: any, empty: any
;; CHECK-NEXT:   %177 = CallInst (:any) %169: any, empty: any, false: boolean, empty: any, %169: any, %176: any, %172: object
;; CHECK-NEXT:   %178 = GetConstructedObjectInst (:object) %176: any, %177: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %175: object, [%VS0.retBufI]: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %178: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %181 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %181: any, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %183 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:i": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %183: any, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:   %185 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %185: any, [%VS0.wasm_type_id_2]: any
;; CHECK-NEXT:   %187 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::i": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %187: any, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:   %189 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_0(): functionCode
;; CHECK-NEXT:   %190 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %191 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %192 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %189: object, %190: any, %191: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %189: object, [%VS0.exported_func_0]: any
;; CHECK-NEXT:   %194 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_1(): functionCode
;; CHECK-NEXT:   %195 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %196 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:   %197 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %194: object, %195: any, %196: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %194: object, [%VS0.exported_func_1]: any
;; CHECK-NEXT:   %199 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_run(): functionCode
;; CHECK-NEXT:   %200 = LoadFrameInst (:any) %0: environment, [%VS0.closure_3]: any
;; CHECK-NEXT:   %201 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:   %202 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %199: object, %200: any, %201: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %199: object, [%VS0.exported_func_3]: any
;; CHECK-NEXT:   %204 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_helper(): functionCode
;; CHECK-NEXT:   %205 = LoadFrameInst (:any) %0: environment, [%VS0.closure_4]: any
;; CHECK-NEXT:   %206 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:   %207 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %204: object, %205: any, %206: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %204: object, [%VS0.exported_func_4]: any
;; CHECK-NEXT:   %209 = LoadFrameInst (:any) %0: environment, [%VS0.import_global_val_0]: any
;; CHECK-NEXT:   %210 = AsInt32Inst (:number) %209: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %210: number, [%VS0.global_0]: any
;; CHECK-NEXT:   %212 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:          StorePropertyStrictInst "anyfunc": string, %212: object, "element": string
;; CHECK-NEXT:          StorePropertyStrictInst 4: number, %212: object, "initial": string
;; CHECK-NEXT:   %215 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %216 = LoadPropertyInst (:any) %215: any, "WebAssembly": string
;; CHECK-NEXT:   %217 = LoadPropertyInst (:any) %216: any, "Table": string
;; CHECK-NEXT:   %218 = CreateThisInst (:any) %217: any, %217: any, empty: any
;; CHECK-NEXT:   %219 = CallInst (:any) %217: any, empty: any, false: boolean, empty: any, %217: any, %218: any, %212: object
;; CHECK-NEXT:   %220 = GetConstructedObjectInst (:object) %218: any, %219: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %220: object, [%VS0.table_0_obj]: any
;; CHECK-NEXT:   %222 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkTable]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %220: object, true: boolean
;; CHECK-NEXT:   %223 = BinaryStrictlyEqualInst (:any) %222: any, null: null
;; CHECK-NEXT:          CondBranchInst %223: any, %BB39, %BB40
;; CHECK-NEXT: %BB37:
;; CHECK-NEXT:   %225 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import math.square is not a function": string
;; CHECK-NEXT:          UnreachableInst
;; CHECK-NEXT: %BB38:
;; CHECK-NEXT:   %227 = TypeOfInst (:string) %90: any
;; CHECK-NEXT:   %228 = BinaryStrictlyEqualInst (:any) %227: string, "function": string
;; CHECK-NEXT:          CondBranchInst %228: any, %BB36, %BB37
;; CHECK-NEXT: %BB39:
;; CHECK-NEXT:   %230 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "WebAssembly.Table did not construct a table for this module's table 0": string
;; CHECK-NEXT:          UnreachableInst
;; CHECK-NEXT: %BB40:
;; CHECK-NEXT:   %232 = LoadPropertyInst (:any) %222: any, 0: number
;; CHECK-NEXT:   %233 = LoadPropertyInst (:any) %222: any, 1: number
;; CHECK-NEXT:   %234 = LoadPropertyInst (:any) %222: any, 2: number
;; CHECK-NEXT:   %235 = LoadPropertyInst (:any) %232: any, "length": string
;; CHECK-NEXT:   %236 = LoadPropertyInst (:any) %222: any, 3: number
;; CHECK-NEXT:   %237 = BinaryStrictlyEqualInst (:any) %235: any, 4: number
;; CHECK-NEXT:          CondBranchInst %237: any, %BB42, %BB41
;; CHECK-NEXT: %BB41:
;; CHECK-NEXT:   %239 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "WebAssembly.Table did not construct a table with this module's declared limits for table 0": string
;; CHECK-NEXT:          UnreachableInst
;; CHECK-NEXT: %BB42:
;; CHECK-NEXT:   %241 = BinaryStrictlyEqualInst (:any) %236: any, -1: number
;; CHECK-NEXT:          CondBranchInst %241: any, %BB43, %BB41
;; CHECK-NEXT: %BB43:
;; CHECK-NEXT:          StoreFrameInst %0: environment, %232: any, [%VS0.table_0_funcs]: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %233: any, [%VS0.table_0_types]: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %234: any, [%VS0.table_0_exported]: any
;; CHECK-NEXT:   %246 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:   %247 = CallInst (:any) %246: any, %wasm_func_2(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:   %248 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:   %249 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_3]: any
;; CHECK-NEXT:          StorePropertyStrictInst %249: any, %248: object, "run": string
;; CHECK-NEXT:   %251 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_4]: any
;; CHECK-NEXT:          StorePropertyStrictInst %251: any, %248: object, "helper": string
;; CHECK-NEXT:          ReturnInst %248: object
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
;; CHECK-NEXT: function wasm_funcref_1(p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:   %3 = AsInt32Inst (:number) %2: any
;; CHECK-NEXT:   %4 = CallInst (:any) %1: any, %wasm_func_1(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %3: number
;; CHECK-NEXT:        ReturnInst %4: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_export_run(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_3]: any
;; CHECK-NEXT:   %2 = CallInst (:any) %1: any, %wasm_func_3(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:        ReturnInst %2: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_export_helper(p0: any): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.closure_4]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:   %3 = AsInt32Inst (:number) %2: any
;; CHECK-NEXT:   %4 = CallInst (:any) %1: any, %wasm_func_4(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %3: number
;; CHECK-NEXT:        ReturnInst %4: any
;; CHECK-NEXT: function_end
