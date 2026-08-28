;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test end-to-end compilation and execution of a simple add function.
;; D.14: compile .wasm to .hbc and run.

;; REQUIRES: wasm

;; Test 1: Two-step compilation (hermesc -emit-binary, then WebAssembly API).
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/load-hbc.js_ -- %t.hbc _start

;; Test 2: Verify IR is well-formed (optimizer doesn't crash).
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (func $add (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
  )
  ;; Start function: asserts add(3, 4) == 7 by trapping otherwise. Without
  ;; the comparison the result was simply dropped, so the execution RUN line
  ;; proved only that instantiation did not throw -- a wrong sum still passed.
  (func (export "_start")
    i32.const 3
    i32.const 4
    call $add
    i32.const 7
    i32.ne
    if
      unreachable
    end
  )
  (start 1)
)

;; CHECK-LABEL: function global(): object
;; CHECK:   CreateScopeInst
;; CHECK:   CreateFunctionInst {{.*}}__wasm_instantiate__
;; CHECK:   ReturnInst

;; CHECK-LABEL: function wasm_func_0(p0: number, p1: number): number 
;; CHECK:   FAddInst
;; CHECK-NEXT:   AsInt32Inst

;; CHECK-LABEL: function wasm_func_1(): undefined 
;; CHECK:   LoadFrameInst
;; CHECK:   CallInst
