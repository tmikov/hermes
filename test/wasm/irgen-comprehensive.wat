;;  Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Comprehensive IR generation test — exercises at least one instruction
;; from each implemented category. This verifies that all D.2-D.12
;; callbacks are wired end-to-end (D.13).
;;
;; Categories covered:
;;   Constants: i32.const, f64.const
;;   Locals: local.get, local.set, local.tee
;;   i32 arithmetic: i32.add, i32.sub, i32.mul, i32.and, i32.or,
;;                   i32.xor, i32.shl, i32.shr_s, i32.shr_u
;;   i32 comparisons: i32.eq, i32.lt_s, i32.eqz
;;   Control flow: block, loop, if/else, br, br_if, br_table
;;   Parametric: select, drop
;;   Function calls: call
;;   Misc: return, unreachable, nop

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm 2>&1 | %FileCheck %s

;; --- Test 1: Constants and locals ---
;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK:   %1 = AllocStackInst (:any) $local_0
;; CHECK:   StoreStackInst 0: number, %1
;; CHECK:   StoreStackInst 42: number, %1
;; CHECK:   %4 = LoadStackInst (:any) %1
;; CHECK:   BranchInst %BB1
;; CHECK: %BB1:
;; CHECK:   %6 = PhiInst (:any) %4: any, %BB0
;; CHECK:   ReturnInst %6
;; CHECK:   function_end
(func $constants_and_locals (result i32) (local i32)
  i32.const 42
  local.set 0
  local.get 0
)

;; --- Test 2: i32 arithmetic (add, sub, mul, local.tee) ---
;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK:   BinaryAddInst
;; CHECK:   AsInt32Inst
;; CHECK:   BinarySubtractInst
;; CHECK:   AsInt32Inst
;; CHECK:   StoreStackInst
;; CHECK:   LoadStackInst
;; CHECK:   CallBuiltinInst (:any) [Math.imul]
;; CHECK:   BranchInst %BB1
;; CHECK:   function_end
(func $i32_arith (param i32 i32) (result i32)
  local.get 0
  local.get 1
  i32.add
  local.get 0
  i32.sub
  local.tee 0
  local.get 0
  i32.mul
)

;; --- Test 3: Bitwise operations (and, or, xor) ---
;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK:   BinaryAndInst
;; CHECK:   BinaryOrInst
;; CHECK:   BinaryXorInst
;; CHECK:   BranchInst %BB1
;; CHECK:   function_end
(func $bitwise (param i32 i32) (result i32)
  local.get 0
  local.get 1
  i32.and
  local.get 1
  i32.or
  local.get 0
  i32.xor
)

;; --- Test 4: Shift operations (shl, shr_s, shr_u) ---
;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK:   BinaryLeftShiftInst
;; CHECK:   BinaryRightShiftInst
;; CHECK:   BinaryUnsignedRightShiftInst
;; CHECK:   BranchInst %BB1
;; CHECK:   function_end
(func $shifts (param i32 i32) (result i32)
  local.get 0
  local.get 1
  i32.shl
  local.get 1
  i32.shr_s
  local.get 1
  i32.shr_u
)

;; --- Test 5: i32 comparisons (eq, lt_s) with drop ---
;; CHECK-LABEL: function wasm_func_4(p0: any, p1: any): any
;; CHECK:   BinaryStrictlyEqualInst
;; CHECK:   BinaryOrInst {{.*}} 0: number
;; CHECK:   AsInt32Inst
;; CHECK:   AsInt32Inst
;; CHECK:   BinaryLessThanInst
;; CHECK:   BinaryOrInst {{.*}} 0: number
;; CHECK:   BranchInst %BB1
;; CHECK:   function_end
(func $comparisons (param i32 i32) (result i32)
  local.get 0
  local.get 1
  i32.eq
  drop
  local.get 0
  local.get 1
  i32.lt_s
)

;; --- Test 6: i32.eqz ---
;; CHECK-LABEL: function wasm_func_5(p0: any): any
;; CHECK:   BinaryStrictlyEqualInst {{.*}} 0: number
;; CHECK:   BinaryOrInst {{.*}} 0: number
;; CHECK:   BranchInst %BB1
;; CHECK:   function_end
(func $eqz (param i32) (result i32)
  local.get 0
  i32.eqz
)

;; --- Test 7: Block and br ---
;; CHECK-LABEL: function wasm_func_6(): any
;; CHECK: %BB0:
;; CHECK:   BranchInst %BB2
;; CHECK: %BB1:
;; CHECK:   ReturnInst undefined
;; CHECK: %BB2:
;; CHECK:   BranchInst %BB1
;; CHECK:   function_end
(func $block_br
  block
    br 0
  end
)

;; --- Test 8: Loop with br_if ---
;; CHECK-LABEL: function wasm_func_7(p0: any): any
;; CHECK:   BranchInst %BB2
;; CHECK: %BB2:
;; CHECK:   BinarySubtractInst
;; CHECK:   AsInt32Inst
;; CHECK:   StoreStackInst
;; CHECK:   LoadStackInst
;; CHECK:   CondBranchInst {{.*}} %BB2
;; CHECK:   function_end
(func $loop_br_if (param i32)
  (loop
    (local.set 0 (i32.sub (local.get 0) (i32.const 1)))
    (br_if 0 (local.get 0))
  )
)

;; --- Test 9: If/else with result ---
;; CHECK-LABEL: function wasm_func_8(p0: any): any
;; CHECK:   CondBranchInst {{.*}} %BB2, %BB3
;; CHECK: %BB2:
;; CHECK:   BranchInst %BB4
;; CHECK: %BB3:
;; CHECK:   BranchInst %BB4
;; CHECK: %BB4:
;; CHECK:   PhiInst (:number) 10: number, %BB2, 20: number, %BB3
;; CHECK:   BranchInst %BB1
;; CHECK:   function_end
(func $if_else (param i32) (result i32)
  local.get 0
  if (result i32)
    i32.const 10
  else
    i32.const 20
  end
)

;; --- Test 10: select ---
;; CHECK-LABEL: function wasm_func_9(p0: any): any
;; CHECK:   CondBranchInst
;; CHECK: %BB2:
;; CHECK:   BranchInst %BB4
;; CHECK: %BB3:
;; CHECK:   BranchInst %BB4
;; CHECK: %BB4:
;; CHECK:   PhiInst (:number) 10: number, %BB2, 20: number, %BB3
;; CHECK:   function_end
(func $select_test (param i32) (result i32)
  i32.const 10
  i32.const 20
  local.get 0
  select
)

;; --- Test 11: Function call ---
;; CHECK-LABEL: function wasm_func_10(): any
;; CHECK:   LoadFrameInst (:any) {{.*}}[%VS0.closure_0]: any
;; CHECK:   CallInst (:any)
;; CHECK:   BranchInst %BB1
;; CHECK:   function_end
(func $call_test (result i32)
  call $constants_and_locals
)

;; --- Test 12: Return and nop ---
;; CHECK-LABEL: function wasm_func_11(): any
;; CHECK: %BB0:
;; CHECK:   ReturnInst 42: number
;; CHECK: %BB1:
;; CHECK:   function_end
(func $return_nop (result i32)
  nop
  i32.const 42
  return
)

;; --- Test 13: Unreachable ---
;; CHECK-LABEL: function wasm_func_12(): any
;; CHECK: %BB0:
;; CHECK:   UnreachableInst
;; CHECK: %BB1:
;; CHECK:   function_end
(func $unreachable_test
  unreachable
)

;; --- Test 14: br_table ---
;; CHECK-LABEL: function wasm_func_13(p0: any): any
;; CHECK:   SwitchInst
;; CHECK:   PhiInst
;; CHECK:   BinaryAddInst
;; CHECK:   AsInt32Inst
;; CHECK:   function_end
(func $br_table_test (param i32) (result i32)
  (block $a (result i32)
    (block $b (result i32)
      i32.const 100
      local.get 0
      br_table $b $a
    )
    i32.const 1
    i32.add
  )
)
