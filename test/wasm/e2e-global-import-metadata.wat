;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A global import used to be validated by comparing a __wasm_type__ string
;; and then reading `.value`. Both are ordinary properties on a script-supplied
;; object, so:
;;
;;   * a plain object literal carrying the right string and a `value` LINKED,
;;     and the module ran on whatever that `value` was. Of the three kinds this
;;     was the only one where a bare forgery succeeded outright;
;;   * __wasm_type__ was read once to validate and again, later, to decide
;;     whether to unwrap `.value`; answering "undefined" the second time sent a
;;     WebAssembly.Global down the raw-value path and stored the import OBJECT
;;     into an i32 slot;
;;   * `.value` is a prototype accessor, so even for a genuine Global the value
;;     the module got was whatever that accessor returned.
;;
;; The import is now resolved by one wasmLinkGlobal call: a dyn_vmcast brand
;; check, an internal-field comparison of the type and the mutability, and the
;; value read straight out of the internal field.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-global-import-metadata-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "g" (global i32))
  (func (export "get") (result i32) global.get 0))

;; The forgeries. A literal shaped exactly like the old ABI used to link and
;; hand the module 1234; so did a Proxy answering "undefined" on the second
;; read; so did an object INHERITING from a genuine Global, which `instanceof`
;; accepts. All three are now refused, and by the message that names what is
;; actually wrong with them: they are not globals, so for this immutable
;; import they are judged as raw values.
;; CHECK: forged literal: LinkError: import e.g must be a Number to satisfy an i32 global import
;; CHECK-NEXT: toctou Proxy: LinkError: import e.g must be a Number to satisfy an i32 global import
;; CHECK-NEXT: Object.create(genuine global): LinkError: import e.g must be a Number to satisfy an i32 global import
;; CHECK-NEXT: object with a value property: LinkError: import e.g must be a Number to satisfy an i32 global import

;; A genuine Global's value comes out of its internal field, so replacing the
;; `value` accessor on WebAssembly.Global.prototype -- which is configurable,
;; and used to be the route the link path took -- cannot change what the
;; module sees.
;; CHECK-NEXT: hijacked prototype accessor reads: 999
;; CHECK-NEXT: WebAssembly.Global(77) with hijacked accessor: 77

;; A raw JS value is still accepted for an immutable import, and still coerced
;; to the declared type. (Retiring that coercion is a separate change.)
;; CHECK-NEXT: 3.7: 3
;; CHECK-NEXT: -1: -1
;; CHECK-NEXT: 2^32+5: 5
;; CHECK-NEXT: raw number 123: 123
;; CHECK-NEXT: done
