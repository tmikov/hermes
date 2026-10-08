;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for cross-module table import wiring.
;; Imports a table from e2e-table-import-exporter.wat and verifies that
;; the two modules share the same table storage — elements placed by the
;; exporter are visible here, and table.grow in the exporter is reflected.

;; REQUIRES: wasm

;; RUN: %wat2wasm %S/e2e-table-import-exporter.wat_ -o %t-exporter.wasm && %hermesc --wasm -emit-binary -out %t-exporter.hbc %t-exporter.wasm && %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-table-import-driver.js_ -- %t-exporter.hbc %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; Import the table from the exporter module.
  (import "exporter" "tbl" (table 2 10 funcref))

  ;; Read the table size.
  (func (export "imported_size") (result i32)
    table.size 0)

  ;; Call a function from the imported table via call_indirect.
  (func (export "call_at") (param i32) (result i32)
    local.get 0
    call_indirect (result i32))
)

;; CHECK: exporter size: 2
;; CHECK-NEXT: importer size: 2
;; CHECK-NEXT: call_at(0): 10
;; CHECK-NEXT: call_at(1): 20
;; CHECK-NEXT: exporter grow 1: 2
;; CHECK-NEXT: exporter size after grow: 3
;; CHECK-NEXT: importer size after grow: 3
