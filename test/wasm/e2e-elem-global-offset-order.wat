;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Regression test for H20 (01a0460b-ab9a): an active element segment offset
;; by a plain `global.get` used to be applied by createTables() BEFORE
;; initializeGlobals() ran, so the offset load read the frame slot's
;; `undefined` placeholder instead of the imported value. At the default
;; optimization level that produced a hard compiler abort (a PhiInst with no
;; incoming values fails lowered-IR verification); at -O0 it silently placed
;; both functions at table index 0 instead of the requested offset.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-elem-global-offset-order-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "g" (global $g i32))
  (table 6 funcref)
  (func $f0 (result i32) (i32.const 100))
  (func $f1 (result i32) (i32.const 200))
  (elem (global.get $g) $f0 $f1)
  (type $t (func (result i32)))
  (func (export "call") (param i32) (result i32)
    (call_indirect (type $t) (local.get 0))))

;; g = 2, so the functions must land at slots 2 and 3, not 0 and 1.
;; CHECK: slot 0: call_indirect: uninitialized element
;; CHECK-NEXT: slot 1: call_indirect: uninitialized element
;; CHECK-NEXT: slot 2: 100
;; CHECK-NEXT: slot 3: 200
;; CHECK-NEXT: slot 4: call_indirect: uninitialized element
;; CHECK-NEXT: slot 5: call_indirect: uninitialized element
;; CHECK-NEXT: done
