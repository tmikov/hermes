;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; What a pristine WebAssembly.Memory / WebAssembly.Table does NOT settle.
;;
;; The constructor can no longer be replaced -- createMemoryViews and
;; createTables read it from HermesInternal.intrinsics.WebAssembly. But the
;; limits it honours come out of a DESCRIPTOR that generated code builds with
;; ordinary strict stores, and [[Set]] walks the prototype chain: an accessor
;; named `initial` or `maximum` on Object.prototype swallows the store and
;; answers the constructor's read with a number of its own choosing. The
;; result is a GENUINE Memory or Table built to limits nobody declared, while
;; the module's own memory.grow / table.grow stay bounded by the compile-time
;; literals.
;;
;; That is what the exact-limits checks in createMemoryViews() and
;; createTables() are for, and this test is the evidence they still have a job
;; after the constructor route closed. Delete those checks and this file goes
;; from a named LinkError to a silently mis-limited module.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-pristine-descriptor-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (memory 1 4)
  (table 1 2 funcref)
  (func (export "grow") (param i32) (result i32) (memory.grow (local.get 0))))

;; The constructor itself is out of reach: replacing globalThis.WebAssembly
;; wholesale changes nothing about what the module builds with.
;; CHECK: with WebAssembly replaced: instantiated
;; CHECK-NEXT: replacement was live: true

;; The descriptor is not out of reach, and the limits check catches it by
;; name rather than letting a mis-limited memory through.
;; CHECK-NEXT: with a hostile descriptor: LinkError
;; CHECK-NEXT: descriptor accessor did fire: true
;; CHECK-NEXT: done
