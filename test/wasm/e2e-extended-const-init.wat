;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Extended constant expressions are recorded as a stack-machine sequence, but
;; the scalar initKind/initValue and offsetKind/offsetValue fields alongside
;; them only ever hold the LAST constant parsed. Only the data-segment consumer
;; read the sequence; globals and element segments read the scalars, so
;; (i32.add (i32.const 1) (i32.const 2)) initialized a global to 2, and an
;; element segment at (i32.add (i32.const 100) (i32.const 5)) placed its
;; functions at index 5. Both were silent -- no diagnostic, wrong result.

;; REQUIRES: wasm
;; RUN: %wat2wasm --enable-extended-const %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-extended-const-init-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (type $v_i32 (func (result i32)))
  (table 200 funcref)

  ;; add: 1 + 2 = 3, not 2
  (global $g_add i32 (i32.add (i32.const 1) (i32.const 2)))
  ;; sub: 10 - 4 = 6, not 4
  (global $g_sub i32 (i32.sub (i32.const 10) (i32.const 4)))
  ;; mul: 6 * 7 = 42, not 7
  (global $g_mul i32 (i32.mul (i32.const 6) (i32.const 7)))
  ;; nested: (2 + 3) * 4 = 20, not 4
  (global $g_nest i32
    (i32.mul (i32.add (i32.const 2) (i32.const 3)) (i32.const 4)))
  ;; a plain constant must keep working
  (global $g_plain i32 (i32.const 9))

  (func $f (result i32) i32.const 42)
  ;; 100 + 5 = 105, not 5
  (elem (i32.add (i32.const 100) (i32.const 5)) $f)

  (func (export "g_add") (result i32) global.get $g_add)
  (func (export "g_sub") (result i32) global.get $g_sub)
  (func (export "g_mul") (result i32) global.get $g_mul)
  (func (export "g_nest") (result i32) global.get $g_nest)
  (func (export "g_plain") (result i32) global.get $g_plain)
  (func (export "at") (param i32) (result i32)
    (call_indirect (type $v_i32) (local.get 0))))

;; CHECK: g_add = 3
;; CHECK-NEXT: g_sub = 6
;; CHECK-NEXT: g_mul = 42
;; CHECK-NEXT: g_nest = 20
;; CHECK-NEXT: g_plain = 9

;; The element must land at the computed index, and the last-constant index
;; must be empty -- checking only the first would still pass if the segment
;; were written to both.
;; CHECK-NEXT: at(105) = 42
;; CHECK-NEXT: at(5) trapped
;; CHECK-NEXT: done
