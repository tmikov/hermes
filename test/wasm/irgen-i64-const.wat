;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: i64 constant split into lo32/hi32 pair.
;; Phase 1 represents i64 as two values on the stack (lo, hi).
;; i64 function results use the hi-stash pattern: stash hi, return lo.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; i64.const 0x0000000100000002 = 4294967298
  ;; Split: lo32 = 2, hi32 = 1
  (func (result i64)
    i64.const 4294967298))

;; CHECK: scope %VS0 [wasm_type_id_0: any, retBufI: any, retBufF: any, closure_0: any]
;; CHECK-EMPTY:
;; CHECK-NEXT: function global(): object 
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
;; CHECK-NEXT: function wasm_func_0(retbuf_I: object, retbuf_F: object): number 
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; CHECK-NEXT:   %1 = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT:   %2 = LoadParamInst (:object) %retbuf_F: object
;; CHECK-NEXT:        BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %4 = PhiInst (:number) 2: number, %BB0
;; CHECK-NEXT:   %5 = PhiInst (:number) 1: number, %BB0
;; CHECK-NEXT:        StorePropertyStrictInst %4: number, %1: object, 0: number
;; CHECK-NEXT:        StorePropertyStrictInst %5: number, %1: object, 1: number
;; CHECK-NEXT:        ReturnInst 0: number
;; CHECK-NEXT: function_end
;; CHECK-EMPTY:
;; CHECK-NEXT: function __wasm_instantiate__(imports: any): object 
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:   %0 = CreateScopeInst (:environment) %VS0: any, empty: any
;; CHECK-NEXT:   %1 = CreateFunctionInst (:object) %0: environment, %VS0: any, %wasm_func_0(): functionCode
;; CHECK-NEXT:        StoreFrameInst %0: environment, %1: object, [%VS0.closure_0]: any
;; CHECK-NEXT:   %3 = TryLoadGlobalPropertyInst (:any) globalObject: object, "ArrayBuffer": string
;; CHECK-NEXT:   %4 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint32Array": string
;; CHECK-NEXT:   %5 = TryLoadGlobalPropertyInst (:any) globalObject: object, "Float64Array": string
;; CHECK-NEXT:   %6 = CreateThisInst (:any) %3: any, %3: any, empty: any
;; CHECK-NEXT:   %7 = CallInst (:any) %3: any, empty: any, false: boolean, empty: any, %3: any, %6: any, 8: number
;; CHECK-NEXT:   %8 = GetConstructedObjectInst (:object) %6: any, %7: any
;; CHECK-NEXT:   %9 = CreateThisInst (:any) %4: any, %4: any, empty: any
;; CHECK-NEXT:   %10 = CallInst (:any) %4: any, empty: any, false: boolean, empty: any, %4: any, %9: any, %8: object
;; CHECK-NEXT:   %11 = GetConstructedObjectInst (:object) %9: any, %10: any
;; CHECK-NEXT:   %12 = CreateThisInst (:any) %5: any, %5: any, empty: any
;; CHECK-NEXT:   %13 = CallInst (:any) %5: any, empty: any, false: boolean, empty: any, %5: any, %12: any, %8: object
;; CHECK-NEXT:   %14 = GetConstructedObjectInst (:object) %12: any, %13: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %11: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %14: object, [%VS0.retBufF]: any
;; CHECK-NEXT:   %17 = AllocObjectLiteralInst (:object) empty: any
;; CHECK-NEXT:         ReturnInst %17: object
;; CHECK-NEXT: function_end
