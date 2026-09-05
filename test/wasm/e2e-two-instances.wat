;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; One module, two instances, different imports.
;;
;; Import resolution used to read a process-global __wasm_imports__ that the
;; Instance constructor set and restored around the call, so the import object
;; was observable and replaceable by any script running during instantiation --
;; an import-object getter or a Proxy trap -- and instantiating one module
;; twice with different imports was not expressible at all. instantiate() now
;; takes the import object as a parameter.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-two-instances-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "base" (global $base i32))
  (import "e" "f" (func $f (param i32) (result i32)))
  (func (export "run") (param i32) (result i32)
    global.get $base
    local.get 0
    call $f
    i32.add))

;; Each instance must use its own imports, not the other's and not whichever
;; was installed last.
;; CHECK: a.run(1) = 110
;; CHECK-NEXT: b.run(1) = 2001
;; CHECK-NEXT: a.run(1) again = 110
;; CHECK-NEXT: no global leaked: true
;; CHECK-NEXT: done
