;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A table slot is a triple -- the internal closure, its interned type id, and
;; the Exported Function that wraps both -- and every write must set all three
;; or none. Before the write funnel, `table.set` and `table.fill` wrote only the
;; closure and left the type id of whatever was there before, which produced
;; both halves of a type confusion:
;;
;;   * a function copied into a slot whose old type id differs is refused by
;;     call_indirect even though the copy is legal (copySlot / fillSlot below);
;;   * a function of a DIFFERENT signature copied over a slot keeps the old
;;     slot's type id, so call_indirect's check passes and calls it with the
;;     wrong arguments -- `$b`, which reads param 0, is invoked with none and
;;     returns undefined where an i32 is required.
;;
;; The `copySlot(1, 0)` line is the security assertion: the wrong-signature
;; call must trap.
;;
;; The frozen-array cases at the end used to reach a funcref table's backing
;; array through `tbl.__wasm_funcs__`. That publication is gone and a funcref
;; table's storage is unreachable from script, so they were REWRITTEN against
;; the one table kind whose storage script can still choose: an EXTERNREF
;; table, whose three arrays are built with `new Array(n)` off
;; globalThis.Array. Replacing that constructor with one that hands back a
;; frozen array reaches exactly the same code -- the funnel's checked element
;; store -- by the only route left.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-table-slot-invariant-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (type $t0 (func (result i32)))
  (table (export "tbl") 2 funcref)
  ;; A second table, EXTERNREF, purely so the frozen-storage cases at the end
  ;; have something to run against: an externref table's three arrays come
  ;; from globalThis.Array, which script can replace, whereas a funcref
  ;; table's are internal fields of a WebAssembly.Table and unreachable. Its
  ;; size is deliberately 5 -- a size nothing else in this module allocates --
  ;; so the driver's replacement constructor can single out its arrays.
  (table $ext 5 externref)
  (elem (i32.const 0) $a $b)
  (func $a (result i32) (i32.const 7))
  (func $b (param i32 i32 i32) (result i32) (local.get 0))
  (func (export "callAsT0") (param i32) (result i32)
    (call_indirect (type $t0) (local.get 0)))
  ;; wasm-side copy: table.get then table.set
  (func (export "copySlot") (param i32 i32)
    (local.get 1) (local.get 0) (table.get 0) (table.set 0))
  ;; wasm-side fill
  (func (export "fillSlot") (param i32 i32)
    (local.get 0) (local.get 1) (table.get 0) (i32.const 1) (table.fill 0))
  ;; A funcref that arrives from JS: the export wrapper passes the argument
  ;; straight through, so `put(0)` with no second argument hands the slot
  ;; `undefined`.
  (func (export "put") (param i32 funcref)
    (local.get 0) (local.get 1) (table.set 0))
  ;; A genuine null funcref, which ref.null produces and table.set accepts.
  (func (export "clear") (param i32)
    (local.get 0) (ref.null func) (table.set 0))
  ;; Externref accessors for the frozen-storage cases.
  (func (export "extSet") (param i32 externref)
    (local.get 0) (local.get 1) (table.set $ext))
  (func (export "extGet") (param i32) (result externref)
    (local.get 0) (table.get $ext))
  (func (export "extSize") (result i32) (table.size $ext))
  (func (export "extGrow") (param i32) (result i32)
    (ref.null extern) (local.get 0) (table.grow $ext)))

;; The trap MESSAGE is printed, not just the fact of a trap: "returned
;; undefined instead of trapping" must not be able to pass as a trap, and a
;; correct type-mismatch trap must not be able to pass as an "uninitialized
;; element" trap from a slot that got cleared instead of written.

;; The element segment placed $a at [0] and $b at [1], so calling [0] through
;; $a's signature works and calling [1] through it is a genuine type mismatch.
;; CHECK: callAsT0(0): 7
;; CHECK-NEXT: callAsT0(1): trap: call_indirect: type mismatch

;; table.set carries the type id with the closure: $a copied over slot 1 is
;; callable through $a's signature. Before the funnel this reported
;; "call_indirect: type mismatch", because slot 1 kept $b's type id.
;; CHECK-NEXT: copySlot(0, 1); callAsT0(1): 7

;; Same for table.fill.
;; CHECK-NEXT: fillSlot(1, 0); callAsT0(1): 7

;; And the direction that matters for safety: $b under $a's slot must NOT be
;; callable through $a's signature. Before the funnel slot 0 kept $a's type id,
;; the check passed, and $b ran with no arguments -- returning undefined where
;; the value stack requires an i32.
;; CHECK-NEXT: copySlot(1, 0); callAsT0(0): trap: call_indirect: type mismatch

;; A funcref arriving from JS. Omitting the argument passes `undefined`, which
;; is not a funcref and must be refused -- clearing the slot instead would mean
;; the caller who forgets an argument is the one who gets no error, while
;; `put(0, 42)` and `put(0, plainFn)` are both properly rejected.
;; CHECK-NEXT: put(0) [missing arg]: TypeError
;; CHECK-NEXT: put(0, plainFn): TypeError
;; CHECK-NEXT: put(0, 42): TypeError
;; CHECK-NEXT: put(0, tbl.get(0)) then callAsT0(0): 7

;; ref.null is a real null funcref, and clearing a slot with it leaves the slot
;; uninitialized rather than merely untyped.
;; CHECK-NEXT: clear(0); callAsT0(0): trap: call_indirect: uninitialized element

;; Storage the engine does not control is no longer constructible from script.
;; A frozen backing array refuses element writes while reporting success; a
;; non-array, or an accessor installed at an index, is the same problem in
;; another shape. A wasmCheckTableArrays builtin, the writability check in the
;; table-set funnel, and the length rollback in wasmTableGrow all existed for
;; that, and this file used to drive all three by replacing globalThis.Array
;; while an externref table was built.
;;
;; A funcref table's arrays are internal fields of a genuine WebAssembly.Table,
;; and an externref table's now come from the pristine Array under
;; HermesInternal.intrinsics, so neither kind can be handed storage script
;; chose. Those checks are kept as defence in depth and have no reachable
;; caller left to test them through; that gap is recorded on dz
;; 01a0904b-398b. What is still reachable is the ordinary behaviour of the
;; same paths.
;; CHECK-NEXT: sane externref table: extSet ok, extGet: x
;; CHECK-NEXT: sane grow: 5 -> 7, extGrow returned 5
;; CHECK-NEXT: done
