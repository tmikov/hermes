;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for memory exports wrapped as WebAssembly.Memory objects.
;; Verifies that exported memories carry __wasm_type__, __wasm_min__,
;; __wasm_max__ metadata and have a .buffer property.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-memory-export-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (memory (export "mem") 2 8)
)

;; CHECK: mem type: object
;; CHECK-NEXT: mem __wasm_type__: memory
;; CHECK-NEXT: mem __wasm_min__: 2
;; CHECK-NEXT: mem __wasm_max__: 8
;; CHECK-NEXT: mem has buffer: true
;; CHECK-NEXT: mem buffer size: 131072
