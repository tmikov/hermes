;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for br_table (switch dispatch).
;; Verifies SwitchInst generation with correct case values and targets.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; func 0: switch_case — br_table dispatches to 4 blocks returning
  ;; 10, 20, 30, 40 by index, or -1 for default.
  ;; First function checked exhaustively including param loading.
  (func (export "switch_case") (param i32) (result i32)
    (block $b0 (result i32)
      (block $b1 (result i32)
        (block $b2 (result i32)
          (block $b3 (result i32)
            (block $b4 (result i32)
              (i32.const -1)  ;; default value
              (local.get 0)
              (br_table $b4 $b3 $b2 $b1 $b0)
            )
            ;; case 0
            (drop)
            (i32.const 10)
            (br $b0)
          )
          ;; case 1
          (drop)
          (i32.const 20)
          (br $b0)
        )
        ;; case 2
        (drop)
        (i32.const 30)
        (br $b0)
      )
      ;; case 3
      (drop)
      (i32.const 40)
    )
  )
;; CHECK-LABEL: function wasm_func_0(p0: number): number 
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:number)
;; CHECK:   %[[P0:.*]] = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:            StoreStackInst %[[P0]]: number, %[[L0]]: number
;; CHECK:   %[[IDX:.*]] = LoadStackInst (:number) %[[L0]]: number
;; CHECK-NEXT:             SwitchInst %[[IDX]]: number, %BB11, 0: number, %BB7, 1: number, %BB8, 2: number, %BB9, 3: number, %BB10
;; CHECK: %BB1:
;; CHECK-NEXT: %[[RET:.*]] = PhiInst (:number) %{{.*}}: number, %BB2
;; CHECK-NEXT:               ReturnInst %[[RET]]: number
;; CHECK: %BB2:
;; CHECK-NEXT: %{{.*}} = PhiInst (:number) -1: number, %BB11, 10: number, %BB6, 20: number, %BB5, 30: number, %BB4, 40: number, %BB3
;; CHECK-NEXT:           BranchInst %BB1

  ;; func 1: simple_switch — all br_table targets go to same block.
  (func (export "simple_switch") (param i32) (result i32)
    (block $out (result i32)
      (i32.const 42)
      (local.get 0)
      (br_table $out $out $out)
    )
  )
;; CHECK-LABEL: function wasm_func_1(p0: number): number 
;; CHECK: %BB0:
;; CHECK:   %[[IDX1:.*]] = LoadStackInst (:number)
;; CHECK-NEXT:              SwitchInst %[[IDX1]]: number, %BB3, 0: number, %BB3, 1: number, %BB3
;; CHECK: %BB1:
;; CHECK-NEXT: %[[RET1:.*]] = PhiInst (:number) %{{.*}}: number, %BB2
;; CHECK-NEXT:                ReturnInst %[[RET1]]: number
;; CHECK: %BB2:
;; CHECK-NEXT: %{{.*}} = PhiInst (:number) 42: number, %BB3
;; CHECK-NEXT:           BranchInst %BB1

  ;; func 2: loop_switch — br_table targeting a loop header.
  (func (export "loop_switch") (param i32) (result i32)
    (local i32)
    (i32.const 0)
    (local.set 1)
    (block $break
      (loop $loop
        (local.get 0)
        (br_table $loop $break $break)
      )
    )
    (local.get 1)
  )
)
;; CHECK-LABEL: function wasm_func_2(p0: number): number 
;; CHECK: %BB0:
;; CHECK:   %[[L0_2:.*]] = AllocStackInst (:number)
;; CHECK:   %[[L1_2:.*]] = AllocStackInst (:number)
;; CHECK:        StoreStackInst 0: number, %[[L1_2]]: number
;; CHECK:        BranchInst %BB3
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI2:.*]] = PhiInst (:number) %{{.*}}: number, %BB2
;; CHECK-NEXT:                ReturnInst %[[PHI2]]: number
;; CHECK: %BB2:
;; CHECK-NEXT: %{{.*}} = LoadStackInst (:number) %[[L1_2]]: number
;; CHECK-NEXT:           BranchInst %BB1
;; CHECK: %BB3:
;; CHECK-NEXT: %[[LIDX:.*]] = LoadStackInst (:number) %[[L0_2]]: number
;; CHECK-NEXT:                SwitchInst %[[LIDX]]: number, %BB5, 0: number, %BB4, 1: number, %BB5
;; CHECK: %BB4:
;; CHECK-NEXT: BranchInst %BB3
;; CHECK: %BB5:
;; CHECK-NEXT: BranchInst %BB2
;; CHECK-NEXT: function_end
