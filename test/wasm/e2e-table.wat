;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for table representation and element segments.
;; Compiles to .hbc and runs via hermescli.loadHBC to verify results.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-table-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (type $void_to_i32 (func (result i32)))

  (table 5 funcref)

  ;; Element segment: place f0 at index 0, f1 at index 1, f2 at index 3.
  (elem (i32.const 0) $f0 $f1)
  (elem (i32.const 3) $f2)

  (func $f0 (result i32)
    i32.const 10
  )

  (func $f1 (result i32)
    i32.const 20
  )

  (func $f2 (result i32)
    i32.const 30
  )

  ;; Return the table size.
  (func $get_size (export "get_size") (result i32)
    table.size 0
  )

  ;; Get a table entry by index and call it (if non-null).
  ;; For now just check table.size since table.get returns a funcref
  ;; which we can't directly call without call_indirect (J.2).
)

;; CHECK: table size: 5
