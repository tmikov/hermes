;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A table import used to be validated by comparing a __wasm_type__ string and
;; a declared minimum against a .length, so script could supply a plain object
;; and choose the module's table storage outright. Whatever it put in
;; __wasm_funcs__/__wasm_types__ reached vmcast<JSArray> in the wasm table
;; builtins, which only asserts: a Debug build aborted in Casting.h and a
;; Release build segfaulted, both reachable from ordinary JavaScript. Each
;; individual shape was then plugged with a check of its own.
;;
;; This test was REWRITTEN when the link path became a brand check. There is
;; no shape to plug any more: a table import is satisfied by a genuine
;; WebAssembly.Table and by nothing else, so every row below is one LinkError
;; and the module never sees a value it did not build. What the file is worth
;; keeping for is that the rows are the hostile shapes that were actually
;; found -- including two that never had a check of their own and that the
;; brand closes by construction:
;;
;;   * a NON-NUMERIC __wasm_types__. `wasmCallIndirect` did
;;     `truncateToInt32(typeVal...getNumber())` on whatever the slot held, so
;;     `__wasm_types__: [{}, {}]` asserted in a Debug build and read object
;;     bits as a double in a Release one. Every earlier version of this test
;;     supplied numeric arrays, so the invariant it claims -- "every hostile
;;     shape ends in a catchable error rather than an assert or a segfault" --
;;     was never actually established for that one.
;;
;;     BE CLEAR ABOUT WHAT THE `types=objects` ROW BELOW PINS: the brand
;;     check, not that read. It is refused by the first statement of
;;     wasmLinkTable, exactly like the nine rows around it, and reverting the
;;     read to something unchecked leaves it green. The row is kept because the
;;     SHAPE is the one that used to be lethal, and because a change that made
;;     forged storage linkable again would have to make this row lie.
;;
;;     The read is unreachable through any VALID module: a funcref table's type
;;     array is an internal field written only by the funnel, which takes every
;;     id from an Exported Function's WasmFuncTypeId internal property -- always
;;     a wasmInternType result, always a number. It is NOT unreachable full
;;     stop. `WebAssembly.Module` validates, but `hermesc --wasm` does not, so
;;     a module built with `wat2wasm --no-check` can call_indirect through an
;;     EXTERNREF table, whose three arrays come from a replaceable
;;     globalThis.Array; seeding its type array with an object reaches the
;;     assert. That is a compile-path validation gap, tracked as H19 in
;;     handoff-artifacts/REVIEW.md, and it is deliberately NOT tested from here
;;     -- a test of it would be a test of an invalid module, which belongs with
;;     the fix.
;;   * an object that INHERITS from a genuine Table, or a Proxy around one.
;;     `instanceof` accepts both; a dyn_vmcast accepts neither. (Pinned in
;;     e2e-table-abi-private.wat, which owns the brand-check story.)

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %wat2wasm %S/e2e-table-import-hostile-donor.wat_ -o %t-donor.wasm && %hermesc --wasm -emit-binary -out %t-donor.hbc %t-donor.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-table-import-hostile-driver.js_ -- %t.hbc %t-donor.hbc | %FileCheck --match-full-lines %s

(module
  (type $v_i32 (func (result i32)))
  (import "e" "t" (table 2 10 funcref))
  (func (export "ci") (param i32) (result i32)
    (call_indirect (type $v_i32) (local.get 0)))
  (func (export "sz") (result i32) (table.size 0))
  ;; table.get reads the third array, which the import also supplied.
  (func (export "g") (param i32) (result funcref)
    (table.get 0 (local.get 0))))

;; Every forged shape is refused at instantiation with a LinkError, before any
;; cast. Whether its arrays were plausible or nonsense no longer matters --
;; the object is not a WebAssembly.Table, and that is the whole test.
;;
;; The MESSAGE is checked, not just the error name. With `LinkError` alone
;; every row below is the identical line, so a replacement check that refused
;; these particular literals for some unrelated reason -- a length test, a
;; typeof test, anything -- would keep all ten green while the brand check it
;; replaced was gone. The message is what says the brand check is the refusal
;; that ran, and naming the import is what says it ran on the right one.
;; CHECK: funcs=string: instantiation LinkError: import e.t is not a WebAssembly.Table
;; CHECK-NEXT: funcs=number: instantiation LinkError: import e.t is not a WebAssembly.Table
;; CHECK-NEXT: funcs=object: instantiation LinkError: import e.t is not a WebAssembly.Table
;; CHECK-NEXT: types=number: instantiation LinkError: import e.t is not a WebAssembly.Table
;; CHECK-NEXT: types=string: instantiation LinkError: import e.t is not a WebAssembly.Table
;; CHECK-NEXT: types=objects: instantiation LinkError: import e.t is not a WebAssembly.Table
;; CHECK-NEXT: well-formed: instantiation LinkError: import e.t is not a WebAssembly.Table
;; CHECK-NEXT: exported=object: instantiation LinkError: import e.t is not a WebAssembly.Table
;; CHECK-NEXT: exported=accessor array: instantiation LinkError: import e.t is not a WebAssembly.Table
;; CHECK-NEXT: accessor never ran: true

;; The metadata alone, with no storage at all, is refused for the same reason.
;; CHECK-NEXT: metadata only: instantiation LinkError: import e.t is not a WebAssembly.Table

;; And a genuine table still instantiates and works, so this is not simply
;; rejecting everything.
;; CHECK-NEXT: genuine table: instantiated, size 2
;; CHECK-NEXT: genuine table: ci(0) = 7
;; CHECK-NEXT: genuine table: g(1) is a function: true
;; CHECK-NEXT: done
