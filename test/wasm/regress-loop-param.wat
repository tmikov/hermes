;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Regression test: br_if targeting a loop with params must update the
;; loop parameter via phi nodes. Without this fix the loop runs forever.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm
;; RUN: %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/load-hbc.js_ -- %t.hbc loop_param_brif | %FileCheck --match-full-lines %s

;; CHECK: 13

(module
  ;; Loop with param and br_if: start at 1, add 4 each iteration,
  ;; loop while < 10. Should return 13 (1->5->9->13).
  (func (export "loop_param_brif") (result i32)
    (local $x i32)
    (i32.const 1)
    (loop (param i32) (result i32)
      (i32.const 4)
      (i32.add)
      (local.tee $x)
      (local.get $x)
      (i32.const 10)
      (i32.lt_u)
      (br_if 0)
    )
  )
)
