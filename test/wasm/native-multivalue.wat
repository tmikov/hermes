;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Multi-value results travel through a scratch buffer shared between caller
;; and callee (see test/wasm/compile-retbuf-loads.wat, an IR-level test of
;; the same module with no driver). This is the behavioural counterpart: it
;; actually calls outer() and outerIndirect() through the native backend and
;; checks the results, for both the direct- and indirect-call sites.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=mvmod -c -o %t-mod.o %t.wasm
;; RUN: %shermes -o %t.exe -Wc,%t-mod.o %S/native-multivalue-driver.js_
;; RUN: %t.exe | %FileCheck --match-full-lines %s
;; RUN: %hermesc --wasm -emit-binary -out %t.hbc %t.wasm
;; RUN: %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/native-multivalue-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (func $inner (result i32 f64 i64)
    (i32.const 11)
    (f64.const 2.5)
    (i64.const 7))

  (func (export "outer") (result i32 f64 i64)
    (call $inner))

  (type $t (func (result i32 f64 i64)))
  (table 1 funcref)
  (elem (i32.const 0) $inner)
  (func (export "outerIndirect") (result i32 f64 i64)
    (call_indirect (type $t) (i32.const 0))))

;; CHECK: outer(): 11,2.5,7
;; CHECK-NEXT: outerIndirect(): 11,2.5,7
;; CHECK-NEXT: done
