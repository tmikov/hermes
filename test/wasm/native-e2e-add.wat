;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A natively compiled Wasm unit is an ordinary WebAssembly.Module.
;;
;; The unit object is linked directly via -Wc,: a name lookup from JS creates
;; no reference to sh_export_addmod, so an archived unit would never be
;; extracted and never register.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=addmod -c -o %t-mod.o %t.wasm
;; RUN: %shermes -o %t.exe -Wc,%t-mod.o %S/native-e2e-add-driver.js_
;; RUN: %t.exe | %FileCheck --match-full-lines %s

(module
  (func (export "add") (param i32 i32) (result i32)
    (i32.add (local.get 0) (local.get 1))))

;; CHECK: add(2,40): 42
;; CHECK-NEXT: exports: add:function
;; CHECK-NEXT: second instance distinct: true
;; CHECK-NEXT: unknown unit: true
;; CHECK-NEXT: done
