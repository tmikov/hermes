;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Regression test for H20 (01a0460b-ab9a): the shortcut that hard-codes the
;; first element of a segment to table index 0 used to fire whenever
;; seg.offsetValue happened to be 0, without checking whether the scalar
;; offsetKind/offsetValue fields were actually authoritative. An extended
;; constant expression such as (i32.add (i32.const 2) (i32.const 0)) records
;; its LAST parsed constant (0) in those scalar fields even though the
;; computed offset is 2, so the shortcut placed the first function at index 0
;; instead of the computed index.

;; REQUIRES: wasm
;; RUN: %wat2wasm --enable-extended-const %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-elem-extconst-offset-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (table 6 funcref)
  (func $f0 (result i32) (i32.const 100))
  (func $f1 (result i32) (i32.const 200))
  (elem (i32.add (i32.const 2) (i32.const 0)) $f0 $f1)
  (type $t (func (result i32)))
  (func (export "call") (param i32) (result i32)
    (call_indirect (type $t) (local.get 0))))

;; The computed offset is 2 + 0 = 2, so the functions must land at slots 2
;; and 3, not 0 and 1.
;; CHECK: slot 0: call_indirect: uninitialized element
;; CHECK-NEXT: slot 1: call_indirect: uninitialized element
;; CHECK-NEXT: slot 2: 100
;; CHECK-NEXT: slot 3: 200
;; CHECK-NEXT: slot 4: call_indirect: uninitialized element
;; CHECK-NEXT: slot 5: call_indirect: uninitialized element
;; CHECK-NEXT: done
