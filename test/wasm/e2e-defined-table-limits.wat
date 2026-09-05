;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A DEFINED table is built by calling `globalThis.WebAssembly.Table`, which is
;; an ordinary property script may replace. The brand check that Task 4 added
;; establishes that what came back is a genuine WebAssembly.Table -- and stops
;; there. It says nothing about the table's LIMITS, which the module's own
;; `table.grow` does not consult: for a defined table the maximum is a
;; compile-time literal, so a substituted table with a smaller maximum of its
;; own is grown straight past it.
;;
;; Against `(table 1 2 funcref)` handed a genuine Table built with
;; `{initial: 1, maximum: 1}`, before this check existed:
;;
;;   instantiation: linked
;;   wasm table.grow(1) -> 1 ; t.length now 2
;;   JS t.grow(0) -> RangeError: would exceed maximum
;;
;; -- the table ends up with maxSize_ at 1 against storage of length 2, and the
;; module runs on limits nobody agreed to. This is the table half of the same
;; hole Task 5b closed for a defined memory (e2e-defined-memory-storage.wat),
;; on a path built in the identical shape and missed.
;;
;; The comparison is EXACT on both numbers, not the import path's >= / <=. The
;; question is not "does the supplied table satisfy a declaration" but "did the
;; constructor build the table this module asked for", and a genuine
;; construction always yields exactly the requested entries and exactly the
;; requested maximum, or none at all. The descriptor is reachable as well as
;; the constructor -- it is a fresh object literal whose `initial`/`maximum`
;; stores walk the prototype chain -- and the same comparison closes that too.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-defined-table-limits-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (table (export "t") 1 2 funcref)
  (func $f (result i32) (i32.const 7))
  (elem (i32.const 0) $f)
  (func (export "size") (result i32) table.size)
  (func (export "grow") (param i32) (result i32)
    (table.grow 0 (ref.null func) (local.get 0)))
  (func (export "call0") (result i32)
    (call_indirect (result i32) (i32.const 0))))

;; The unsubstituted module, so that "everything is a LinkError" cannot be why
;; the rows below fail.
;; CHECK: honest: size 1, call0 7, t.length 1

;; Each of the three differs from `(table 1 2 funcref)` in a different place,
;; and each gets its own row: a check comparing only the entry count or only
;; the maximum would let the other through.
;; CHECK-NEXT: substituted 1 entry, no maximum: LinkError: WebAssembly.Table did not construct a table with this module's declared limits for table 0
;; CHECK-NEXT: substituted 1 entry, maximum 1: LinkError: WebAssembly.Table did not construct a table with this module's declared limits for table 0
;; CHECK-NEXT: substituted 2 entries, maximum 2: LinkError: WebAssembly.Table did not construct a table with this module's declared limits for table 0

;; A substitute that matches the declaration exactly still links and still
;; works -- the check refuses a DIFFERENT table, not every replaced
;; constructor.
;; CHECK-NEXT: substituted exactly as declared: size 1, call0 7

;; The state the check exists to prevent, measured on the far side of it: the
;; module's table.grow must never be able to take the table past the maximum
;; the object itself carries, so the two must still agree afterwards.
;; CHECK-NEXT: honest grow(1) -> 1, t.length 2, wasm grow(1) -> -1, JS t.grow(1) -> RangeError: WebAssembly.Table.prototype.grow: would exceed maximum
;; CHECK-NEXT: done
