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
;; CHECK-NEXT:   %15 = LoadPropertyInst (:any) %10: any, "__wasm_type__": string
;; CHECK-NEXT:   %16 = BinaryStrictlyEqualInst (:any) %15: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %16: any, %BB5, %BB6
;; CHECK-NEXT: %BB5:
;; CHECK-NEXT:   %18 = TypeOfInst (:string) %10: any
;; CHECK-NEXT:   %19 = BinaryStrictlyEqualInst (:any) %18: string, "function": string
;; CHECK-NEXT:         CondBranchInst %19: any, %BB7, %BB8
;; CHECK-NEXT: %BB6:
;; CHECK-NEXT:   %21 = BinaryStrictlyNotEqualInst (:any) %15: any, "func:i:": string
;; CHECK-NEXT:         CondBranchInst %21: any, %BB8, %BB9
;; CHECK-NEXT: %BB7:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %10: any, [%VS0.import_func_0]: any
;; CHECK-NEXT:   %24 = LoadPropertyInst (:any) %4: any, "config": string
;; CHECK-NEXT:   %25 = BinaryStrictlyEqualInst (:any) %24: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %25: any, %BB10, %BB11
;; CHECK-NEXT: %BB8:
;; CHECK-NEXT:   %27 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.log is not a function": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB9:
;; CHECK-NEXT:   %29 = TypeOfInst (:string) %10: any
;; CHECK-NEXT:   %30 = BinaryStrictlyEqualInst (:any) %29: string, "function": string
;; CHECK-NEXT:         CondBranchInst %30: any, %BB7, %BB8
;; CHECK-NEXT: %BB10:
;; CHECK-NEXT:   %32 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace config": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB11:
;; CHECK-NEXT:   %34 = LoadPropertyInst (:any) %24: any, "max_size": string
;; CHECK-NEXT:   %35 = BinaryStrictlyEqualInst (:any) %34: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %35: any, %BB12, %BB13
;; CHECK-NEXT: %BB12:
;; CHECK-NEXT:   %37 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import config.max_size": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB13:
;; CHECK-NEXT:   %39 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkGlobal]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %34: any, 0: number, false: boolean
;; CHECK-NEXT:   %40 = BinaryStrictlyEqualInst (:any) %39: any, null: null
;; CHECK-NEXT:         CondBranchInst %40: any, %BB18, %BB14
;; CHECK-NEXT: %BB14:
;; CHECK-NEXT:   %42 = BinaryStrictlyEqualInst (:any) %39: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %42: any, %BB16, %BB19
;; CHECK-NEXT: %BB15:
;; CHECK-NEXT:   %44 = PhiInst (:any) %34: any, %BB18, %56: any, %BB19
;; CHECK-NEXT:         StoreFrameInst %0: environment, %44: any, [%VS0.import_global_val_0]: any
;; CHECK-NEXT:   %46 = LoadPropertyInst (:any) %4: any, "env": string
;; CHECK-NEXT:   %47 = BinaryStrictlyEqualInst (:any) %46: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %47: any, %BB20, %BB21
;; CHECK-NEXT: %BB16:
;; CHECK-NEXT:   %49 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import config.max_size is a WebAssembly.Global that does not match the declared immutable i32 global import": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB17:
;; CHECK-NEXT:   %51 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import config.max_size must be a Number to satisfy an i32 global import": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB18:
;; CHECK-NEXT:   %53 = TypeOfInst (:string) %34: any
;; CHECK-NEXT:   %54 = BinaryStrictlyEqualInst (:any) %53: string, "number": string
;; CHECK-NEXT:         CondBranchInst %54: any, %BB15, %BB17
;; CHECK-NEXT: %BB19:
;; CHECK-NEXT:   %56 = CallBuiltinInst (:any) [HermesBuiltin.wasmGlobalGet]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %39: any
;; CHECK-NEXT:         BranchInst %BB15
;; CHECK-NEXT: %BB20:
;; CHECK-NEXT:   %58 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace env": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB21:
;; CHECK-NEXT:   %60 = LoadPropertyInst (:any) %46: any, "memory": string
;; CHECK-NEXT:   %61 = BinaryStrictlyEqualInst (:any) %60: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %61: any, %BB22, %BB23
;; CHECK-NEXT: %BB22:
;; CHECK-NEXT:   %63 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import env.memory": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB23:
;; CHECK-NEXT:   %65 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkMemory]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %60: any
;; CHECK-NEXT:   %66 = BinaryStrictlyEqualInst (:any) %65: any, null: null
;; CHECK-NEXT:         CondBranchInst %66: any, %BB24, %BB27
;; CHECK-NEXT: %BB24:
;; CHECK-NEXT:   %68 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.memory is not a WebAssembly.Memory": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB25:
;; CHECK-NEXT:   %70 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import env.memory does not satisfy the declared memory limits": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB26:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %60: any, [%VS0.mem_obj]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %79: any, [%VS0.imported_mem_max]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %80: any, [%VS0.imported_mem_buf]: any
;; CHECK-NEXT:   %75 = LoadPropertyInst (:any) %4: any, "math": string
;; CHECK-NEXT:   %76 = BinaryStrictlyEqualInst (:any) %75: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %76: any, %BB30, %BB31
;; CHECK-NEXT: %BB27:
;; CHECK-NEXT:   %78 = LoadPropertyInst (:any) %65: any, 0: number
;; CHECK-NEXT:   %79 = LoadPropertyInst (:any) %65: any, 1: number
;; CHECK-NEXT:   %80 = LoadPropertyInst (:any) %65: any, 2: number
;; CHECK-NEXT:   %81 = BinaryGreaterThanOrEqualInst (:any) %78: any, 1: number
;; CHECK-NEXT:         CondBranchInst %81: any, %BB28, %BB25
;; CHECK-NEXT: %BB28:
;; CHECK-NEXT:   %83 = BinaryStrictlyEqualInst (:any) %79: any, -1: number
;; CHECK-NEXT:         CondBranchInst %83: any, %BB25, %BB29
;; CHECK-NEXT: %BB29:
;; CHECK-NEXT:   %85 = BinaryLessThanOrEqualInst (:any) %79: any, 10: number
;; CHECK-NEXT:         CondBranchInst %85: any, %BB26, %BB25
;; CHECK-NEXT: %BB30:
;; CHECK-NEXT:   %87 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import namespace math": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB31:
;; CHECK-NEXT:   %89 = LoadPropertyInst (:any) %75: any, "square": string
;; CHECK-NEXT:   %90 = BinaryStrictlyEqualInst (:any) %89: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %90: any, %BB32, %BB33
;; CHECK-NEXT: %BB32:
;; CHECK-NEXT:   %92 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "module has no import math.square": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB33:
;; CHECK-NEXT:   %94 = LoadPropertyInst (:any) %89: any, "__wasm_type__": string
;; CHECK-NEXT:   %95 = BinaryStrictlyEqualInst (:any) %94: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %95: any, %BB34, %BB35
;; CHECK-NEXT: %BB34:
;; CHECK-NEXT:   %97 = TypeOfInst (:string) %89: any
;; CHECK-NEXT:   %98 = BinaryStrictlyEqualInst (:any) %97: string, "function": string
;; CHECK-NEXT:         CondBranchInst %98: any, %BB36, %BB37
;; CHECK-NEXT: %BB35:
;; CHECK-NEXT:   %100 = BinaryStrictlyNotEqualInst (:any) %94: any, "func:i:i": string
;; CHECK-NEXT:          CondBranchInst %100: any, %BB37, %BB38
;; CHECK-NEXT: %BB36:
;; CHECK-NEXT:          StoreFrameInst %0: environment, %89: any, [%VS0.import_func_1]: any
;; CHECK-NEXT:   %103 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_0(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %103: object, [%VS0.closure_0]: any
;; CHECK-NEXT:   %105 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_1(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %105: object, [%VS0.closure_1]: any
;; CHECK-NEXT:   %107 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_2(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %107: object, [%VS0.closure_2]: any
;; CHECK-NEXT:   %109 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_3(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %109: object, [%VS0.closure_3]: any
;; CHECK-NEXT:   %111 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_4(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %111: object, [%VS0.closure_4]: any
;; CHECK-NEXT:   %113 = LoadFrameInst (:any) %0: environment, [%VS0.imported_mem_buf]: any
;; CHECK-NEXT:   %114 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %115 = LoadPropertyInst (:any) %114: any, "Int8Array": string
;; CHECK-NEXT:   %116 = CreateThisInst (:any) %115: any, %115: any, empty: any
;; CHECK-NEXT:   %117 = CallInst (:any) %115: any, empty: any, false: boolean, empty: any, %115: any, %116: any, %113: any
;; CHECK-NEXT:   %118 = GetConstructedObjectInst (:object) %116: any, %117: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %118: object, [%VS0.HEAP8]: any
;; CHECK-NEXT:   %120 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %121 = LoadPropertyInst (:any) %120: any, "Uint8Array": string
;; CHECK-NEXT:   %122 = CreateThisInst (:any) %121: any, %121: any, empty: any
;; CHECK-NEXT:   %123 = CallInst (:any) %121: any, empty: any, false: boolean, empty: any, %121: any, %122: any, %113: any
;; CHECK-NEXT:   %124 = GetConstructedObjectInst (:object) %122: any, %123: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %124: object, [%VS0.HEAPU8]: any
;; CHECK-NEXT:   %126 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %127 = LoadPropertyInst (:any) %126: any, "Int16Array": string
;; CHECK-NEXT:   %128 = CreateThisInst (:any) %127: any, %127: any, empty: any
;; CHECK-NEXT:   %129 = CallInst (:any) %127: any, empty: any, false: boolean, empty: any, %127: any, %128: any, %113: any
;; CHECK-NEXT:   %130 = GetConstructedObjectInst (:object) %128: any, %129: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %130: object, [%VS0.HEAP16]: any
;; CHECK-NEXT:   %132 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %133 = LoadPropertyInst (:any) %132: any, "Uint16Array": string
;; CHECK-NEXT:   %134 = CreateThisInst (:any) %133: any, %133: any, empty: any
;; CHECK-NEXT:   %135 = CallInst (:any) %133: any, empty: any, false: boolean, empty: any, %133: any, %134: any, %113: any
;; CHECK-NEXT:   %136 = GetConstructedObjectInst (:object) %134: any, %135: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %136: object, [%VS0.HEAPU16]: any
;; CHECK-NEXT:   %138 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %139 = LoadPropertyInst (:any) %138: any, "Int32Array": string
;; CHECK-NEXT:   %140 = CreateThisInst (:any) %139: any, %139: any, empty: any
;; CHECK-NEXT:   %141 = CallInst (:any) %139: any, empty: any, false: boolean, empty: any, %139: any, %140: any, %113: any
;; CHECK-NEXT:   %142 = GetConstructedObjectInst (:object) %140: any, %141: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %142: object, [%VS0.HEAP32]: any
;; CHECK-NEXT:   %144 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %145 = LoadPropertyInst (:any) %144: any, "Uint32Array": string
;; CHECK-NEXT:   %146 = CreateThisInst (:any) %145: any, %145: any, empty: any
;; CHECK-NEXT:   %147 = CallInst (:any) %145: any, empty: any, false: boolean, empty: any, %145: any, %146: any, %113: any
;; CHECK-NEXT:   %148 = GetConstructedObjectInst (:object) %146: any, %147: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %148: object, [%VS0.HEAPU32]: any
;; CHECK-NEXT:   %150 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %151 = LoadPropertyInst (:any) %150: any, "Float32Array": string
;; CHECK-NEXT:   %152 = CreateThisInst (:any) %151: any, %151: any, empty: any
;; CHECK-NEXT:   %153 = CallInst (:any) %151: any, empty: any, false: boolean, empty: any, %151: any, %152: any, %113: any
;; CHECK-NEXT:   %154 = GetConstructedObjectInst (:object) %152: any, %153: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %154: object, [%VS0.HEAPF32]: any
;; CHECK-NEXT:   %156 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %157 = LoadPropertyInst (:any) %156: any, "Float64Array": string
;; CHECK-NEXT:   %158 = CreateThisInst (:any) %157: any, %157: any, empty: any
;; CHECK-NEXT:   %159 = CallInst (:any) %157: any, empty: any, false: boolean, empty: any, %157: any, %158: any, %113: any
;; CHECK-NEXT:   %160 = GetConstructedObjectInst (:object) %158: any, %159: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %160: object, [%VS0.HEAPF64]: any
;; CHECK-NEXT:   %162 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %163 = LoadPropertyInst (:any) %162: any, "ArrayBuffer": string
;; CHECK-NEXT:   %164 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %165 = LoadPropertyInst (:any) %164: any, "Uint32Array": string
;; CHECK-NEXT:   %166 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %167 = LoadPropertyInst (:any) %166: any, "Float64Array": string
;; CHECK-NEXT:   %168 = CreateThisInst (:any) %163: any, %163: any, empty: any
;; CHECK-NEXT:   %169 = CallInst (:any) %163: any, empty: any, false: boolean, empty: any, %163: any, %168: any, 8: number
;; CHECK-NEXT:   %170 = GetConstructedObjectInst (:object) %168: any, %169: any
;; CHECK-NEXT:   %171 = CreateThisInst (:any) %165: any, %165: any, empty: any
;; CHECK-NEXT:   %172 = CallInst (:any) %165: any, empty: any, false: boolean, empty: any, %165: any, %171: any, %170: object
;; CHECK-NEXT:   %173 = GetConstructedObjectInst (:object) %171: any, %172: any
;; CHECK-NEXT:   %174 = CreateThisInst (:any) %167: any, %167: any, empty: any
;; CHECK-NEXT:   %175 = CallInst (:any) %167: any, empty: any, false: boolean, empty: any, %167: any, %174: any, %170: object
;; CHECK-NEXT:   %176 = GetConstructedObjectInst (:object) %174: any, %175: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %173: object, [%VS0.retBufI]: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %176: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %179 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %179: any, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %181 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:i": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %181: any, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:   %183 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %183: any, [%VS0.wasm_type_id_2]: any
;; CHECK-NEXT:   %185 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::i": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %185: any, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:   %187 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_0(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func:i:": string, %187: object, "__wasm_type__": string
;; CHECK-NEXT:   %189 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %190 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %191 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %187: object, %189: any, %190: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %187: object, [%VS0.exported_func_0]: any
;; CHECK-NEXT:   %193 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_funcref_1(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func:i:i": string, %193: object, "__wasm_type__": string
;; CHECK-NEXT:   %195 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %196 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:   %197 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %193: object, %195: any, %196: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %193: object, [%VS0.exported_func_1]: any
;; CHECK-NEXT:   %199 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_run(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func::i": string, %199: object, "__wasm_type__": string
;; CHECK-NEXT:   %201 = LoadFrameInst (:any) %0: environment, [%VS0.closure_3]: any
;; CHECK-NEXT:   %202 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:   %203 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %199: object, %201: any, %202: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %199: object, [%VS0.exported_func_3]: any
;; CHECK-NEXT:   %205 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_helper(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func:i:i": string, %205: object, "__wasm_type__": string
;; CHECK-NEXT:   %207 = LoadFrameInst (:any) %0: environment, [%VS0.closure_4]: any
;; CHECK-NEXT:   %208 = LoadFrameInst (:any) %0: environment, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:   %209 = CallBuiltinInst (:any) [HermesBuiltin.wasmSetFuncInfo]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %205: object, %207: any, %208: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %205: object, [%VS0.exported_func_4]: any
;; CHECK-NEXT:   %211 = LoadFrameInst (:any) %0: environment, [%VS0.import_global_val_0]: any
;; CHECK-NEXT:   %212 = AsInt32Inst (:number) %211: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %212: number, [%VS0.global_0]: any
;; CHECK-NEXT:   %214 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:          StorePropertyStrictInst "anyfunc": string, %214: object, "element": string
;; CHECK-NEXT:          StorePropertyStrictInst 4: number, %214: object, "initial": string
;; CHECK-NEXT:   %217 = LoadFrameInst (:any) %0: environment, [%VS0.intrinsics]: any
;; CHECK-NEXT:   %218 = LoadPropertyInst (:any) %217: any, "WebAssembly": string
;; CHECK-NEXT:   %219 = LoadPropertyInst (:any) %218: any, "Table": string
;; CHECK-NEXT:   %220 = CreateThisInst (:any) %219: any, %219: any, empty: any
;; CHECK-NEXT:   %221 = CallInst (:any) %219: any, empty: any, false: boolean, empty: any, %219: any, %220: any, %214: object
;; CHECK-NEXT:   %222 = GetConstructedObjectInst (:object) %220: any, %221: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %222: object, [%VS0.table_0_obj]: any
;; CHECK-NEXT:   %224 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkTable]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %222: object, true: boolean
;; CHECK-NEXT:   %225 = BinaryStrictlyEqualInst (:any) %224: any, null: null
;; CHECK-NEXT:          CondBranchInst %225: any, %BB39, %BB40
;; CHECK-NEXT: %BB37:
;; CHECK-NEXT:   %227 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "import math.square is not a function": string
;; CHECK-NEXT:          UnreachableInst
;; CHECK-NEXT: %BB38:
;; CHECK-NEXT:   %229 = TypeOfInst (:string) %89: any
;; CHECK-NEXT:   %230 = BinaryStrictlyEqualInst (:any) %229: string, "function": string
;; CHECK-NEXT:          CondBranchInst %230: any, %BB36, %BB37
;; CHECK-NEXT: %BB39:
;; CHECK-NEXT:   %232 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "WebAssembly.Table did not construct a table for this module's table 0": string
;; CHECK-NEXT:          UnreachableInst
;; CHECK-NEXT: %BB40:
;; CHECK-NEXT:   %234 = LoadPropertyInst (:any) %224: any, 0: number
;; CHECK-NEXT:   %235 = LoadPropertyInst (:any) %224: any, 1: number
;; CHECK-NEXT:   %236 = LoadPropertyInst (:any) %224: any, 2: number
;; CHECK-NEXT:   %237 = LoadPropertyInst (:any) %234: any, "length": string
;; CHECK-NEXT:   %238 = LoadPropertyInst (:any) %224: any, 3: number
;; CHECK-NEXT:   %239 = BinaryStrictlyEqualInst (:any) %237: any, 4: number
;; CHECK-NEXT:          CondBranchInst %239: any, %BB42, %BB41
;; CHECK-NEXT: %BB41:
;; CHECK-NEXT:   %241 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "WebAssembly.Table did not construct a table with this module's declared limits for table 0": string
;; CHECK-NEXT:          UnreachableInst
;; CHECK-NEXT: %BB42:
;; CHECK-NEXT:   %243 = BinaryStrictlyEqualInst (:any) %238: any, -1: number
;; CHECK-NEXT:          CondBranchInst %243: any, %BB43, %BB41
;; CHECK-NEXT: %BB43:
;; CHECK-NEXT:          StoreFrameInst %0: environment, %234: any, [%VS0.table_0_funcs]: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %235: any, [%VS0.table_0_types]: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %236: any, [%VS0.table_0_exported]: any
;; CHECK-NEXT:   %248 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:   %249 = CallInst (:any) %248: any, %wasm_func_2(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:   %250 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:   %251 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_3]: any
;; CHECK-NEXT:          StorePropertyStrictInst %251: any, %250: object, "run": string
;; CHECK-NEXT:   %253 = LoadFrameInst (:any) %0: environment, [%VS0.exported_func_4]: any
;; CHECK-NEXT:          StorePropertyStrictInst %253: any, %250: object, "helper": string
;; CHECK-NEXT:          ReturnInst %250: object
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
