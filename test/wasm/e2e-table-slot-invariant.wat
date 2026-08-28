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

;; A frozen backing array must not be able to take the triple apart. A refused
;; element write reports SUCCESS -- a frozen JSArray answers false with no
;; exception -- so an unchecked funnel wrote some of the three arrays and left
;; the rest, which is the desynchronization that made a function callable
;; through another function's signature.
;;
;; Reached here through an externref table's storage, which comes from
;; globalThis.Array; see the note at the top. The write order is the same for
;; both table kinds, and so is the checked store, so freezing any one of the
;; three arrays must raise rather than silently drop the write.
;; CHECK-NEXT: sane externref table: extSet ok, extGet: x
;; CHECK-NEXT: frozen funcs: extSet: TypeError: Wasm table storage is not writable
;; CHECK-NEXT: frozen funcs; extGet(0): null
;; CHECK-NEXT: frozen types: extSet: TypeError: Wasm table storage is not writable
;; CHECK-NEXT: frozen types; extGet(0): null
;; CHECK-NEXT: frozen exported: extSet: TypeError: Wasm table storage is not writable
;; CHECK-NEXT: frozen exported; extGet(0): null

;; The same storage is script's to choose in shape as well as in writability,
;; so it is validated once at instantiation. A non-array is a LinkError there
;; rather than an unchecked cast in the table builtins later.
;; CHECK-NEXT: non-array storage: LinkError

;; And being a genuine array is not enough: an accessor installed at an index
;; runs on an ordinary property read, so table.get reads the indexed storage
;; directly and never calls anything.
;; CHECK-NEXT: accessor ran: false, extGet(0): null

;; table.grow has the same problem one level up: it extends all three array
;; LENGTHS and then fills the new slots. A refused length write leaves that
;; array short with no exception raised, so an unchecked grow would answer
;; "grown" over three arrays of different lengths, and a later write to a new
;; slot would land in some of them and extend others. The refusal is caught by
;; the fill below it, which is turned into the spec's "could not grow" answer
;; of -1 with the lengths rolled back -- so the table must be exactly as long
;; afterwards as it was before, whichever of the three arrays refused.
;;
;; ALL THREE LENGTHS are measured, in funcs/types/exported order. extSize()
;; compiles to funcs.length alone, and in the `frozen funcs` row Object.freeze
;; pins funcs.length at 5 no matter what the engine does -- so that row's
;; "size still 5" could not fail, and it was the rollback of the OTHER two that
;; it was supposed to be asserting. Deleting the rollback in wasmTableGrow
;; (HermesBuiltin.cpp) leaves this row at `lengths 5/7/7` while extSize() still
;; answers 5: exactly the desynchronization the paragraph above forbids, and
;; previously invisible to the whole suite.
;; CHECK-NEXT: sane grow: 5 -> 7, lengths 7/7/7, extGrow returned 5
;; CHECK-NEXT: frozen funcs grow: -1, lengths 5/5/5, size still 5
;; CHECK-NEXT: frozen types grow: -1, lengths 5/5/5, size still 5
;; CHECK-NEXT: frozen exported grow: -1, lengths 5/5/5, size still 5
;; CHECK-NEXT: done
