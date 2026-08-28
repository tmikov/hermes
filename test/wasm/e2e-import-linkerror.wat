;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A missing or wrong-typed import must raise WebAssembly.LinkError, and the
;; message must say which import and why.
;;
;; Every failure used to produce one of three strings -- "unknown import
;; module", "unknown import", "incompatible import type" -- none of which
;; named the import, and nothing tested any of them: no test on this branch
;; exercised a LinkError path at all. A wrong-typed memory did not even reach
;; a LinkError once the module started operating on the imported object's
;; buffer; it died later in `new Uint8Array(undefined)` with a TypeError.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-import-linkerror-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "env" "host_add" (func (param i32) (result i32)))
  (import "env" "memory" (memory 1))
  (func (export "run") (param i32) (result i32)
    local.get 0
    call 0))

;; CHECK: no namespace: LinkError: module has no import namespace env
;; CHECK-NEXT: missing function: LinkError: module has no import env.host_add
;; CHECK-NEXT: wrong-typed function: LinkError: import env.host_add is not a function
;; CHECK-NEXT: non-callable with matching type: LinkError: import env.host_add is not a function
;; CHECK-NEXT: missing memory: LinkError: module has no import env.memory
;; CHECK-NEXT: wrong-typed memory: LinkError: import env.memory is not a WebAssembly.Memory
;; CHECK-NEXT: plain object as memory: LinkError: import env.memory is not a WebAssembly.Memory
;; CHECK-NEXT: ArrayBuffer as memory: LinkError: import env.memory is not a WebAssembly.Memory
;; CHECK-NEXT: too-small memory: LinkError: import env.memory does not satisfy the declared memory limits
;; CHECK-NEXT: all good: 8
;; CHECK-NEXT: done
