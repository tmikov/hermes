;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for table exports wrapped as WebAssembly.Table objects.
;; Verifies that exported tables carry __wasm_type__, __wasm_min__,
;; __wasm_max__ metadata and __wasm_funcs__/__wasm_types__ arrays for
;; cross-module sharing.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-table-export-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (table (export "tbl") 5 10 funcref)
)

;; CHECK: tbl type: object
;; CHECK-NEXT: tbl __wasm_type__: table:funcref
;; CHECK-NEXT: tbl __wasm_min__: 5
;; CHECK-NEXT: tbl __wasm_max__: 10
;; CHECK-NEXT: tbl __wasm_funcs__ type: object
;; CHECK-NEXT: tbl __wasm_funcs__ length: 5
;; CHECK-NEXT: tbl __wasm_types__ type: object
;; CHECK-NEXT: tbl __wasm_types__ length: 5
