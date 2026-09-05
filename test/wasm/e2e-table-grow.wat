;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for table.grow instruction.
;; Verifies growing, bounds checking against max, and table.size consistency.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-table-grow-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (table $t 2 10 funcref)

  (func $size (export "size") (result i32)
    table.size $t)

  (func $grow (export "grow") (param i32) (result i32)
    ref.null func
    local.get 0
    table.grow $t)
)

;; CHECK: initial size: 2
;; CHECK: grow 3: 2
;; CHECK: after grow: 5
;; CHECK: grow 5: 5
;; CHECK: after grow2: 10
;; CHECK: grow 1 (over max): -1
;; CHECK: still 10: 10
