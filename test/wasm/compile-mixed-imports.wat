;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test module with mixed imports (functions, table, memory, global)
;; from different modules, and a start function.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

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

;; CHECK: scope %VS0 [HEAP8: any, HEAPU8: any, HEAP16: any, HEAPU16: any, HEAP32: any, HEAPU32: any, HEAPF32: any, HEAPF64: any, table_0_funcs: any, table_0_types: any, global_0: any, import_func_0: any, import_func_1: any, import_global_val_0: any, imported_mem_min: any, imported_mem_max: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any, closure_2: any, closure_3: any, closure_4: any]
;; CHECK-EMPTY:
;; CHECK-NEXT: function global(): object
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:   %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %__wasm_instantiate__(): functionCode
;; CHECK-NEXT:   %2 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK-NEXT:   %3 = CreateThisInst (:any) %2: any, %2: any, empty: any
;; CHECK-NEXT:   %4 = CallInst (:any) %2: any, empty: any, false: boolean, empty: any, %2: any, %3: any, 2: number
;; CHECK-NEXT:   %5 = GetConstructedObjectInst (:object) %3: any, %4: any
;; CHECK-NEXT:   %6 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:        StorePropertyStrictInst "run": string, %6: object, "name": string
;; CHECK-NEXT:        StorePropertyStrictInst "function": string, %6: object, "kind": string
;; CHECK-NEXT:        StorePropertyStrictInst %6: object, %5: object, 0: number
;; CHECK-NEXT:   %10 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "helper": string, %10: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %10: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %10: object, %5: object, 1: number
;; CHECK-NEXT:   %14 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK-NEXT:   %15 = CreateThisInst (:any) %14: any, %14: any, empty: any
;; CHECK-NEXT:   %16 = CallInst (:any) %14: any, empty: any, false: boolean, empty: any, %14: any, %15: any, 4: number
;; CHECK-NEXT:   %17 = GetConstructedObjectInst (:object) %15: any, %16: any
;; CHECK-NEXT:   %18 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "env": string, %18: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "log": string, %18: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %18: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %18: object, %17: object, 0: number
;; CHECK-NEXT:   %23 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "config": string, %23: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "max_size": string, %23: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "global": string, %23: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %23: object, %17: object, 1: number
;; CHECK-NEXT:   %28 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "env": string, %28: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "memory": string, %28: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "memory": string, %28: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %28: object, %17: object, 2: number
;; CHECK-NEXT:   %33 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst "math": string, %33: object, "module": string
;; CHECK-NEXT:         StorePropertyStrictInst "square": string, %33: object, "name": string
;; CHECK-NEXT:         StorePropertyStrictInst "function": string, %33: object, "kind": string
;; CHECK-NEXT:         StorePropertyStrictInst %33: object, %17: object, 3: number
;; CHECK-NEXT:   %38 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         StorePropertyStrictInst %1: object, %38: object, "instantiate": string
;; CHECK-NEXT:         StorePropertyStrictInst %5: object, %38: object, "exportDescs": string
;; CHECK-NEXT:         StorePropertyStrictInst %17: object, %38: object, "importDescs": string
;; CHECK-NEXT:         ReturnInst %38: object
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
;; CHECK-NEXT: function wasm_func_1(p0: any): any 
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.import_func_1]: any
;; CHECK-NEXT:   %2 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:   %3 = CallInst (:any) %1: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %2: any
;; CHECK-NEXT:   %4 = AsInt32Inst (:number) %3: any
;; CHECK-NEXT:        ReturnInst %4: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_2(): any 
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = LoadFrameInst (:any) %0: environment, [%VS0.closure_0]: any
;; CHECK-NEXT:   %3 = CallInst (:any) %2: any, %wasm_func_0(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, 0: number
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:        ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function wasm_func_3(): any 
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
;; CHECK-NEXT: function wasm_func_4(p0: any): any 
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; CHECK-NEXT:   %2 = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT:   %3 = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:        StoreStackInst %3: any, %2: any
;; CHECK-NEXT:   %5 = LoadStackInst (:any) %2: any
;; CHECK-NEXT:   %6 = LoadFrameInst (:any) %0: environment, [%VS0.closure_1]: any
;; CHECK-NEXT:   %7 = CallInst (:any) %6: any, %wasm_func_1(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %5: any
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %9 = PhiInst (:any) %7: any, %BB0
;; CHECK-NEXT:         ReturnInst %9: any
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function __wasm_instantiate__(): object
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
;; CHECK-NEXT:   %21 = LoadPropertyInst (:any) %1: any, "config": string
;; CHECK-NEXT:   %22 = BinaryStrictlyEqualInst (:any) %21: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %22: any, %BB9, %BB10
;; CHECK-NEXT: %BB8:
;; CHECK-NEXT:   %24 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "incompatible import type": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB9:
;; CHECK-NEXT:   %26 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import module": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB10:
;; CHECK-NEXT:   %28 = LoadPropertyInst (:any) %21: any, "max_size": string
;; CHECK-NEXT:   %29 = BinaryStrictlyEqualInst (:any) %28: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %29: any, %BB11, %BB12
;; CHECK-NEXT: %BB11:
;; CHECK-NEXT:   %31 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB12:
;; CHECK-NEXT:   %33 = LoadPropertyInst (:any) %28: any, "__wasm_type__": string
;; CHECK-NEXT:   %34 = BinaryStrictlyEqualInst (:any) %33: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %34: any, %BB13, %BB14
;; CHECK-NEXT: %BB13:
;; CHECK-NEXT:   %36 = TypeOfInst (:string) %28: any
;; CHECK-NEXT:   %37 = BinaryStrictlyEqualInst (:any) %36: string, "number": string
;; CHECK-NEXT:         CondBranchInst %37: any, %BB15, %BB17
;; CHECK-NEXT: %BB14:
;; CHECK-NEXT:   %39 = BinaryStrictlyNotEqualInst (:any) %33: any, "global:i32:const": string
;; CHECK-NEXT:         CondBranchInst %39: any, %BB16, %BB15
;; CHECK-NEXT: %BB15:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %28: any, [%VS0.import_global_val_0]: any
;; CHECK-NEXT:   %42 = LoadPropertyInst (:any) %1: any, "env": string
;; CHECK-NEXT:   %43 = BinaryStrictlyEqualInst (:any) %42: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %43: any, %BB18, %BB19
;; CHECK-NEXT: %BB16:
;; CHECK-NEXT:   %45 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "incompatible import type": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB17:
;; CHECK-NEXT:   %47 = BinaryStrictlyEqualInst (:any) %36: string, "bigint": string
;; CHECK-NEXT:         CondBranchInst %47: any, %BB15, %BB16
;; CHECK-NEXT: %BB18:
;; CHECK-NEXT:   %49 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import module": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB19:
;; CHECK-NEXT:   %51 = LoadPropertyInst (:any) %42: any, "memory": string
;; CHECK-NEXT:   %52 = BinaryStrictlyEqualInst (:any) %51: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %52: any, %BB20, %BB21
;; CHECK-NEXT: %BB20:
;; CHECK-NEXT:   %54 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB21:
;; CHECK-NEXT:   %56 = LoadPropertyInst (:any) %51: any, "__wasm_type__": string
;; CHECK-NEXT:   %57 = BinaryStrictlyEqualInst (:any) %56: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %57: any, %BB23, %BB22
;; CHECK-NEXT: %BB22:
;; CHECK-NEXT:   %59 = BinaryStrictlyNotEqualInst (:any) %56: any, "memory": string
;; CHECK-NEXT:         CondBranchInst %59: any, %BB23, %BB25
;; CHECK-NEXT: %BB23:
;; CHECK-NEXT:   %61 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "incompatible import type": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB24:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %69: any, [%VS0.imported_mem_min]: any
;; CHECK-NEXT:   %64 = LoadPropertyInst (:any) %51: any, "__wasm_max__": string
;; CHECK-NEXT:         StoreFrameInst %0: environment, %64: any, [%VS0.imported_mem_max]: any
;; CHECK-NEXT:   %66 = LoadPropertyInst (:any) %1: any, "math": string
;; CHECK-NEXT:   %67 = BinaryStrictlyEqualInst (:any) %66: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %67: any, %BB28, %BB29
;; CHECK-NEXT: %BB25:
;; CHECK-NEXT:   %69 = LoadPropertyInst (:any) %51: any, "__wasm_min__": string
;; CHECK-NEXT:   %70 = BinaryGreaterThanOrEqualInst (:any) %69: any, 1: number
;; CHECK-NEXT:         CondBranchInst %70: any, %BB26, %BB23
;; CHECK-NEXT: %BB26:
;; CHECK-NEXT:   %72 = LoadPropertyInst (:any) %51: any, "__wasm_max__": string
;; CHECK-NEXT:   %73 = BinaryStrictlyEqualInst (:any) %72: any, -1: number
;; CHECK-NEXT:         CondBranchInst %73: any, %BB23, %BB27
;; CHECK-NEXT: %BB27:
;; CHECK-NEXT:   %75 = BinaryLessThanOrEqualInst (:any) %72: any, 10: number
;; CHECK-NEXT:         CondBranchInst %75: any, %BB24, %BB23
;; CHECK-NEXT: %BB28:
;; CHECK-NEXT:   %77 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import module": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB29:
;; CHECK-NEXT:   %79 = LoadPropertyInst (:any) %66: any, "square": string
;; CHECK-NEXT:   %80 = BinaryStrictlyEqualInst (:any) %79: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %80: any, %BB30, %BB31
;; CHECK-NEXT: %BB30:
;; CHECK-NEXT:   %82 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB31:
;; CHECK-NEXT:   %84 = LoadPropertyInst (:any) %79: any, "__wasm_type__": string
;; CHECK-NEXT:   %85 = BinaryStrictlyEqualInst (:any) %84: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %85: any, %BB32, %BB33
;; CHECK-NEXT: %BB32:
;; CHECK-NEXT:   %87 = TypeOfInst (:string) %79: any
;; CHECK-NEXT:   %88 = BinaryStrictlyEqualInst (:any) %87: string, "function": string
;; CHECK-NEXT:         CondBranchInst %88: any, %BB34, %BB35
;; CHECK-NEXT: %BB33:
;; CHECK-NEXT:   %90 = BinaryStrictlyNotEqualInst (:any) %84: any, "func:i:i": string
;; CHECK-NEXT:         CondBranchInst %90: any, %BB35, %BB34
;; CHECK-NEXT: %BB34:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %79: any, [%VS0.import_func_1]: any
;; CHECK-NEXT:   %93 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_0(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %93: object, [%VS0.closure_0]: any
;; CHECK-NEXT:   %95 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_1(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %95: object, [%VS0.closure_1]: any
;; CHECK-NEXT:   %97 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_2(): functionCode
;; CHECK-NEXT:         StoreFrameInst %0: environment, %97: object, [%VS0.closure_2]: any
;; CHECK-NEXT:   %99 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_3(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %99: object, [%VS0.closure_3]: any
;; CHECK-NEXT:   %101 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_4(): functionCode
;; CHECK-NEXT:          StoreFrameInst %0: environment, %101: object, [%VS0.closure_4]: any
;; CHECK-NEXT:   %103 = LoadFrameInst (:any) %0: environment, [%VS0.imported_mem_min]: any
;; CHECK-NEXT:   %104 = BinaryMultiplyInst (:any) %103: any, 65536: number
;; CHECK-NEXT:   %105 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:   %106 = CreateThisInst (:any) %105: any, %105: any, empty: any
;; CHECK-NEXT:   %107 = CallInst (:any) %105: any, empty: any, false: boolean, empty: any, %105: any, %106: any, %104: any
;; CHECK-NEXT:   %108 = GetConstructedObjectInst (:object) %106: any, %107: any
;; CHECK-NEXT:   %109 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int8Array": string
;; CHECK-NEXT:   %110 = CreateThisInst (:any) %109: any, %109: any, empty: any
;; CHECK-NEXT:   %111 = CallInst (:any) %109: any, empty: any, false: boolean, empty: any, %109: any, %110: any, %108: object
;; CHECK-NEXT:   %112 = GetConstructedObjectInst (:object) %110: any, %111: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %112: object, [%VS0.HEAP8]: any
;; CHECK-NEXT:   %114 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint8Array": string
;; CHECK-NEXT:   %115 = CreateThisInst (:any) %114: any, %114: any, empty: any
;; CHECK-NEXT:   %116 = CallInst (:any) %114: any, empty: any, false: boolean, empty: any, %114: any, %115: any, %108: object
;; CHECK-NEXT:   %117 = GetConstructedObjectInst (:object) %115: any, %116: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %117: object, [%VS0.HEAPU8]: any
;; CHECK-NEXT:   %119 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int16Array": string
;; CHECK-NEXT:   %120 = CreateThisInst (:any) %119: any, %119: any, empty: any
;; CHECK-NEXT:   %121 = CallInst (:any) %119: any, empty: any, false: boolean, empty: any, %119: any, %120: any, %108: object
;; CHECK-NEXT:   %122 = GetConstructedObjectInst (:object) %120: any, %121: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %122: object, [%VS0.HEAP16]: any
;; CHECK-NEXT:   %124 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint16Array": string
;; CHECK-NEXT:   %125 = CreateThisInst (:any) %124: any, %124: any, empty: any
;; CHECK-NEXT:   %126 = CallInst (:any) %124: any, empty: any, false: boolean, empty: any, %124: any, %125: any, %108: object
;; CHECK-NEXT:   %127 = GetConstructedObjectInst (:object) %125: any, %126: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %127: object, [%VS0.HEAPU16]: any
;; CHECK-NEXT:   %129 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int32Array": string
;; CHECK-NEXT:   %130 = CreateThisInst (:any) %129: any, %129: any, empty: any
;; CHECK-NEXT:   %131 = CallInst (:any) %129: any, empty: any, false: boolean, empty: any, %129: any, %130: any, %108: object
;; CHECK-NEXT:   %132 = GetConstructedObjectInst (:object) %130: any, %131: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %132: object, [%VS0.HEAP32]: any
;; CHECK-NEXT:   %134 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:   %135 = CreateThisInst (:any) %134: any, %134: any, empty: any
;; CHECK-NEXT:   %136 = CallInst (:any) %134: any, empty: any, false: boolean, empty: any, %134: any, %135: any, %108: object
;; CHECK-NEXT:   %137 = GetConstructedObjectInst (:object) %135: any, %136: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %137: object, [%VS0.HEAPU32]: any
;; CHECK-NEXT:   %139 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float32Array": string
;; CHECK-NEXT:   %140 = CreateThisInst (:any) %139: any, %139: any, empty: any
;; CHECK-NEXT:   %141 = CallInst (:any) %139: any, empty: any, false: boolean, empty: any, %139: any, %140: any, %108: object
;; CHECK-NEXT:   %142 = GetConstructedObjectInst (:object) %140: any, %141: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %142: object, [%VS0.HEAPF32]: any
;; CHECK-NEXT:   %144 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:   %145 = CreateThisInst (:any) %144: any, %144: any, empty: any
;; CHECK-NEXT:   %146 = CallInst (:any) %144: any, empty: any, false: boolean, empty: any, %144: any, %145: any, %108: object
;; CHECK-NEXT:   %147 = GetConstructedObjectInst (:object) %145: any, %146: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %147: object, [%VS0.HEAPF64]: any
;; CHECK-NEXT:   %149 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:   %150 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:   %151 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:   %152 = CreateThisInst (:any) %149: any, %149: any, empty: any
;; CHECK-NEXT:   %153 = CallInst (:any) %149: any, empty: any, false: boolean, empty: any, %149: any, %152: any, 8: number
;; CHECK-NEXT:   %154 = GetConstructedObjectInst (:object) %152: any, %153: any
;; CHECK-NEXT:   %155 = CreateThisInst (:any) %150: any, %150: any, empty: any
;; CHECK-NEXT:   %156 = CallInst (:any) %150: any, empty: any, false: boolean, empty: any, %150: any, %155: any, %154: object
;; CHECK-NEXT:   %157 = GetConstructedObjectInst (:object) %155: any, %156: any
;; CHECK-NEXT:   %158 = CreateThisInst (:any) %151: any, %151: any, empty: any
;; CHECK-NEXT:   %159 = CallInst (:any) %151: any, empty: any, false: boolean, empty: any, %151: any, %158: any, %154: object
;; CHECK-NEXT:   %160 = GetConstructedObjectInst (:object) %158: any, %159: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %157: object, [%VS0.retBufI]: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %160: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %163 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK-NEXT:   %164 = CreateThisInst (:any) %163: any, %163: any, empty: any
;; CHECK-NEXT:   %165 = CallInst (:any) %163: any, empty: any, false: boolean, empty: any, %163: any, %164: any, 4: number
;; CHECK-NEXT:   %166 = GetConstructedObjectInst (:object) %164: any, %165: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %166: object, [%VS0.table_0_funcs]: any
;; CHECK-NEXT:   %168 = CreateThisInst (:any) %163: any, %163: any, empty: any
;; CHECK-NEXT:   %169 = CallInst (:any) %163: any, empty: any, false: boolean, empty: any, %163: any, %168: any, 4: number
;; CHECK-NEXT:   %170 = GetConstructedObjectInst (:object) %168: any, %169: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %170: object, [%VS0.table_0_types]: any
;; CHECK-NEXT:   %172 = LoadFrameInst (:any) %0: environment, [%VS0.import_global_val_0]: any
;; CHECK-NEXT:   %173 = LoadPropertyInst (:any) %172: any, "__wasm_type__": string
;; CHECK-NEXT:   %174 = BinaryStrictlyEqualInst (:any) %173: any, undefined: undefined
;; CHECK-NEXT:          CondBranchInst %174: any, %BB36, %BB37
;; CHECK-NEXT: %BB35:
;; CHECK-NEXT:   %176 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "incompatible import type": string
;; CHECK-NEXT:          UnreachableInst
;; CHECK-NEXT: %BB36:
;; CHECK-NEXT:          BranchInst %BB38
;; CHECK-NEXT: %BB37:
;; CHECK-NEXT:   %179 = LoadPropertyInst (:any) %172: any, "value": string
;; CHECK-NEXT:          BranchInst %BB38
;; CHECK-NEXT: %BB38:
;; CHECK-NEXT:   %181 = PhiInst (:any) %172: any, %BB36, %179: any, %BB37
;; CHECK-NEXT:          StoreFrameInst %0: environment, %181: any, [%VS0.global_0]: any
;; CHECK-NEXT:   %183 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:   %184 = CallInst (:any) %183: any, %wasm_func_2(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:   %185 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:   %186 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_run(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func::i": string, %186: object, "__wasm_type__": string
;; CHECK-NEXT:          StorePropertyStrictInst %186: object, %185: object, "run": string
;; CHECK-NEXT:   %189 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_helper(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func:i:i": string, %189: object, "__wasm_type__": string
;; CHECK-NEXT:          StorePropertyStrictInst %189: object, %185: object, "helper": string
;; CHECK-NEXT:          ReturnInst %185: object
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
