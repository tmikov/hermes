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

;; CHECK: scope %VS0 [HEAP8: any, HEAPU8: any, HEAP16: any, HEAPU16: any, HEAP32: any, HEAPU32: any, HEAPF32: any, HEAPF64: any, wasm_type_id_0: any, wasm_type_id_1: any, wasm_type_id_2: any, wasm_type_id_3: any, table_0_funcs: any, table_0_types: any, global_0: any, import_func_0: any, import_func_1: any, import_global_val_0: any, imported_mem_min: any, imported_mem_max: any, retBufI: any, retBufF: any, closure_0: any, closure_1: any, closure_2: any, closure_3: any, closure_4: any]
;; CHECK-EMPTY:
;; CHECK-NEXT: function global(): any
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
;; CHECK-NEXT: function __wasm_instantiate__(): any
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
;; CHECK-NEXT:         CondBranchInst %39: any, %BB16, %BB18
;; CHECK-NEXT: %BB15:
;; CHECK-NEXT:   %41 = PhiInst (:any) %28: any, %BB13, %28: any, %BB17, %50: any, %BB18
;; CHECK-NEXT:         StoreFrameInst %0: environment, %41: any, [%VS0.import_global_val_0]: any
;; CHECK-NEXT:   %43 = LoadPropertyInst (:any) %1: any, "env": string
;; CHECK-NEXT:   %44 = BinaryStrictlyEqualInst (:any) %43: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %44: any, %BB19, %BB20
;; CHECK-NEXT: %BB16:
;; CHECK-NEXT:   %46 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "incompatible import type": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB17:
;; CHECK-NEXT:   %48 = BinaryStrictlyEqualInst (:any) %36: string, "bigint": string
;; CHECK-NEXT:         CondBranchInst %48: any, %BB15, %BB16
;; CHECK-NEXT: %BB18:
;; CHECK-NEXT:   %50 = LoadPropertyInst (:any) %28: any, "value": string
;; CHECK-NEXT:         BranchInst %BB15
;; CHECK-NEXT: %BB19:
;; CHECK-NEXT:   %52 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import module": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB20:
;; CHECK-NEXT:   %54 = LoadPropertyInst (:any) %43: any, "memory": string
;; CHECK-NEXT:   %55 = BinaryStrictlyEqualInst (:any) %54: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %55: any, %BB21, %BB22
;; CHECK-NEXT: %BB21:
;; CHECK-NEXT:   %57 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB22:
;; CHECK-NEXT:   %59 = LoadPropertyInst (:any) %54: any, "__wasm_type__": string
;; CHECK-NEXT:   %60 = BinaryStrictlyEqualInst (:any) %59: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %60: any, %BB24, %BB23
;; CHECK-NEXT: %BB23:
;; CHECK-NEXT:   %62 = BinaryStrictlyNotEqualInst (:any) %59: any, "memory": string
;; CHECK-NEXT:         CondBranchInst %62: any, %BB24, %BB26
;; CHECK-NEXT: %BB24:
;; CHECK-NEXT:   %64 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "incompatible import type": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB25:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %72: number, [%VS0.imported_mem_min]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %74: number, [%VS0.imported_mem_max]: any
;; CHECK-NEXT:   %68 = LoadPropertyInst (:any) %1: any, "math": string
;; CHECK-NEXT:   %69 = BinaryStrictlyEqualInst (:any) %68: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %69: any, %BB29, %BB30
;; CHECK-NEXT: %BB26:
;; CHECK-NEXT:   %71 = LoadPropertyInst (:any) %54: any, "__wasm_min__": string
;; CHECK-NEXT:   %72 = AsNumberInst (:number) %71: any
;; CHECK-NEXT:   %73 = LoadPropertyInst (:any) %54: any, "__wasm_max__": string
;; CHECK-NEXT:   %74 = AsNumberInst (:number) %73: any
;; CHECK-NEXT:   %75 = BinaryGreaterThanOrEqualInst (:any) %72: number, 1: number
;; CHECK-NEXT:         CondBranchInst %75: any, %BB27, %BB24
;; CHECK-NEXT: %BB27:
;; CHECK-NEXT:   %77 = BinaryStrictlyEqualInst (:any) %74: number, -1: number
;; CHECK-NEXT:         CondBranchInst %77: any, %BB24, %BB28
;; CHECK-NEXT: %BB28:
;; CHECK-NEXT:   %79 = BinaryLessThanOrEqualInst (:any) %74: number, 10: number
;; CHECK-NEXT:         CondBranchInst %79: any, %BB25, %BB24
;; CHECK-NEXT: %BB29:
;; CHECK-NEXT:   %81 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import module": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB30:
;; CHECK-NEXT:   %83 = LoadPropertyInst (:any) %68: any, "square": string
;; CHECK-NEXT:   %84 = BinaryStrictlyEqualInst (:any) %83: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %84: any, %BB31, %BB32
;; CHECK-NEXT: %BB31:
;; CHECK-NEXT:   %86 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "unknown import": string
;; CHECK-NEXT:         UnreachableInst
;; CHECK-NEXT: %BB32:
;; CHECK-NEXT:   %88 = LoadPropertyInst (:any) %83: any, "__wasm_type__": string
;; CHECK-NEXT:   %89 = BinaryStrictlyEqualInst (:any) %88: any, undefined: undefined
;; CHECK-NEXT:         CondBranchInst %89: any, %BB33, %BB34
;; CHECK-NEXT: %BB33:
;; CHECK-NEXT:   %91 = TypeOfInst (:string) %83: any
;; CHECK-NEXT:   %92 = BinaryStrictlyEqualInst (:any) %91: string, "function": string
;; CHECK-NEXT:         CondBranchInst %92: any, %BB35, %BB36
;; CHECK-NEXT: %BB34:
;; CHECK-NEXT:   %94 = BinaryStrictlyNotEqualInst (:any) %88: any, "func:i:i": string
;; CHECK-NEXT:         CondBranchInst %94: any, %BB36, %BB35
;; CHECK-NEXT: %BB35:
;; CHECK-NEXT:         StoreFrameInst %0: environment, %83: any, [%VS0.import_func_1]: any
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
;; CHECK-NEXT:   %107 = LoadFrameInst (:any) %0: environment, [%VS0.imported_mem_min]: any
;; CHECK-NEXT:   %108 = BinaryMultiplyInst (:any) %107: any, 65536: number
;; CHECK-NEXT:   %109 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:   %110 = CreateThisInst (:any) %109: any, %109: any, empty: any
;; CHECK-NEXT:   %111 = CallInst (:any) %109: any, empty: any, false: boolean, empty: any, %109: any, %110: any, %108: any
;; CHECK-NEXT:   %112 = GetConstructedObjectInst (:object) %110: any, %111: any
;; CHECK-NEXT:   %113 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int8Array": string
;; CHECK-NEXT:   %114 = CreateThisInst (:any) %113: any, %113: any, empty: any
;; CHECK-NEXT:   %115 = CallInst (:any) %113: any, empty: any, false: boolean, empty: any, %113: any, %114: any, %112: object
;; CHECK-NEXT:   %116 = GetConstructedObjectInst (:object) %114: any, %115: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %116: object, [%VS0.HEAP8]: any
;; CHECK-NEXT:   %118 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint8Array": string
;; CHECK-NEXT:   %119 = CreateThisInst (:any) %118: any, %118: any, empty: any
;; CHECK-NEXT:   %120 = CallInst (:any) %118: any, empty: any, false: boolean, empty: any, %118: any, %119: any, %112: object
;; CHECK-NEXT:   %121 = GetConstructedObjectInst (:object) %119: any, %120: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %121: object, [%VS0.HEAPU8]: any
;; CHECK-NEXT:   %123 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int16Array": string
;; CHECK-NEXT:   %124 = CreateThisInst (:any) %123: any, %123: any, empty: any
;; CHECK-NEXT:   %125 = CallInst (:any) %123: any, empty: any, false: boolean, empty: any, %123: any, %124: any, %112: object
;; CHECK-NEXT:   %126 = GetConstructedObjectInst (:object) %124: any, %125: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %126: object, [%VS0.HEAP16]: any
;; CHECK-NEXT:   %128 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint16Array": string
;; CHECK-NEXT:   %129 = CreateThisInst (:any) %128: any, %128: any, empty: any
;; CHECK-NEXT:   %130 = CallInst (:any) %128: any, empty: any, false: boolean, empty: any, %128: any, %129: any, %112: object
;; CHECK-NEXT:   %131 = GetConstructedObjectInst (:object) %129: any, %130: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %131: object, [%VS0.HEAPU16]: any
;; CHECK-NEXT:   %133 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Int32Array": string
;; CHECK-NEXT:   %134 = CreateThisInst (:any) %133: any, %133: any, empty: any
;; CHECK-NEXT:   %135 = CallInst (:any) %133: any, empty: any, false: boolean, empty: any, %133: any, %134: any, %112: object
;; CHECK-NEXT:   %136 = GetConstructedObjectInst (:object) %134: any, %135: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %136: object, [%VS0.HEAP32]: any
;; CHECK-NEXT:   %138 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:   %139 = CreateThisInst (:any) %138: any, %138: any, empty: any
;; CHECK-NEXT:   %140 = CallInst (:any) %138: any, empty: any, false: boolean, empty: any, %138: any, %139: any, %112: object
;; CHECK-NEXT:   %141 = GetConstructedObjectInst (:object) %139: any, %140: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %141: object, [%VS0.HEAPU32]: any
;; CHECK-NEXT:   %143 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float32Array": string
;; CHECK-NEXT:   %144 = CreateThisInst (:any) %143: any, %143: any, empty: any
;; CHECK-NEXT:   %145 = CallInst (:any) %143: any, empty: any, false: boolean, empty: any, %143: any, %144: any, %112: object
;; CHECK-NEXT:   %146 = GetConstructedObjectInst (:object) %144: any, %145: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %146: object, [%VS0.HEAPF32]: any
;; CHECK-NEXT:   %148 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:   %149 = CreateThisInst (:any) %148: any, %148: any, empty: any
;; CHECK-NEXT:   %150 = CallInst (:any) %148: any, empty: any, false: boolean, empty: any, %148: any, %149: any, %112: object
;; CHECK-NEXT:   %151 = GetConstructedObjectInst (:object) %149: any, %150: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %151: object, [%VS0.HEAPF64]: any
;; CHECK-NEXT:   %153 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:   %154 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:   %155 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:   %156 = CreateThisInst (:any) %153: any, %153: any, empty: any
;; CHECK-NEXT:   %157 = CallInst (:any) %153: any, empty: any, false: boolean, empty: any, %153: any, %156: any, 8: number
;; CHECK-NEXT:   %158 = GetConstructedObjectInst (:object) %156: any, %157: any
;; CHECK-NEXT:   %159 = CreateThisInst (:any) %154: any, %154: any, empty: any
;; CHECK-NEXT:   %160 = CallInst (:any) %154: any, empty: any, false: boolean, empty: any, %154: any, %159: any, %158: object
;; CHECK-NEXT:   %161 = GetConstructedObjectInst (:object) %159: any, %160: any
;; CHECK-NEXT:   %162 = CreateThisInst (:any) %155: any, %155: any, empty: any
;; CHECK-NEXT:   %163 = CallInst (:any) %155: any, empty: any, false: boolean, empty: any, %155: any, %162: any, %158: object
;; CHECK-NEXT:   %164 = GetConstructedObjectInst (:object) %162: any, %163: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %161: object, [%VS0.retBufI]: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %164: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %167 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %167: any, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT:   %169 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func:i:i": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %169: any, [%VS0.wasm_type_id_1]: any
;; CHECK-NEXT:   %171 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %171: any, [%VS0.wasm_type_id_2]: any
;; CHECK-NEXT:   %173 = CallBuiltinInst (:any) [HermesBuiltin.wasmInternType]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "func::i": string
;; CHECK-NEXT:          StoreFrameInst %0: environment, %173: any, [%VS0.wasm_type_id_3]: any
;; CHECK-NEXT:   %175 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK-NEXT:   %176 = CreateThisInst (:any) %175: any, %175: any, empty: any
;; CHECK-NEXT:   %177 = CallInst (:any) %175: any, empty: any, false: boolean, empty: any, %175: any, %176: any, 4: number
;; CHECK-NEXT:   %178 = GetConstructedObjectInst (:object) %176: any, %177: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %178: object, [%VS0.table_0_funcs]: any
;; CHECK-NEXT:   %180 = CreateThisInst (:any) %175: any, %175: any, empty: any
;; CHECK-NEXT:   %181 = CallInst (:any) %175: any, empty: any, false: boolean, empty: any, %175: any, %180: any, 4: number
;; CHECK-NEXT:   %182 = GetConstructedObjectInst (:object) %180: any, %181: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %182: object, [%VS0.table_0_types]: any
;; CHECK-NEXT:   %184 = LoadFrameInst (:any) %0: environment, [%VS0.import_global_val_0]: any
;; CHECK-NEXT:   %185 = AsInt32Inst (:number) %184: any
;; CHECK-NEXT:          StoreFrameInst %0: environment, %185: number, [%VS0.global_0]: any
;; CHECK-NEXT:   %187 = LoadFrameInst (:any) %0: environment, [%VS0.closure_2]: any
;; CHECK-NEXT:   %188 = CallInst (:any) %187: any, %wasm_func_2(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:   %189 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:   %190 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_run(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func::i": string, %190: object, "__wasm_type__": string
;; CHECK-NEXT:          StorePropertyStrictInst %190: object, %189: object, "run": string
;; CHECK-NEXT:   %193 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_export_helper(): functionCode
;; CHECK-NEXT:          StorePropertyStrictInst "func:i:i": string, %193: object, "__wasm_type__": string
;; CHECK-NEXT:          StorePropertyStrictInst %193: object, %189: object, "helper": string
;; CHECK-NEXT:          ReturnInst %189: object
;; CHECK-NEXT: %BB36:
;; CHECK-NEXT:   %197 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkError]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, "incompatible import type": string
;; CHECK-NEXT:          UnreachableInst
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
