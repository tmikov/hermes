;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; An immutable i64 global import's value was split into the lo/hi pair via
;; the retBufI Uint32Array, but the two halves were read back out of that
;; buffer WITHOUT narrowing with AsInt32Inst -- unlike every other retBufI
;; read in the pipeline. i32.wrap_i64 just forwards the lo half unchanged
;; (no builtin call, no re-coercion downstream), so it is the operation that
;; exposes the raw unsigned value directly: importing -1n produced
;; 4294967295 instead of -1. This is the same mistake as the already-fixed
;; C2 (the export wrapper's i64 param unmarshal), here in the immutable
;; import's link-time snapshot.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-i64-global-import-sign-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "g" (global i64))
  (func (export "wrap") (result i32) global.get 0 i32.wrap_i64))

;; CHECK: -1n -> -1
;; CHECK-NEXT: -2n -> -2
;; CHECK-NEXT: 5n -> 5
;; CHECK-NEXT: done
