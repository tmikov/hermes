;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test end-to-end compilation and execution with loops and branches.
;; D.14: compile .wasm to .hbc and run.

;; REQUIRES: wasm

;; Test 1: Two-step compilation (hermesc -emit-binary, then WebAssembly API).
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/load-hbc.js_ -- %t.hbc _start

;; Test 2: Verify IR is well-formed (optimizer doesn't crash).
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; sum(n) = 1 + 2 + ... + n using a loop.
  (func $sum (param i32) (result i32)
    (local i32 i32)  ;; counter, accumulator
    i32.const 1
    local.set 1      ;; counter = 1
    i32.const 0
    local.set 2      ;; acc = 0
    (block $exit
      (loop $loop
        ;; if counter > n, break
        local.get 1
        local.get 0
        i32.gt_u
        br_if $exit
        ;; acc += counter
        local.get 2
        local.get 1
        i32.add
        local.set 2
        ;; counter++
        local.get 1
        i32.const 1
        i32.add
        local.set 1
        br $loop
      )
    )
    local.get 2
  )
  ;; Start function: calls sum(10) = 55.
  (func (export "_start")
    i32.const 10
    call $sum
    drop
  )
  (start 1)
)

;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK:   AllocStackInst
;; CHECK:   CondBranchInst
;; CHECK:   BinaryAddInst

;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK:   LoadFrameInst
;; CHECK:   CallInst
