;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; wabt reports a defined global's index counting the imported globals ahead
;; of it, but moduleInfo_.globals holds the defined globals only. The init
;; expression therefore subscripted that vector with an index one-per-import
;; too high: each defined global's initializer landed in the *next* global's
;; slot, and the last one ran off the end of the vector -- an assertion in a
;; Debug build, and a heap-corrupting out-of-bounds write in a build without
;; assertions.
;;
;; Any module with an imported global and at least one defined global hits
;; it, which is an ordinary shape; nothing caught it because no test module
;; combined the two.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-imported-global-index-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "base" (global $base i32))
  (global $a i32 (i32.const 111))
  (global $b (mut i32) (i32.const 222))
  (global $c i32 (i32.const 333))

  (func (export "base") (result i32) global.get $base)
  (func (export "a") (result i32) global.get $a)
  (func (export "b") (result i32) global.get $b)
  (func (export "c") (result i32) global.get $c))

;; Each defined global must hold its own initializer, not its neighbour's,
;; and the imported one must still come from the import object.
;; CHECK: base = 9
;; CHECK-NEXT: a = 111
;; CHECK-NEXT: b = 222
;; CHECK-NEXT: c = 333
;; CHECK-NEXT: done
