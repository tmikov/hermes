;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A function import is type-checked against the exporter's real signature,
;; read from the Exported Function brand rather than from a property.
;;
;; It used to be checked against `__wasm_type__`, an ordinary string property
;; the compiler STORED on each export wrapper. An ordinary store walks the
;; prototype chain, so the whole thing was script's:
;;
;;   - a setter on Function.prototype.__wasm_type__ ran user JS INSIDE
;;     instantiation, with the module's own wrapper as `this`. Node runs none.
;;   - the property arrived writable, enumerable and configurable on an object
;;     handed straight to script, so a plain assignment rewrote the signature
;;     after the fact -- no setter needed. An importer then compared against
;;     the forgery and linked.
;;
;; The interned type id that replaces it is already on the wrapper, put there
;; by wasmSetFuncInfo, in an internal property: script cannot read, write or
;; shadow it, because it has no name to assign to.
;;
;; What must NOT change is that an unbranded JS callable still satisfies a
;; function import. That is the JS API working as specified -- the host function takes
;; its signature from the IMPORTING module's declaration -- and it is why the
;; forgery bought an attacker nothing they could not already have by passing a
;; forwarding function. The check exists to catch a genuine export wired to
;; the wrong import, which is what Node reports as
;; "imported function does not match the expected type".

;; Every line this file checks was compared against Node v24.13.1 on the same
;; three modules, and the whole output matches it exactly -- including the two
;; that are NOT about the defect: a mismatched import is a LinkError there too,
;; and a plain callable links there too.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %wat2wasm %S/e2e-func-import-brand-match.wat_ -o %t-m.wasm && %hermesc --wasm -emit-binary -out %t-m.hbc %t-m.wasm && %wat2wasm %S/e2e-func-import-brand-mismatch.wat_ -o %t-x.wasm && %hermesc --wasm -emit-binary -out %t-x.hbc %t-x.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-func-import-brand-driver.js_ -- %t.hbc %t-m.hbc %t-x.hbc | %FileCheck --match-full-lines %s

(module
  (func (export "f") (param i32) (result i32)
    (i32.mul (local.get 0) (i32.const 2)))
  ;; The signature consumer 2 declares. It exists so that consumer 2 can be
  ;; handed a BRANDED function it must accept, which is what makes its
  ;; refusals above about the signature rather than about branding.
  (func (export "g") (param f64) (result f64)
    (f64.mul (local.get 0) (f64.const 2))))

;; Nothing is published on the wrapper. Node reports undefined here too.
;; CHECK: export carries __wasm_type__: undefined

;; The signature store is gone, so the setter it used to invoke never runs.
;; This says nothing about instantiation running no user JS at all -- it still
;; reads import properties and stores export names with ordinary operations,
;; either of which can reach an accessor. Node reports 0 here too.
;; CHECK-NEXT: setter fired during instantiation: 0 times

;; The interception was not only a way to FORGE a signature, it was a way to
;; DELETE one: the setter swallowed the store, so a wrapper created while it
;; was installed carried nothing and the importer's check fell through to the
;; path that accepts any callable. That is every module instantiated while the
;; setter is in place -- wrappers made earlier keep their own property. With
;; nothing published there is nothing to swallow.
;; CHECK-NEXT: swallowed signature, mismatched import: LinkError

;; The check still does its job in both directions.
;; CHECK-NEXT: matching import: linked, call(21) = 42
;; CHECK-NEXT: mismatched import: LinkError

;; And the forgery no longer reaches it. Assigning the property is still
;; ALLOWED -- the wrapper is an ordinary extensible object -- it is simply not
;; what the importer consults, so the mismatch is still refused. The second
;; line proves the assignment took, so the first is not passing because the
;; write silently failed.
;; CHECK-NEXT: forged signature, mismatched import: LinkError
;; CHECK-NEXT: the forged property is really there: func:d:d

;; Unchanged, and deliberately so: an UNBRANDED callable satisfies a function
;; import. A branded one is checked, and the mismatch row above is refused.
;; CHECK-NEXT: plain JS callable: linked, call(21) = 42

;; The controls for the three "mismatched import" rows. They all use the f64
;; consumer and all expect refusal, so a consumer that refused every function
;; would satisfy every one of them.
;;
;; Both are needed. The unbranded callable alone would not exclude an
;; implementation that refused every BRANDED f64 import, which is what most of
;; those rows hand it; the second line closes that by handing it the
;; exporter's own branded (f64)->f64 function. 21 doubled is 42 either way.
;; CHECK-NEXT: f64 consumer, unbranded callable: linked, call(21) = 42
;; CHECK-NEXT: f64 consumer, the matching branded export: linked, call(21) = 42
;; CHECK-NEXT: done
