;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; WebAssembly.Module.fromHermesBytecode loads caller-supplied .hbc, but only
;; when EnableUntrustedBytecodeFromJS is set; otherwise it throws.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-from-hermes-bytecode-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s
;; RUN: %hermes -Xhermes-internal-test-methods %S/e2e-from-hermes-bytecode-driver.js_ -- %t.hbc | %FileCheck --check-prefix=OFF --match-full-lines %s

(module (func (export "f") (result i32) (i32.const 7)))

;; CHECK: gated on: f() = 7
;; OFF: gated off: TypeError
