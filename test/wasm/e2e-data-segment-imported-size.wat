;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A data segment past the *declared* minimum of an imported memory is not
;; out of bounds. The declaration states a minimum; the module runs on the
;; memory it is actually given, which may be larger.
;;
;; The compile-time bounds check measured segments against that declared
;; minimum and planted an unconditional trap when one lay beyond it, so this
;; module -- which other engines accept -- died at instantiation with
;; "unreachable executed" whatever memory it was handed. Segments beyond the
;; declared minimum are now left to the runtime check, which measures the
;; memory in hand.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-data-segment-imported-size-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "m" (memory 1))
  (data (i32.const 65536) "hi")
  (func (export "peek") (param i32) (result i32)
    local.get 0
    i32.load8_u))

;; Given two pages, the segment is in bounds and must be applied.
;; CHECK: two pages: h=104 i=105
;; Given one page it really is out of bounds, and the runtime check must
;; still refuse it -- otherwise the fix above would have removed the check
;; rather than moved it.
;; CHECK-NEXT: one page: Error: wasmDataSegmentInit: out of bounds memory access
;; CHECK-NEXT: done
