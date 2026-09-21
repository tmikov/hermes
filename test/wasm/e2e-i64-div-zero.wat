;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test i64.div_s traps on division by zero.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && ( ! %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/instantiate-hbc.js_ -- %t.hbc 2>&1 ) | %FileCheck --match-full-lines %s

(module
  (func $start
    i64.const 42
    i64.const 0
    i64.div_s
    drop)
  (start $start)
)

;; CHECK: Uncaught Error: integer divide by zero
