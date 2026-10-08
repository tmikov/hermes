;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; i32.mul compiled natively, including a product stored back into a local.
;;
;; IRGen lowers i32.mul to a Math.imul builtin call it has already typed as a
;; Number, and the store into a Number-typed local relies on that type. The
;; native pipeline verifies the IR after every pass, so any later pass that
;; replaces the call with something less precisely typed fails the compile
;; here rather than going unnoticed.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=mulmod -c -o %t-mod.o %t.wasm
;; RUN: %shermes -o %t.exe -Wc,%t-mod.o %S/native-i32-mul-driver.js_
;; RUN: %t.exe | %FileCheck --match-full-lines %s

(module
  (func (export "mul") (param i32 i32) (result i32)
    (i32.mul (local.get 0) (local.get 1)))
  ;; Iterative factorial: the running product lives in a local.
  (func (export "fact") (param i32) (result i32) (local i32)
    (local.set 1 (i32.const 1))
    (block
      (loop
        (br_if 1 (i32.eqz (local.get 0)))
        (local.set 1 (i32.mul (local.get 1) (local.get 0)))
        (local.set 0 (i32.sub (local.get 0) (i32.const 1)))
        (br 0)))
    (local.get 1)))

;; CHECK: mul(123456789,987654321): -67153019
;; CHECK-NEXT: mul(-7,6): -42
;; CHECK-NEXT: mul(0x7fffffff,2): -2
;; CHECK-NEXT: fact(10): 3628800
;; CHECK-NEXT: fact(13): 1932053504
;; CHECK-NEXT: done
