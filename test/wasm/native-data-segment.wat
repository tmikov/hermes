;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Exercises _sh_wasm_data_segment_init, the helper Task 3 added: at
;; instantiation, a native Wasm unit copies each active data segment into
;; memory through this helper rather than through emitted string literals.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=dsegmod -c -o %t-mod.o %t.wasm
;; RUN: %shermes -o %t.exe -Wc,%t-mod.o %S/native-data-segment-driver.js_
;; RUN: %t.exe | %FileCheck --match-full-lines %s
;; RUN: %hermesc --wasm -emit-binary -out %t.hbc %t.wasm
;; RUN: %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/native-data-segment-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (memory (export "mem") 1)
  (data (i32.const 0) "hello")
  (data (i32.const 16) "\00\ff\80")
  (func (export "at") (param i32) (result i32)
    (i32.load8_u (local.get 0))))

;; An ASCII segment and one with bytes above 0x7f: the rejected string-literal
;; encoding would have handled the first and doubled the second.
;; CHECK: at(0): 104
;; CHECK-NEXT: at(4): 111
;; CHECK-NEXT: at(17): 255
;; CHECK-NEXT: at(18): 128
;; CHECK-NEXT: done
