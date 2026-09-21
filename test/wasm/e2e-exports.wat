;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test that exported functions get wrapper functions and are returned as
;; properties of the exports JS object. Internal (non-exported) functions
;; should not appear in the exports object.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (func $add (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
  )
  (func $helper (param i32) (result i32)
    ;; Internal function, not exported.
    local.get 0
  )
  (func $sub (export "sub") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.sub
  )
)

;; The global function builds the module info object with descriptor arrays
;; and the instantiate closure, then returns it.
;; CHECK-LABEL: function global(): object
;; CHECK:   CreateScopeInst
;; CHECK:   CreateFunctionInst {{.*}}__wasm_instantiate__
;; CHECK:   StorePropertyStrictInst {{.*}}, {{.*}}, "instantiate"
;; CHECK:   StorePropertyStrictInst {{.*}}, {{.*}}, "exportDescs"
;; CHECK:   StorePropertyStrictInst {{.*}}, {{.*}}, "importDescs"
;; CHECK-NEXT:   ReturnInst

;; The internal "add" function.
;; CHECK-LABEL: function wasm_func_0(p0: number, p1: number): number 
;; CHECK:   FAddInst
;; CHECK-NEXT:   AsInt32Inst

;; The internal helper (not exported).
;; CHECK-LABEL: function wasm_func_1(p0: number): number 

;; The internal "sub" function.
;; CHECK-LABEL: function wasm_func_2(p0: number, p1: number): number 
;; CHECK:   FSubtractInst
;; CHECK-NEXT:   AsInt32Inst

;; Export wrapper for "add": coerces args, calls internal wasm_func_0.
;; CHECK-LABEL: function wasm_export_add(p0: any, p1: any): any
;; CHECK:   GetParentScopeInst
;; CHECK-NEXT:   LoadFrameInst
;; CHECK-NEXT:   LoadParamInst
;; CHECK-NEXT:   AsInt32Inst
;; CHECK-NEXT:   LoadParamInst
;; CHECK-NEXT:   AsInt32Inst
;; CHECK-NEXT:   CallInst
;; CHECK-NEXT:   ReturnInst

;; Export wrapper for "sub": coerces args, calls internal wasm_func_2.
;; CHECK-LABEL: function wasm_export_sub(p0: any, p1: any): any
;; CHECK:   GetParentScopeInst
;; CHECK-NEXT:   LoadFrameInst
;; CHECK-NEXT:   LoadParamInst
;; CHECK-NEXT:   AsInt32Inst
;; CHECK-NEXT:   LoadParamInst
;; CHECK-NEXT:   AsInt32Inst
;; CHECK-NEXT:   CallInst
;; CHECK-NEXT:   ReturnInst
