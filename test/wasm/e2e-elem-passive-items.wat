;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; PASSIVE element segments whose entries are not all function indices, USED
;; through table.init. Instantiating such a module proves nothing: a passive
;; segment initializes nothing on its own, so a module carrying one loads
;; cleanly whether its contents were recorded or discarded, and table.init is
;; what reads them.
;;
;; The old model stored a passive segment as a vector of function indices, so
;; `(global.get 0) (ref.null func) (ref.func $f1)` became a one-entry segment
;; and `init(0, 0, 3)` below failed the segment bounds check outright.
;;
;; As in e2e-elem-mixed-items.wat, the destination slots are filled with $f0
;; by an active segment first, so the `ref.null` entry is asserted to have
;; ERASED something rather than to have left an already-null slot alone; and
;; slot 1 is asserted by identity against a function from a different
;; instance, which neither of this instance's own wrappers satisfies.
;;
;; `init(4, 2, 1)` is the only place a non-zero `src` is used. A table.init
;; that ignored `src` and always copied from the segment's start would satisfy
;; the block above, which asks for all three entries from offset 0, and would
;; put the wrong one at slot 4.
;;
;; The externref half exercises the other new thing: table.init used to
;; brand-check every value it wrote as a funcref, which an externref entry
;; holding anything but null cannot satisfy. Whether the check applies is a
;; property of the destination table.
;;
;; Element expressions name globals by INDEX rather than by $name: wabt
;; 1.0.39's WAT parser hits an `is_index()` assertion on a named global
;; inside an element expression (dz 01a089a2-2a19). Index 0 is $gf and index 1 is $ge, in import order.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-elem-passive-items-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "gf" (global $gf funcref))
  (import "e" "ge" (global $ge externref))

  (table (export "t") 6 funcref)
  (table $et 3 externref)

  (func $f0 (export "f0") (result i32) (i32.const 100))
  (func $f1 (export "f1") (result i32) (i32.const 200))

  ;; Active, applied at instantiation: slots 0..3 hold $f0 before any
  ;; table.init runs.
  (elem (i32.const 0) $f0 $f0 $f0 $f0)

  ;; Passive, mixed.
  (elem $seg funcref
    (item (global.get 0))
    (item (ref.null func))
    (item (ref.func $f1)))

  ;; Passive, externref: neither entry is a function index.
  (elem $eseg externref
    (item (global.get 1))
    (item (ref.null extern)))

  (func (export "init") (param i32 i32 i32)
    (table.init $seg (local.get 0) (local.get 1) (local.get 2)))
  (func (export "einit") (param i32 i32 i32)
    (table.init $et $eseg (local.get 0) (local.get 1) (local.get 2)))
  (func (export "drop") (elem.drop $seg))

  (type $sig (func (result i32)))
  (func (export "call") (param i32) (result i32)
    (call_indirect (type $sig) (local.get 0)))
  (func (export "eget") (param i32) (result externref)
    (table.get $et (local.get 0))))

;; What the active segment left, before table.init touches anything.
;; CHECK: before init, slot 0 is this instance's f0: true
;; CHECK-NEXT: before init, slot 2 is this instance's f0: true

;; init(0, 0, 3): the whole segment at slot 0.
;; CHECK-NEXT: slot 0 is the imported function: true
;; CHECK-NEXT: slot 1 is null: true
;; CHECK-NEXT: slot 2 is this instance's f1: true
;; Slot 3 is past the three entries just written, so it keeps the $f0 the
;; active segment left there.
;; CHECK-NEXT: slot 3 is this instance's f0: true

;; The same three slots through call_indirect.
;; CHECK-NEXT: call(0): 100
;; CHECK-NEXT: call(1): call_indirect: uninitialized element
;; CHECK-NEXT: call(2): 200

;; init(4, 2, 1): the third entry alone, at slot 4.
;; CHECK-NEXT: slot 4 is this instance's f1: true

;; einit(0, 0, 2) into the externref table.
;; CHECK-NEXT: externref slot 0 is the imported object: true
;; CHECK-NEXT: externref slot 1 is null: true

;; After elem.drop the segment reads as empty.
;; CHECK-NEXT: init after drop: table.init: out of bounds element segment access
;; CHECK-NEXT: done
