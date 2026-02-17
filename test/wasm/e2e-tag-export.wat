;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for tag exports as plain objects with __wasm_type__ metadata.
;; Verifies that exported tags carry correct type strings.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s --enable-exceptions -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-tag-export-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (tag (export "tag_empty"))
  (tag (export "tag_i32") (param i32))
  (tag (export "tag_f32") (param f32))
  (tag (export "tag_i32_f64") (param i32 f64))
)

;; CHECK: tag_empty type: object
;; CHECK-NEXT: tag_empty __wasm_type__: tag::
;; CHECK-NEXT: tag_i32 type: object
;; CHECK-NEXT: tag_i32 __wasm_type__: tag:i:
;; CHECK-NEXT: tag_f32 type: object
;; CHECK-NEXT: tag_f32 __wasm_type__: tag:f:
;; CHECK-NEXT: tag_i32_f64 type: object
;; CHECK-NEXT: tag_i32_f64 __wasm_type__: tag:id:
