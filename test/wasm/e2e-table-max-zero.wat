;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A table maximum of 0 is a real, declarable maximum -- a
;; {initial: 0, maximum: 0} table must never grow, and importing it as
;; (table 0 0 funcref) must link. The constructor used 0 as its
;; no-maximum sentinel, so such a table recorded __wasm_max__ = -1
;; ("unbounded"), was rejected by the declared-maximum link check, and
;; grew without limit. The memory constructor always got this right; the
;; sentinel is now UINT32_MAX, which no genuine maximum can exceed.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-table-max-zero-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "t" (table 0 0 funcref))
  (func (export "size") (result i32) table.size))

;; A maximum of 0 is enforced and satisfies a (table 0 0) import.
;; CHECK: max-0 grow(1): RangeError
;; CHECK-NEXT: max-0 import: linked, size = 0

;; A table with no maximum still grows, and does not satisfy a
;; declared maximum.
;; CHECK-NEXT: no-max grow(1) -> 0
;; CHECK-NEXT: no-max import: LinkError
;; CHECK-NEXT: done
