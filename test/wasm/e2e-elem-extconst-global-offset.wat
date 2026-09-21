;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Regression test for H20 (01a0460b-ab9a), exercising both bugs at once: an
;; active element segment offset by the extended constant expression
;; (i32.add (global.get $g) (i32.const 0)) with g = 2. Bug 1 (ordering) means
;; the global.get inside the expression must see the initialized value, not
;; the frame slot's `undefined` placeholder. Bug 2 (the first-element
;; shortcut) means the shortcut must not fire just because the LAST parsed
;; constant (the (i32.const 0) operand of i32.add) happens to be 0 -- the
;; computed offset is 2, not 0.

;; REQUIRES: wasm
;; RUN: %wat2wasm --enable-extended-const %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-elem-extconst-global-offset-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "g" (global $g i32))
  (table 6 funcref)
  (func $f0 (result i32) (i32.const 100))
  (func $f1 (result i32) (i32.const 200))
  (elem (i32.add (global.get $g) (i32.const 0)) $f0 $f1)
  (type $t (func (result i32)))
  (func (export "call") (param i32) (result i32)
    (call_indirect (type $t) (local.get 0))))

;; The computed offset is g + 0 = 2, so the functions must land at slots 2
;; and 3, not 0 and 1.
;; CHECK: slot 0: call_indirect: uninitialized element
;; CHECK-NEXT: slot 1: call_indirect: uninitialized element
;; CHECK-NEXT: slot 2: 100
;; CHECK-NEXT: slot 3: 200
;; CHECK-NEXT: slot 4: call_indirect: uninitialized element
;; CHECK-NEXT: slot 5: call_indirect: uninitialized element
;; CHECK-NEXT: done
