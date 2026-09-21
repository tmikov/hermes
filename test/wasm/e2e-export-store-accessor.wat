;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The exports object is POPULATED, not assigned into.
;;
;; Each export name was published with an ordinary property store. An ordinary
;; store walks the prototype chain, and the exports object is an object
;; literal, so its prototype is Object.prototype -- which means an accessor
;; installed there under an export's name intercepted the store. Two
;; consequences, both measured, and the second is the bad one:
;;
;;   - user JS ran inside instantiation, with the half-built exports object as
;;     `this`;
;;   - the store was SWALLOWED, so the export was missing from the exports
;;     object entirely. A module whose export name collides with an accessor
;;     on Object.prototype silently lost that export.
;;
;; Node does neither. This is not the __wasm_type__ problem, which also needed
;; the published value to be unforgeable afterwards and so needed an internal
;; property: the exports object is handed to script by design and is frozen
;; before the Instance is returned. All that was wrong is the store consulting
;; the prototype chain, which DefineOwnPropertyInst does not do.
;;
;; Every export kind is covered, because each has its own publication site.

;; REQUIRES: wasm
;; RUN: %wat2wasm --enable-exceptions %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-export-store-accessor-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (memory (export "m") 1)
  (table (export "t") 1 funcref)
  (global (export "g") i32 (i32.const 7))
  (tag (export "e") (param i32))
  ;; A PASSIVE data segment, so its bytes live in the engine-built segments
  ;; array and memory.init copies them out of it. That array is filled with
  ;; indexed stores, which walk the prototype chain exactly as the named ones
  ;; do -- so an Object.prototype accessor named "0" corrupted a module's own
  ;; data rather than only its exports object.
  (data $d "ABCD")
  (func (export "f") (result i32) (i32.const 42))
  (func (export "init") (memory.init $d (i32.const 0) (i32.const 0) (i32.const 4)))
  (func (export "peek") (param i32) (result i32) (i32.load8_u (local.get 0))))

;; Not one of the accessors fires, for any export kind or for the property
;; names the module-description objects use.
;; Both setters AND getters are counted, so a definition that went missing --
;; leaving the read to fall through to the inherited getter and answer
;; undefined -- shows up here rather than passing quietly.
;; CHECK: accessors fired during instantiation: 0

;; The PUBLIC description API builds its objects natively, with the same kind
;; of store, and came back with every field missing: five entries of `{}`.
;; CHECK-NEXT: Module.exports() names: e,f,g,init,m,peek,t, imports: 0

;; And every export is present and of the right kind -- the half that actually
;; loses data.
;; CHECK-NEXT: f: function
;; CHECK-NEXT: m: object, a Memory: true
;; CHECK-NEXT: t: object, a Table: true
;; CHECK-NEXT: g: object, a Global: true
;; CHECK-NEXT: e: object, a Tag: true

;; The exports object is still frozen and still enumerates its exports, so
;; defining rather than assigning has not changed what script sees.
;; CHECK-NEXT: frozen: true, keys: e,f,g,init,m,peek,t

;; The ARRAYS are filled with indexed stores, which walk the chain too, so
;; accessors named "0".."7" on Object.prototype reached them. Two consequences,
;; and again the data loss is the bad one: WebAssembly.Module.exports() came
;; back with empty entries, and a passive data segment's bytes never reached
;; the segments array, so memory.init copied nothing.
;; CHECK-NEXT: indexed accessors fired: 0
;; CHECK-NEXT: Module.exports() intact: true
;; CHECK-NEXT: memory.init copied the segment: 65,66,67,68
;; CHECK-NEXT: done
