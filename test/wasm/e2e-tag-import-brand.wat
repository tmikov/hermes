;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A module-defined tag is a real WebAssembly.Tag, and a tag import is checked
;; against it rather than against a string the module published.
;;
;; createTagObjects used to build a plain object literal and store a
;; `__wasm_type__` signature on it. That store was an ordinary one, and the
;; object it went on was an object literal, whose prototype is
;; Object.prototype -- so the same three failures the function half had
;; applied here:
;;
;;   - a setter on Object.prototype.__wasm_type__ ran user JS INSIDE
;;     instantiation;
;;   - it swallowed the store, leaving the tag unsigned, at which point the
;;     importer's check fell through to its "raw JS value as tag" path and
;;     accepted anything;
;;   - and what did get stored was writable and configurable on an object
;;     handed to script, so a plain assignment rewrote it afterwards.
;;
;; The signature now lives in the C++ field of a JSWebAssemblyTag, which is
;; what `new WebAssembly.Tag(...)` has always produced and what globals,
;; memories and tables already did. Script has no name to assign to.
;;
;; This closes a spec gap on the way past: a module exporting a tag handed out
;; a plain `{}` rather than a WebAssembly.Tag, so `new
;; WebAssembly.Exception(moduleTag, ...)` could not work at all.

;; REQUIRES: wasm
;; RUN: %wat2wasm --enable-exceptions %S/e2e-tag-import-brand-exporter.wat_ -o %t-e.wasm && %hermesc --wasm -emit-binary -out %t-e.hbc %t-e.wasm && %wat2wasm --enable-exceptions %S/e2e-tag-import-brand-match.wat_ -o %t-m.wasm && %hermesc --wasm -emit-binary -out %t-m.hbc %t-m.wasm && %wat2wasm --enable-exceptions %S/e2e-tag-import-brand-mismatch.wat_ -o %t-x.wasm && %hermesc --wasm -emit-binary -out %t-x.hbc %t-x.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-tag-import-brand-driver.js_ -- %t-e.hbc %t-m.hbc %t-x.hbc | %FileCheck --match-full-lines %s

;; The module under test is the exporter; this file carries the assertions.
(module)

;; A module's tag is the same kind of object `new WebAssembly.Tag(...)` makes,
;; including when a parameter is a reference -- which JSWebAssemblyTag::ValType
;; could not express until this change.
;; CHECK: exported tag is a WebAssembly.Tag: true
;; CHECK-NEXT: a tag with a reference parameter too: true

;; And it publishes nothing.
;; CHECK-NEXT: exported tag carries __wasm_type__: undefined

;; The signature store is gone, so the setter it used to invoke never runs.
;; This says nothing about instantiation running no user JS at all -- it still
;; reads import properties and stores export names with ordinary operations,
;; either of which can reach an accessor.
;; CHECK-NEXT: setter fired during instantiation: 0 times

;; With nothing published there is nothing to swallow, so the importer's check
;; no longer falls through to accepting whatever it was handed.
;; CHECK-NEXT: swallowed signature, mismatched import: LinkError

;; The check still does its job in both directions.
;; CHECK-NEXT: matching import: linked
;; CHECK-NEXT: mismatched import: LinkError

;; Assigning the property is still allowed -- the tag is an ordinary
;; extensible object -- it is simply not what the importer consults.
;; CHECK-NEXT: forged signature, mismatched import: LinkError

;; A tag import must be a genuine tag. Both shapes are refused: one carrying
;; the signature the old check consulted, and a bare object carrying nothing.
;; The bare one is the regression the old "raw JS value as tag" path actually
;; admitted; a checker that only refused the forged property would pass the
;; first row and fail the second.
;;
;; Note what refusing these COSTS, because it is not nothing: the accepted
;; object went into tagVars_, and throw/catch compares identity, so a module
;; handed an ordinary object could throw and catch with it. That was
;; nonconforming, and node refuses it, but it was not inert.
;; CHECK-NEXT: raw JS object with a forged signature: LinkError
;; CHECK-NEXT: bare JS object as the tag: LinkError

;; The control for the three rows above that use the f64 consumer. They all
;; expect refusal, so a consumer that refused EVERY tag would satisfy them.
;; Handed the tag it actually declares, it links.
;; CHECK-NEXT: f64 consumer, the matching f64 tag: linked

;; The check is on the brand and the parameters, not on provenance: a tag JS
;; built satisfies an import whose signature it matches, and not one it does
;; not.
;; CHECK-NEXT: JS-constructed Tag with the right signature: linked
;; CHECK-NEXT: JS-constructed Tag with the wrong signature: LinkError
;; CHECK-NEXT: done
