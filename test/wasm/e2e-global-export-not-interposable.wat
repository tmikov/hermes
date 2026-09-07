;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; What a module exports is not script's to choose. The global export path
;; used to reach the constructor with an ordinary property read of
;; globalThis.WebAssembly.Global, so replacing that function handed the
;; importer a forged object carrying the module's descriptor and value -- the
;; same class of hole as the retired __wasm_type__ forgery. The memory and
;; table export paths already refused it. Construction now goes through the
;; wasmMakeGlobal builtin, which has no property to interpose on.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-global-export-not-interposable-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (global (export "g") (mut i32) (i32.const 5))
  (global (export "c") i32 (i32.const 7)))

;; CHECK: g is a genuine Global = true
;; CHECK-NEXT: g was not hijacked = true
;; CHECK-NEXT: g.value = 5
;; CHECK-NEXT: c is a genuine Global = true
;; CHECK-NEXT: c.value = 7
;; CHECK-NEXT: done
