;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for table exports wrapped as WebAssembly.Table objects.
;;
;; This test used to assert the OPPOSITE of what it asserts now. An exported
;; table carried its __wasm_type__/__wasm_min__/__wasm_max__ metadata and its
;; __wasm_funcs__/__wasm_types__ backing arrays as ordinary own properties,
;; and the test pinned their presence and their lengths, because that is how
;; another module reached the storage. That publication was the linking ABI
;; itself, and it is gone: a table is now linked by brand check and its
;; storage read from internal fields. What is left to assert is that the
;; exported value is a genuine WebAssembly.Table of the declared size and that
;; it exposes NOTHING -- which is also what the spec requires of it.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-table-export-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (table (export "tbl") 5 10 funcref)
)

;; CHECK: tbl type: object
;; CHECK-NEXT: tbl instanceof WebAssembly.Table: true
;; CHECK-NEXT: tbl.length: 5
;; CHECK-NEXT: tbl own props: []
;; CHECK-NEXT: tbl JSON: {}

;; The declared maximum still bounds it, so the limits survived the move into
;; internal state even though nothing reports them any more.
;; CHECK-NEXT: grow to the maximum: 5, length 10
;; CHECK-NEXT: grow past the maximum: RangeError, length 10
