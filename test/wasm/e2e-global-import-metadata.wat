;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; __wasm_type__ and a WebAssembly.Global's .value are ordinary properties on
;; a script-supplied object, so a getter or Proxy can answer differently on
;; each read. __wasm_type__ was read once to validate a global import and
;; again, later, to decide whether to unwrap .value; answering "undefined" the
;; second time sent a WebAssembly.Global down the raw-value path and stored the
;; import OBJECT into an i32 slot. The value is now resolved once, under the
;; check that validated it, and coerced to the declared type.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-global-import-metadata-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "g" (global i32))
  (func (export "get") (result i32) global.get 0))

;; A Proxy answering "undefined" on a second read can no longer divert a
;; WebAssembly.Global to the raw path.
;; CHECK: toctou global: reads = 1, typeof = number, value = 42

;; The resolved value is coerced to the declared type.
;; CHECK-NEXT: object value -> 0
;; CHECK-NEXT: 3.7 -> 3
;; CHECK-NEXT: -1 -> -1
;; CHECK-NEXT: 2^32+5 -> 5

;; Legitimate imports keep working.
;; CHECK-NEXT: WebAssembly.Global(77) -> 77
;; CHECK-NEXT: raw number 123 -> 123
;; CHECK-NEXT: done
