;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; An imported MUTABLE reference-typed global, re-exported.
;;
;; A mutable global import is shared state, so the importing module keeps the
;; WebAssembly.Global OBJECT rather than a snapshot of its value, and
;; re-exporting it must publish that very object -- not a fresh Global
;; wrapping a copy, which would track neither side's writes. The identity
;; assertion in the driver is what says so; the write assertions around it
;; would pass against any Global that happened to share the same storage.
;;
;; The global this module exports is LIVE: it is mutable and defined here, so
;; wasmMakeGlobal backs it with getter/setter closures over this module's own
;; frame slot. Its value at link time is `null`, which is the point. A live
;; global's link result was already the matched object before this change, so
;; the value cannot collide with either refusal sentinel here, and what is
;; under test is the re-export rather than the collision --
;; e2e-global-ref-import.wat covers the collision, on snapshot Globals.
;;
;; The consumer is a separate module: an import is what puts the Global on
;; the importing module's re-export path at all. It also carries the MUTABLE
;; half of the sentinel table -- a mutable declaration satisfied by a mutable
;; SNAPSHOT Global holding `null` or `undefined`, which the old link protocol
;; refused as "not a WebAssembly.Global" and as a type mismatch respectively.
;; The immutable half is in e2e-global-ref-import.wat, which has no mutable
;; declaration to satisfy.
;;
;; It carries the third side of the `undefined` exemption for the same reason:
;; the exemption is IMMUTABLE-externref-only, so a mutable externref import
;; still refuses a supplied or omitted `undefined`, and this is the module
;; that can be asked.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %wat2wasm %S/e2e-global-ref-mut-reexport-consumer.wat_ -o %t-con.wasm && %hermesc --wasm -emit-binary -out %t-con.hbc %t-con.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-global-ref-mut-reexport-driver.js_ -- %t.hbc %t-con.hbc | %FileCheck --match-full-lines %s

(module
  ;; Mutable and defined here, so it is exported live, and it starts null.
  (global $g (export "g") (mut externref) (ref.null extern))

  ;; Reads and writes the frame slot directly, reaching neither JS accessor
  ;; nor either closure. This is where a write made by the other module has
  ;; to become visible.
  (func (export "get") (result externref) global.get $g)
  (func (export "set") (param externref) (global.set $g (local.get 0)))
)

;; CHECK: the exported mutable global holds null: true
;; CHECK-NEXT: an externref Global holding null links: true

;; The re-export is the supplied object itself.
;; CHECK-NEXT: the re-export is the very object supplied: true

;; ...and it is shared, in both directions, across the module boundary.
;; CHECK-NEXT: a consumer write reaches the exporter: true
;; CHECK-NEXT: a consumer write reaches the re-export: true
;; CHECK-NEXT: an exporter write reaches the consumer: true
;; CHECK-NEXT: the re-export is still the same object: true

;; The mutable rows of the sentinel table. Each row reads its sentinel back
;; -- from the Global and from inside the module -- before anything writes,
;; keeps the object supplied, and only then has the sentinel written over, so
;; that it is shown to have been a value rather than an absence.
;; CHECK-NEXT: mutable declaration <- mutable snapshot holding null: true true true
;; CHECK-NEXT: ...whose null the module then replaces: true true
;; CHECK-NEXT: ...and one holding undefined: true true true
;; CHECK-NEXT: ...whose undefined the module then replaces: true true

;; The third side of the `undefined` exemption: it is IMMUTABLE-externref-only,
;; so a mutable externref import still takes the missing-import guard, whether
;; the property is present and undefined or absent.
;; CHECK-NEXT: mut externref <- undefined: LinkError: module has no import e.g
;; CHECK-NEXT: mut externref <- absent: LinkError: module has no import e.g
;; CHECK-NEXT: done
