;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for mutual recursion between two Wasm functions.
;; is_even(n) = if n == 0 then 1 else is_odd(n - 1)
;; is_odd(n)  = if n == 0 then 0 else is_even(n - 1)
;; Verifies each function loads the other's closure for the cross-call.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: is_even — calls is_odd (closure_1) in the else branch.
  ;; First function checked exhaustively including param loading.
  (func $is_even (param i32) (result i32)
    local.get 0
    i32.eqz
    if (result i32)
      i32.const 1
    else
      local.get 0
      i32.const 1
      i32.sub
      call $is_odd
    end)
;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK: %BB0:
;; CHECK:   %[[SCOPE0:.*]] = GetParentScopeInst (:environment)
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
;; CHECK:   %[[N2:.*]] = LoadStackInst (:any) %[[L0]]: any
;; CHECK-NEXT: %[[SUB:.*]] = BinarySubtractInst (:any) %[[N2]]: any, 1: number
;; CHECK-NEXT: %[[TRUNC:.*]] = AsInt32Inst (:number) %[[SUB]]: any
;; CHECK-NEXT: %[[ODD:.*]] = LoadFrameInst (:any) %[[SCOPE0]]: environment, [%VS0.closure_1]: any
;; CHECK-NEXT: %[[RES:.*]] = CallInst (:any) %[[ODD]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %[[TRUNC]]: number
;; CHECK-NEXT:               BranchInst %BB4
;; CHECK: %BB4:
;; CHECK-NEXT: %{{.*}} = PhiInst (:any) 1: number, %BB2, %[[RES]]: any, %BB3
;; CHECK-NEXT:           BranchInst %BB1
;; CHECK-NEXT: function_end

  ;; func 1: is_odd — calls is_even (closure_0) in the else branch.
  (func $is_odd (param i32) (result i32)
    local.get 0
    i32.eqz
    if (result i32)
      i32.const 0
    else
      local.get 0
      i32.const 1
      i32.sub
      call $is_even
    end)
)
;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; CHECK: %BB0:
;; CHECK:   %[[SCOPE1:.*]] = GetParentScopeInst (:environment)
;; CHECK:   %[[N1:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[EQZ1:.*]] = BinaryStrictlyEqualInst (:any) %[[N1]]: any, 0: number
;; CHECK-NEXT: %[[COND1:.*]] = BinaryOrInst (:any) %[[EQZ1]]: any, 0: number
;; CHECK-NEXT:                 CondBranchInst %[[COND1]]: any, %BB2, %BB3
;; CHECK: %BB1:
;; CHECK-NEXT: %[[RET1:.*]] = PhiInst (:any) %{{.*}}: any, %BB4
;; CHECK-NEXT:                ReturnInst %[[RET1]]: any
;; CHECK: %BB2:
;; CHECK-NEXT: BranchInst %BB4
;; CHECK: %BB3:
;; CHECK:   %[[N1B:.*]] = LoadStackInst (:any)
;; CHECK-NEXT: %[[SUB1:.*]] = BinarySubtractInst (:any) %[[N1B]]: any, 1: number
;; CHECK-NEXT: %[[TRUNC1:.*]] = AsInt32Inst (:number) %[[SUB1]]: any
;; CHECK-NEXT: %[[EVEN:.*]] = LoadFrameInst (:any) %[[SCOPE1]]: environment, [%VS0.closure_0]: any
;; CHECK-NEXT: %[[RES1:.*]] = CallInst (:any) %[[EVEN]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %[[TRUNC1]]: number
;; CHECK-NEXT:                BranchInst %BB4
;; CHECK: %BB4:
;; CHECK-NEXT: %{{.*}} = PhiInst (:any) 0: number, %BB2, %[[RES1]]: any, %BB3
;; CHECK-NEXT:           BranchInst %BB1
;; CHECK-NEXT: function_end
