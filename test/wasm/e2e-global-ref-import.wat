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
;; There are two externref imports so that the slots are distinguishable. In
;; the cases where they carry different values -- which is most of them, the
;; plain-function case being the exception, since it puts one callable in both
;; -- a link path that dropped a value or crossed two slots reads as a false
;; rather than as an equal-looking pair. The `undefined` assertions need that
;; most: undefined is also what an unwritten slot reads as, and what rules
;; that out is `a` coming back by identity in the same instantiation --
;; itself an exempt immutable externref, so it travels the same guard-elided
;; path.
;;
;; `undefined` is an externref value like any other, and the import-object
;; lookup used to refuse it before the raw rule ever ran: a property holding
;; `undefined` and an absent property read alike, and the guard called both a
;; missing import. Both link now, for an IMMUTABLE EXTERNREF import and for
;; nothing else. Measured on node v24.13.1, which has no missing-import
;; concept for globals at all -- it reads the property, takes `undefined` when
;; absent, and applies the type rule. The `n` import below is here to keep the
;; exemption narrow: a numeric global still refuses `undefined`, as does a
;; funcref one, and each keeps the message it had. The third side of that
;; boundary -- a MUTABLE externref import, which also keeps the guard -- is
;; pinned in e2e-global-ref-mut-reexport.wat, which has a mutable declaration
;; to supply.
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
  ;; A numeric import, so that the `undefined` exemption above is shown to be
  ;; narrow rather than a hole in the guard. Nothing in the module body reads
  ;; it: its whole purpose is to be supplied, or not, from the driver.
  (import "e" "n" (global $n i32))

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

;; A raw `undefined` satisfies an externref import, and so does an ABSENT
;; property, which reads as `undefined`. Both reach the same arm.
;; CHECK-NEXT: raw undefined satisfies externref: true true
;; CHECK-NEXT: an absent externref import satisfies it too: true true

;; ...and the exemption is IMMUTABLE-externref-only. A funcref and a numeric
;; global still refuse `undefined`, and an absent property for either, each
;; with the message it had; so does a mutable externref import, pinned in
;; e2e-global-ref-mut-reexport.wat. (That the message names a missing import
;; rather than a type error is a divergence from node, filed as dz
;; 01a0855d-6b5b.)
;; CHECK-NEXT: funcref <- undefined: LinkError: module has no import e.f
;; CHECK-NEXT: funcref <- absent: LinkError: module has no import e.f
;; CHECK-NEXT: i32 <- undefined: LinkError: module has no import e.n
;; CHECK-NEXT: i32 <- absent: LinkError: module has no import e.n

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
