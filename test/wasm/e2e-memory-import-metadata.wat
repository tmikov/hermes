;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The limits a memory import is checked against, and the ceiling memory.grow
;; enforces, used to be ordinary own properties -- __wasm_min__ and
;; __wasm_max__ -- on a script-supplied object. Three things followed:
;;
;;   * a getter or Proxy could answer differently on each read, and
;;     __wasm_max__ was read once to validate and again to store, so a rising
;;     getter passed validation and then raised the ceiling;
;;   * with no declared maximum nothing validated __wasm_max__ at all, yet it
;;     still reached a native builtin that calls getNumber() on it;
;;   * __wasm_min__ was a snapshot the constructor wrote and grow() never
;;     updated, so a grown memory understated its own size (H7).
;;
;; Both properties are gone. The size and the maximum now come out of the
;; memory's internal fields, in one wasmLinkMemory call, as numbers -- so
;; there is nothing left to answer twice, nothing to coerce, and nothing to
;; forge. What this file pins is that the values the engine uses are the
;; MEMORY'S OWN and not the import declaration's, and not anything script
;; writes onto the object.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %wat2wasm %S/e2e-memory-import-metadata-nomax.wat_ -o %t-nomax.wasm && %hermesc --wasm -emit-binary -out %t-nomax.hbc %t-nomax.wasm && %wat2wasm %S/e2e-memory-import-metadata-max64k.wat_ -o %t-max64k.wasm && %hermesc --wasm -emit-binary -out %t-max64k.hbc %t-max64k.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-memory-import-metadata-driver.js_ -- %t.hbc %t-nomax.hbc %t-max64k.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "m" (memory 1 2))
  (func (export "grow") (result i32) (memory.grow (i32.const 100)))
  (func (export "grow1") (result i32) (memory.grow (i32.const 1))))

;; A __wasm_max__ written onto the memory by script is just an own property of
;; an ordinary object now: it is not read, and growth stays bounded by the
;; maximum of 2 the memory was constructed with.
;; CHECK: forged __wasm_max__ is an own property: true
;; CHECK-NEXT: grow past max = -1

;; A grow that is within the memory's own maximum must still succeed,
;; returning the previous size -- otherwise the -1 above could be produced by
;; a blanket failure rather than by the check under test.
;; CHECK-NEXT: grow within max = 1

;; And the memory that grew is the embedder's own object, not a copy of it.
;; CHECK-NEXT: imported object followed grow: true

;; With NO declared maximum in the module, the ceiling is still the memory's
;; own: a memory whose maximum is 1 page cannot be grown, and one with no
;; maximum can. That pair is what distinguishes "the memory's maximum" from
;; both "the declaration's maximum" (there is none) and "no maximum at all".
;; CHECK-NEXT: undeclared max, memory max 1: grow = -1
;; CHECK-NEXT: undeclared max, unbounded memory: grow = 1

;; The module above declares a maximum of 2, so a memory that declares none
;; does not satisfy it and neither does one whose maximum EXCEEDS it. These
;; three rows pin the comparison, but NOT the "no maximum" sentinel: an
;; unbounded memory fails here for the arithmetic reason (65536 > 2) whichever
;; way the sentinel is spelled. The rows after them are what pin the sentinel.
;; CHECK-NEXT: unbounded memory vs (memory 1 2): LinkError: import e.m does not satisfy the declared memory limits
;; CHECK-NEXT: max-3 memory vs (memory 1 2): LinkError: import e.m does not satisfy the declared memory limits
;; CHECK-NEXT: max-2 memory vs (memory 1 2): linked

;; ...and the case where ONLY the sentinel decides. Every row above rejects an
;; unbounded memory because 65536 exceeds the declared 2, whichever way "no
;; maximum" is spelled; against a declared maximum of 65536 the comparison
;; passes and only "does it have a maximum at all" is left.
;; CHECK-NEXT: unbounded memory vs (memory 1 65536): LinkError: import e.m does not satisfy the declared memory limits
;; CHECK-NEXT: max-65536 memory vs (memory 1 65536): linked, size = 1
;; CHECK-NEXT: done
