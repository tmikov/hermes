;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A unit compiled with -test262 carries t262=1 in its registration, and a
;; runtime without -test262 refuses to run it: fromNativeUnit() raises a
;; TypeError naming both configurations instead of executing code generated
;; under rules this runtime does not follow.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -test262 -exported-unit=t262mod -c -o %t-mod.o %t.wasm
;; RUN: %shermes -o %t.exe -Wc,%t-mod.o %S/native-config-mismatch-driver.js_
;; RUN: %t.exe | %FileCheck --match-full-lines %s

(module
  (func (export "nop")))

;; CHECK: TypeError: true
;; CHECK-NEXT: names the unit: true
;; CHECK-NEXT: names both configurations: true
