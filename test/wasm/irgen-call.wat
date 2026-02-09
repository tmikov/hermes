;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for Wasm call instruction.
;; Verifies that calls load the callee closure from the parent scope
;; and pass the correct arguments via CallInst.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: returns constant 42 (no call, just branch to return).
  (func $getConst (result i32)
    i32.const 42)
;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK: %BB0:
;; CHECK:   BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI0:.*]] = PhiInst (:number) 42: number, %BB0
;; CHECK-NEXT:                ReturnInst %[[PHI0]]: number
;; CHECK-NEXT: function_end

  ;; func 1: calls $getConst (func 0) and returns its result.
  ;; First call function checked exhaustively.
  (func $callAndReturn (result i32)
    call $getConst)
;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK: %BB0:
;; CHECK:   %[[SCOPE1:.*]] = GetParentScopeInst (:environment)
;; CHECK-NEXT: %[[CALLEE1:.*]] = LoadFrameInst (:any) %[[SCOPE1]]: environment, [%VS0.closure_0]: any
;; CHECK-NEXT: %[[RES1:.*]] = CallInst (:any) %[[CALLEE1]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:                BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI1:.*]] = PhiInst (:any) %[[RES1]]: any, %BB0
;; CHECK-NEXT:                ReturnInst %[[PHI1]]: any
;; CHECK-NEXT: function_end

  ;; func 2: add(a, b) = a + b (callee for func 3).
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)
;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK:   %[[A:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[ADD:.*]] = BinaryAddInst (:any) %[[A]]: any, %[[B]]: any
;; CHECK-NEXT: %[[TRUNC:.*]] = AsInt32Inst (:number) %[[ADD]]: any
;; CHECK-NEXT:                 BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:number) %[[TRUNC]]: number, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  ;; func 3: calls $add (func 2) with constants 10, 20.
  (func $callWithArgs (result i32)
    i32.const 10
    i32.const 20
    call $add)
;; CHECK-LABEL: function wasm_func_3(): any
;; CHECK: %BB0:
;; CHECK:   %[[SCOPE3:.*]] = GetParentScopeInst (:environment)
;; CHECK-NEXT: %[[CALLEE3:.*]] = LoadFrameInst (:any) %[[SCOPE3]]: environment, [%VS0.closure_2]: any
;; CHECK-NEXT: %[[RES3:.*]] = CallInst (:any) %[[CALLEE3]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, 10: number, 20: number
;; CHECK-NEXT:                BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI3:.*]] = PhiInst (:any) %[[RES3]]: any, %BB0
;; CHECK-NEXT:                ReturnInst %[[PHI3]]: any
;; CHECK-NEXT: function_end

  ;; func 4: void callee (empty function).
  (func $voidCallee)
;; CHECK-LABEL: function wasm_func_4(): any
;; CHECK: %BB0:
;; CHECK:   BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: ReturnInst undefined: undefined
;; CHECK-NEXT: function_end

  ;; func 5: calls void callee (func 4).
  (func $callVoid
    call $voidCallee)
)
;; CHECK-LABEL: function wasm_func_5(): any
;; CHECK: %BB0:
;; CHECK:   %[[SCOPE5:.*]] = GetParentScopeInst (:environment)
;; CHECK-NEXT: %[[CALLEE5:.*]] = LoadFrameInst (:any) %[[SCOPE5]]: environment, [%VS0.closure_4]: any
;; CHECK-NEXT: %{{.*}} = CallInst (:any) %[[CALLEE5]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK-NEXT:           BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: ReturnInst undefined: undefined
;; CHECK-NEXT: function_end
