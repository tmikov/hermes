;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test: compile a Wasm module to HBC, then load and invoke
;; its exported functions from a JS driver using hermescli.loadHBC.

;; REQUIRES: wasm

;; Compile .wat -> .wasm -> .hbc, then run the JS driver with the .hbc
;; path passed as a script argument.
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-hermescli-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (func $add (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
  )
  (func $sub (export "sub") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.sub
  )
  (func $mul (export "mul") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.mul
  )
)

;; CHECK-LABEL: exports type: object
;; CHECK-NEXT: add(3, 4) = 7
;; CHECK-NEXT: sub(10, 3) = 7
;; CHECK-NEXT: mul(6, 7) = 42
