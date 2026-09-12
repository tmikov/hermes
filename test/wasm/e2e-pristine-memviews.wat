;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The eight linear-memory views are built from pristine constructors taken
;; from HermesInternal.intrinsics, not from globalThis.
;;
;; This one is sharper than the return buffer. A float memory load emits:
;;
;;   %v = LoadPropertyInst view, idx
;;   ... trap if %v === undefined ...
;;   %t = UnionNarrowTrustedInst %v, number   ; "we proved it is not
;;                                            ;  undefined, so it must be a
;;                                            ;  Number"
;;
;; That reasoning holds for a genuine typed array and for nothing else. With
;; a replaced Float64Array the load returned whatever a script chose and the
;; result was narrowed to `number` under a TRUSTED narrow -- the strongest
;; claim the IR can make -- and it was the module's linear memory, not a
;; scratch buffer.
;;
;; memory.grow rebuilds all eight views at run time, so the grown module is
;; exercised too: that second construction site reads the same holder.
;;
;; As with the return buffer, the replacement is installed BEFORE
;; instantiation, because the views are built during instantiation.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-pristine-memviews-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (memory 1 4)
  (func (export "storeF64") (param f64) (f64.store (i32.const 0) (local.get 0)))
  (func (export "loadF64") (result f64) (f64.load (i32.const 0)))
  (func (export "storeI32") (param i32) (i32.store (i32.const 16) (local.get 0)))
  (func (export "loadI32") (result i32) (i32.load (i32.const 16)))
  (func (export "grow") (param i32) (result i32) (memory.grow (local.get 0))))

;; CHECK: replacement is live: true
;; CHECK-NEXT: module called the replaced constructors: 0

;; typeof AND a strict comparison, on a value that made the round trip through
;; linear memory.
;; CHECK-NEXT: f64 round trip: typeof number, === -3.5: true
;; CHECK-NEXT: i32 round trip: typeof number, === 1234: true

;; memory.grow rebuilds the views; the rebuilt ones are pristine too, and the
;; bytes written before the grow are still there.
;; CHECK-NEXT: grow returned old size: true
;; CHECK-NEXT: module called the replaced constructors after grow: 0
;; CHECK-NEXT: f64 survives grow: typeof number, === -3.5: true
;; CHECK-NEXT: done
