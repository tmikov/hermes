;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for recursive Wasm function call.
;; factorial(n) = if n == 0 then 1 else n * factorial(n - 1)
;; Verifies CondBranchInst for the if/else, recursive CallInst
;; loading closure_0 (self), and Math.imul for the multiplication.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (func $factorial (param i32) (result i32)
    local.get 0
    i32.eqz
    if (result i32)
      i32.const 1
    else
      local.get 0
      local.get 0
      i32.const 1
      i32.sub
      call $factorial
      i32.mul
    end)
)
;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK: %BB0:
;; CHECK:   %[[SCOPE:.*]] = GetParentScopeInst (:environment)
;; CHECK:   %[[L0:.*]] = AllocStackInst (:any)
;; CHECK:   %[[P0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:            StoreStackInst %[[P0]]: any, %[[L0]]: any
;; CHECK:   %[[N:.*]] = LoadStackInst (:any) %[[L0]]: any
;; CHECK-NEXT: %[[EQZ:.*]] = BinaryStrictlyEqualInst (:any) %[[N]]: any, 0: number
;; CHECK-NEXT: %[[COND:.*]] = BinaryOrInst (:any) %[[EQZ]]: any, 0: number
;; CHECK-NEXT:                CondBranchInst %[[COND]]: any, %BB2, %BB3
;; CHECK: %BB1:
;; CHECK-NEXT: %[[RET:.*]] = PhiInst (:any) %{{.*}}: any, %BB4
;; CHECK-NEXT:               ReturnInst %[[RET]]: any
;; CHECK: %BB2:
;; CHECK-NEXT: BranchInst %BB4
;; CHECK: %BB3:
;; CHECK:   %[[N1:.*]] = LoadStackInst (:any) %[[L0]]: any
;; CHECK-NEXT: %[[N2:.*]] = LoadStackInst (:any) %[[L0]]: any
;; CHECK-NEXT: %[[SUB:.*]] = BinarySubtractInst (:any) %[[N2]]: any, 1: number
;; CHECK-NEXT: %[[TRUNC:.*]] = AsInt32Inst (:number) %[[SUB]]: any
;; CHECK-NEXT: %[[SELF:.*]] = LoadFrameInst (:any) %[[SCOPE]]: environment, [%VS0.closure_0]: any
;; CHECK-NEXT: %[[REC:.*]] = CallInst (:any) %[[SELF]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %[[TRUNC]]: number
;; CHECK-NEXT: %[[MUL:.*]] = CallBuiltinInst (:any) [Math.imul]{{.*}}, %[[N1]]: any, %[[REC]]: any
;; CHECK-NEXT:               BranchInst %BB4
;; CHECK: %BB4:
;; CHECK-NEXT: %{{.*}} = PhiInst (:any) 1: number, %BB2, %[[MUL]]: any, %BB3
;; CHECK-NEXT:           BranchInst %BB1
;; CHECK-NEXT: function_end
