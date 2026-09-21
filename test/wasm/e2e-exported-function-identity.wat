;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; There is exactly ONE Exported Function object per Wasm function index, so a
;; function exported under two names is the same object under both. Before the
;; canonical wrapper existed, the wrapper was built once per *export entry*, so
;; `a` and `b` were two distinct closures wrapping the same function.
;;
;; The table view is the same object too: a table slot's Exported Function is
;; that one canonical wrapper, and `table.get` yields it rather than the
;; internal closure it wraps. `tbl.get(0)` is the JS-API read of the slot and
;; `getViaWasm()` is the Wasm-side `table.get` of it; both must be `a`.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-exported-function-identity-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

;; $g has the SAME signature as $f, so it shares $f's canonical type index and
;; its interned type id. It must still be a different Exported Function:
;; exportedFuncVars_ is keyed by function index, not by type.

(module
  (table (export "tbl") 1 funcref)
  (elem (i32.const 0) $f)
  (func $f (export "a") (export "b") (result i32) (i32.const 7))
  (func $g (export "c") (result i32) (i32.const 9))
  ;; The Wasm-side read of the same slot, so both the JS-API view and
  ;; table.get are pinned to the one canonical object.
  (func (export "getViaWasm") (result funcref) (table.get 0 (i32.const 0))))

;; CHECK: a === b: true
;; CHECK-NEXT: a === c: false
;; CHECK-NEXT: a === tbl.get(0): true
;; CHECK-NEXT: a === getViaWasm(): true
;; CHECK-NEXT: a() = 7
;; CHECK-NEXT: b() = 7
;; CHECK-NEXT: c() = 9
;; CHECK-NEXT: names: length,name,prototype,__wasm_type__
;; CHECK-NEXT: keys: __wasm_type__
;; CHECK-NEXT: symbols: 0
