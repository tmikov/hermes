;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test end-to-end compilation and execution of a simple add function.
;; D.14: compile .wasm to .hbc and run.

;; REQUIRES: wasm

;; Test 1: Two-step compilation (hermesc -emit-binary, then hermes).
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes %t.hbc

;; Test 2: Direct execution.
;; RUN: %hermes --wasm %t.wasm

;; Test 3: Verify IR is well-formed (optimizer doesn't crash).
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
  )
  ;; Start function: calls add(3, 4).
  (func (export "_start")
    i32.const 3
    i32.const 4
    call $add
    drop
  )
  (start 1)
)

;; CHECK-LABEL: function global(): any
;; CHECK:   CreateScopeInst
;; CHECK-NEXT:   CreateFunctionInst
;; CHECK-NEXT:   StoreFrameInst
;; CHECK:   ReturnInst

;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK:   BinaryAddInst
;; CHECK-NEXT:   AsInt32Inst

;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK:   LoadFrameInst
;; CHECK:   CallInst
