;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for memory exports.
;;
;; This file used to assert the PRESENCE of __wasm_type__ / __wasm_min__ /
;; __wasm_max__ on the exported memory, because that trio was the linking ABI.
;; It is now the absence that matters: the ABI moved into the Memory's
;; internal fields, script cannot see it, and a WebAssembly.Memory has no own
;; properties at all -- which is also what the spec says of it.
;;
;; The limits did not become unobservable, only unpublished, so this test
;; still pins both of them -- through the accessors and the behaviour the spec
;; does define, which is the only place they were ever supposed to show.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-memory-export-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (memory (export "mem") 2 8)
)

;; CHECK: mem type: object
;; CHECK-NEXT: mem instanceof WebAssembly.Memory: true

;; Nothing is published.
;; CHECK-NEXT: mem own props: []
;; CHECK-NEXT: mem JSON: {}
;; CHECK-NEXT: mem __wasm_type__: undefined

;; The minimum is the buffer it was given: 2 pages * 65536 bytes.
;; CHECK-NEXT: mem has buffer: true
;; CHECK-NEXT: mem buffer size: 131072

;; And the declared maximum of 8 is still enforced -- the only remaining way
;; to observe it. Done last, because it mutates.
;; CHECK-NEXT: mem grow(6): 2, pages now 8
;; CHECK-NEXT: mem grow(1): RangeError, pages still 8
