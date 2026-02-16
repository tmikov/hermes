;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for global exports wrapped as WebAssembly.Global objects.
;; Verifies that exported globals carry __wasm_type__ metadata and that
;; cross-module global type validation works.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-global-export-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (global (export "g_i32") i32 (i32.const 42))
  (global (export "g_f64") f64 (f64.const 3.14))
  (global (export "g_mut") (mut i32) (i32.const 100))
)

;; CHECK: g_i32 type: object
;; CHECK-NEXT: g_i32 __wasm_type__: global:i32:const
;; CHECK-NEXT: g_i32 value: 42
;; CHECK-NEXT: g_f64 type: object
;; CHECK-NEXT: g_f64 __wasm_type__: global:f64:const
;; CHECK-NEXT: g_f64 value: 3.14
;; CHECK-NEXT: g_mut type: object
;; CHECK-NEXT: g_mut __wasm_type__: global:i32:var
;; CHECK-NEXT: g_mut value: 100
