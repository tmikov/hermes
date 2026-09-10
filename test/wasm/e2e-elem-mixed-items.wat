;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; ELEMENT EXPRESSIONS other than `ref.func`. An element segment used to be
;; stored as a bare `std::vector<uint32_t>` of function indices, so a
;; `global.get` entry and a `ref.null` entry recorded nothing at all. That did
;; not merely lose those entries: every LATER entry of the segment moved down
;; by the number of them, so `(global.get 0) (ref.null func) (ref.func $f1)`
;; put $f1 at the segment's first index instead of its third.
;;
;; The segment below is applied at offset 1 over slots the PRECEDING segment
;; has already filled with $f0. That is what makes the `ref.null` assertion
;; mean something: a slot that was null to begin with reads as null whether
;; the null entry was stored or skipped, so the entry is aimed at an occupied
;; slot instead.
;;
;; Slot 1 is asserted by IDENTITY against a function from a DIFFERENT
;; instance -- the one handed in through the funcref global -- so neither of
;; this instance's own two wrappers satisfies it, and the misplacement the old
;; model produced (this instance's $f1 at slot 1) reads as false rather than
;; as a plausible function. Identity rather than a printed value because an
;; Exported Function is named after the Wasm function it wraps: the other
;; instance's $f0 and this one's print the same text.
;;
;; The `call` results are the second, independent consumer: table.get reads
;; the wrapper array, call_indirect reads the closure and interned-type
;; arrays, and a segment written through only one of the three would satisfy
;; one of the two and not the other.
;;
;; The externref segment is here because whether the slot funnel brand-checks
;; a value is a property of the TABLE, not of the segment. The active-element
;; loop used to hand it a hardcoded "this is a funcref table", which was true
;; only while every entry was a function index; an externref entry carrying an
;; ordinary JS object would have been refused with a TypeError at
;; instantiation.
;;
;; Element expressions name globals by INDEX rather than by $name throughout:
;; wabt 1.0.39's WAT parser hits an `is_index()` assertion on a named global
;; inside an element expression (dz 01a089a2-2a19). Index 0 is $gf and index 1 is $ge, in import
;; order.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-elem-mixed-items-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "gf" (global $gf funcref))
  (import "e" "ge" (global $ge externref))

  (table (export "t") 6 funcref)
  (table $et 3 externref)

  (func $f0 (export "f0") (result i32) (i32.const 100))
  (func $f1 (export "f1") (result i32) (i32.const 200))

  ;; Applied first, so slots 0..3 hold $f0 when the segment below runs.
  (elem (i32.const 0) $f0 $f0 $f0 $f0)

  ;; The mixed segment: three entries at slots 1, 2 and 3.
  (elem (i32.const 1) funcref
    (item (global.get 0))
    (item (ref.null func))
    (item (ref.func $f1)))

  ;; Two entries, neither of them a function index, into an externref table.
  (elem (table $et) (i32.const 0) externref
    (item (global.get 1))
    (item (ref.null extern)))

  (type $sig (func (result i32)))
  (func (export "call") (param i32) (result i32)
    (call_indirect (type $sig) (local.get 0)))
  (func (export "eget") (param i32) (result externref)
    (table.get $et (local.get 0))))

;; Slot 0 is outside the mixed segment, so it keeps what the first segment
;; put there.
;; CHECK: slot 0 is this instance's f0: true
;; The global.get entry, at the segment's FIRST index.
;; CHECK-NEXT: slot 1 is the imported function: true
;; CHECK-NEXT: slot 1 is not this instance's f1: true
;; The ref.null entry erased the $f0 the first segment left at slot 2.
;; CHECK-NEXT: slot 2 is null: true
;; The ref.func entry, at the segment's THIRD index.
;; CHECK-NEXT: slot 3 is this instance's f1: true
;; Slot 4 is past the end of both segments.
;; CHECK-NEXT: slot 4 is null: true

;; The same three slots read through call_indirect, which consults the other
;; two arrays of the triple.
;; CHECK-NEXT: call(1): 100
;; CHECK-NEXT: call(2): call_indirect: uninitialized element
;; CHECK-NEXT: call(3): 200

;; The externref table.
;; CHECK-NEXT: externref slot 0 is the imported object: true
;; CHECK-NEXT: externref slot 1 is null: true
;; CHECK-NEXT: done
