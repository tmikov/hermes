;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Reference-typed global IMPORTS: the direction e2e-global-ref-export.wat
;; does not cover.
;;
;; THE SENTINEL COLLISION is what this file exists for. wasmLinkGlobal used
;; to answer a matching SNAPSHOT global with its VALUE, keeping `null` for
;; "not a WebAssembly.Global" and `undefined` for "a Global that does not
;; match". For a numeric global that is three distinguishable answers; for a
;; reference-typed one it is not, because `null` and `undefined` are both
;; ordinary externref values and `null` an ordinary funcref one. Measured
;; before the fix, over both mutabilities: an externref Global holding `null`
;; was refused as not being a WebAssembly.Global, and one holding `undefined`
;; as a type mismatch. Neither had happened. The link builtin now answers a
;; match with the OBJECT, which is neither sentinel, and an immutable import
;; fetches the value from it afterwards.
;;
;; The raw-value rule is the other half. What a NON-Global JS value may be is
;; decided by the declared type at compile time: a Number, or a BigInt for
;; i64, as before; ANY JS value for externref; `null` or a WebAssembly
;; Exported Function for funcref. The funcref arm asks the
;; wasmIsExportedFunction builtin, which shares its brand with the JS API's
;; own check, so a plain JS function is refused where an Exported Function is
;; taken.
;;
;; Both directions of that arm are asserted here, and each is what keeps the
;; other from passing vacuously: a predicate stuck at false fails
;; "raw Exported Function", and one stuck at true fails
;; "raw plain function".
;;
;; The two externref imports carry DISTINCT values in every case that links,
;; so a link path that dropped a value, or crossed two slots, shows up as a
;; false rather than as an equal-looking pair. The `undefined` assertion in
;; particular needs that: undefined is what an unwritten slot would read as
;; too, and what rules that out is the same module reading `obj_a` back by
;; identity a few lines above.
;;
;; It runs with -gc-sanitize-handles=1 because the funcref raw arm calls
;; wasmIsExportedFunction, which ALLOCATES -- it reaches
;; HiddenClass::findPropertyNoMap, which initializes a missing property map
;; -- from the middle of the instantiate body's import loop. In a build
;; without HERMESVM_SANITIZE_HANDLES the flag is ignored and this is an
;; ordinary behavioural test.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js -gc-sanitize-handles=1 %S/e2e-global-ref-import-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; Two externref imports rather than one, so that the two slots are
  ;; distinguishable and a value that failed to arrive cannot be mistaken for
  ;; one that did.
  (import "e" "a" (global $a externref))
  (import "e" "b" (global $b externref))
  (import "e" "f" (global $f funcref))

  ;; These read the module's own frame slot -- what the link path snapshotted
  ;; -- and not the import object, which nothing consults again.
  (func (export "get_a") (result externref) global.get $a)
  (func (export "get_b") (result externref) global.get $b)

  ;; A global initializer fed by an immutable reference import, re-exported.
  ;; This is the second consumer of the snapshot: the value travels the link
  ;; path, an initializing constant expression, and wasmMakeGlobal's export
  ;; wrapping. `g_f` is also how a funcref value is read back, since a
  ;; `(result funcref)` export would annotate its return type as object, from
  ;; which null is excluded until the nullable-annotation task lands.
  (global (export "g_f") funcref (global.get $f))
  (global (export "g_a") externref (global.get $a))
  (global (export "g_b") externref (global.get $b))
)

;; A genuine Global of the declared type satisfies a reference import, and
;; the value arrives by IDENTITY -- through the link path, the frame slot,
;; and, for g_a, an initializer and an export wrapper as well.
;; CHECK: Global(externref) imports: true true
;; CHECK-NEXT: Global(anyfunc) import is the same function: true
;; CHECK-NEXT: initializer fed by the import: true

;; A raw JS value satisfies an immutable reference import. Any JS value is an
;; externref; a funcref takes null or an Exported Function.
;; CHECK-NEXT: raw objects satisfy externref: true true
;; CHECK-NEXT: raw null satisfies funcref: true
;; CHECK-NEXT: raw Exported Function satisfies funcref: true
;; CHECK-NEXT: a plain function is an ordinary externref: true true

;; A raw `undefined` is an externref value the import object cannot deliver:
;; the import lookup reports an absent property and a present undefined one
;; the same way, before any of this branch runs. Recorded rather than fixed.
;; CHECK-NEXT: raw undefined for externref: LinkError: module has no import e.b

;; THE SENTINEL COLLISION. Each of these was a false diagnostic before the
;; matched-object answer; they are immutable snapshot Globals, so the import
;; keeps the link result's VALUE rather than the object.
;; CHECK-NEXT: Global(externref) holding null: true
;; CHECK-NEXT: Global(externref) holding undefined: true
;; CHECK-NEXT: Global(anyfunc) holding null: true
;; CHECK-NEXT: both sentinels through an initializer: true true

;; The funcref arm refuses what is not a funcref, with a message that names
;; the rule rather than the old one about Numbers.
;; CHECK-NEXT: funcref <- plain function: LinkError: import e.f must be null or a WebAssembly exported function to satisfy a funcref global import
;; CHECK-NEXT: funcref <- plain object: LinkError: import e.f must be null or a WebAssembly exported function to satisfy a funcref global import
;; CHECK-NEXT: funcref <- number: LinkError: import e.f must be null or a WebAssembly exported function to satisfy a funcref global import

;; Both halves of the brand check still discriminate. Accepting "any Global"
;; would satisfy every positive assertion above and these three would go red.
;; CHECK-NEXT: externref <- Global(anyfunc): LinkError: import e.a is a WebAssembly.Global that does not match the declared immutable externref global import
;; CHECK-NEXT: funcref <- Global(externref): LinkError: import e.f is a WebAssembly.Global that does not match the declared immutable funcref global import
;; CHECK-NEXT: externref <- Global(i32): LinkError: import e.a is a WebAssembly.Global that does not match the declared immutable externref global import

;; ...and so does mutability, in the direction a reference type reaches: a
;; MUTABLE Global does not satisfy this immutable declaration.
;; CHECK-NEXT: externref <- Global(mut externref): LinkError: import e.a is a WebAssembly.Global that does not match the declared immutable externref global import
;; CHECK-NEXT: done
