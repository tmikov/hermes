;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; table.copy chose its copy direction by comparing the two FUNCS arrays and
;; nothing else. Two table indices can refer to ONE table's storage -- a module
;; may import the same table under two names -- and an overlapping `dst > src`
;; copy between them is a self-copy that must run backward. Deciding from a
;; single pair of arrays and then running forward reads slots it has already
;; overwritten, smearing one entry across the whole range.
;;
;; That is not fail-closed. It is the wrong-signature call this whole pass
;; exists to eliminate: the smeared slots hold `$f1`/`$f2` under `$f0`'s type
;; id, so call_indirect's check passes and calls a function that takes
;; parameters with none at all. It returned 11 and 12 where it must trap.
;;
;; The direction is chosen from an alias predicate over all six arrays --
;; every destination against every source, so a cross-role alias counts too --
;; and backward is used whenever any of them is shared. Backward is equally
;; correct for arrays that are not shared, since ordering is irrelevant then.
;;
;; REWRITTEN when the __wasm_* publications were deleted, which removed the
;; forged plain-object imports this file used to build its tables from. Two
;; scenarios replace them, and BOTH are needed -- the first alone cannot tell a
;; six-way predicate from a one-pair one:
;;
;;   1. TOTAL aliasing: one table satisfying two imports. A FUNCREF table's
;;      three arrays travel together out of one object's internal fields, so
;;      two funcref tables share all six or none. This kills "always copy
;;      forward" but not "compare the funcs pair only", since when the storage
;;      aliases at all it aliases in every role at once.
;;
;;   2. PARTIAL aliasing, which is still reachable: an EXTERNREF table's three
;;      arrays are three independent `new Array(n)` calls off globalThis.Array,
;;      and wasmCheckTableArrays checks only that each is an array, not that
;;      they are distinct. A replaced Array constructor gives two externref
;;      tables a shared array for ONE role and private arrays for the others.
;;      This is the case the six-way predicate exists for, and it is a live
;;      wrong-value read through table.get, not a fail-closed one.

;; REQUIRES: wasm

;; RUN: %wat2wasm %S/e2e-table-copy-alias-donor.wat_ -o %t-donor.wasm && %hermesc --wasm -emit-binary -out %t-donor.hbc %t-donor.wasm && %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-table-copy-alias-driver.js_ -- %t-donor.hbc %t.hbc | %FileCheck --match-full-lines %s

(module
  (type $t0 (func (result i32)))
  ;; Two table imports. The driver satisfies BOTH with one table: for FUNCREF
  ;; tables that is the only way two table indices can share storage, since
  ;; their three arrays travel together out of one object's internal fields.
  (import "e" "a" (table 4 funcref))
  (import "e" "b" (table 4 funcref))

  ;; Two DEFINED externref tables, whose six arrays are six independent
  ;; `new Array(4)` calls the driver can make overlap however it likes.
  (table $xa 4 externref)
  (table $xb 4 externref)
  (func (export "xSet") (param i32 externref)
    (local.get 0) (local.get 1) (table.set $xa))
  (func (export "xGet") (param i32) (result externref)
    (local.get 0) (table.get $xa))
  (func (export "xCopyAcross") (param i32 i32 i32)
    (local.get 0) (local.get 1) (local.get 2) (table.copy $xa $xb))

  ;; Copy from table 1 into table 0: dst, src, n.
  (func (export "copyAcross") (param i32 i32 i32)
    (local.get 0) (local.get 1) (local.get 2) (table.copy 0 1))

  ;; Call through table 0 with $f0's signature.
  (func (export "call0") (param i32) (result i32)
    (call_indirect 0 (type $t0) (local.get 0)))

  ;; The funcref in a slot of table 0, which reads the third array.
  (func (export "getAt") (param i32) (result funcref)
    (table.get 0 (local.get 0))))

;; The same table under both import names, so table 0 and table 1 are one
;; storage. copyAcross(1, 0, 3) moves slots 0..2 up by one: an overlapping
;; self-copy with dst > src, which must run backward.
;;
;; Slots 0 and 1 legitimately hold $f0 and match $t0. Slots 2 and 3 hold $f1
;; and $f2, whose signatures differ, so they must be refused. A forward copy
;; smears $f0's type id across the whole range, the check then passes, and
;; they returned 11 and 12.
;; CHECK: call0 before copy: 10 | trap | trap | trap
;; CHECK-NEXT: call0 after copy: 10 | 10 | trap | trap

;; The third array is read only by table.get, so a smear there is invisible to
;; call_indirect and shows up as the wrong Exported Function coming back out.
;; CHECK-NEXT: getAt after copy: 10 | 10 | 11 | 12

;; --- Partial aliasing: two externref tables sharing ONE array ---
;; Only the EXPORTED arrays are shared; the funcs and types arrays are private
;; to each table. A predicate that compares the funcs pair alone therefore says
;; "different tables", runs forward, and smears the first entry across the
;; shared array -- `a,a,a,a` instead of `a,a,b,c`. table.get is the only reader
;; of that array, so this is a wrong reference handed straight to Wasm code,
;; with call_indirect none the wiser.
;; CHECK-NEXT: shared exported before: a,b,c,null
;; CHECK-NEXT: shared exported after: a,a,b,c
;; CHECK-NEXT: done
