;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test that exported functions are returned as properties of a JS object.
;; The top-level function should create an object, store closures for each
;; exported function, and return the object. Internal (non-exported) functions
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

;; The global function creates closures, builds the exports object, and
;; returns it. Exports are "add" (func 0) and "sub" (func 2).
;; CHECK-LABEL: function global(): any
;; CHECK:   CreateScopeInst
;; CHECK-NEXT:   CreateFunctionInst
;; CHECK-NEXT:   StoreFrameInst
;; CHECK:   CreateFunctionInst
;; CHECK-NEXT:   StoreFrameInst
;; CHECK:   CreateFunctionInst
;; CHECK-NEXT:   StoreFrameInst
;; CHECK:   AllocObjectLiteralInst
;; CHECK-NEXT:   LoadFrameInst
;; CHECK-NEXT:   StorePropertyStrictInst {{.*}}, {{.*}}, "add"
;; CHECK-NEXT:   LoadFrameInst
;; CHECK-NEXT:   StorePropertyStrictInst {{.*}}, {{.*}}, "sub"
;; CHECK-NEXT:   ReturnInst

;; The exported "add" function.
;; CHECK-LABEL: function {{.*}}(p0: any, p1: any): any
;; CHECK:   BinaryAddInst
;; CHECK-NEXT:   AsInt32Inst

;; The internal helper (not exported).
;; CHECK-LABEL: function {{.*}}(p0: any): any

;; The exported "sub" function.
;; CHECK-LABEL: function {{.*}}(p0: any, p1: any): any
;; CHECK:   BinarySubtractInst
;; CHECK-NEXT:   AsInt32Inst
