;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir %t.wasm | %FileCheck %s
;; REQUIRES: wasm

;; Test 1: Function A calls function B which returns a constant.
;; A returns B's result.
(module
  ;; func 0: returns 42
  (func $getConst (result i32)
    i32.const 42
  )

  ;; func 1: calls $getConst and returns its result
  (func $callAndReturn (result i32)
    call $getConst
  )

  ;; func 2: calls a function with arguments
  ;; add(a, b) = a + b
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
  )

  ;; func 3: calls $add with constants
  (func $callWithArgs (result i32)
    i32.const 10
    i32.const 20
    call $add
  )

  ;; func 4: void function that calls a void function
  (func $voidCallee)
  (func $callVoid
    call $voidCallee
  )
)

;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK:   %BB0:
;; CHECK:     {{.*}}BranchInst %BB1

;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK:   %BB0:
;; CHECK:     %{{[0-9]+}} = CallInst (:any) %wasm_func_0(): functionCode, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK:     {{.*}}BranchInst %BB1

;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK:   %BB0:

;; CHECK-LABEL: function wasm_func_3(): any
;; CHECK:   %BB0:
;; CHECK:     %{{[0-9]+}} = CallInst (:any) %wasm_func_2(): functionCode, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, 10: number, 20: number
;; CHECK:     {{.*}}BranchInst %BB1

;; CHECK-LABEL: function wasm_func_4(): any
;; CHECK:   %BB0:
;; CHECK:     {{.*}}BranchInst %BB1

;; CHECK-LABEL: function wasm_func_5(): any
;; CHECK:   %BB0:
;; CHECK:     %{{[0-9]+}} = CallInst (:any) %wasm_func_4(): functionCode, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK:     {{.*}}BranchInst %BB1
