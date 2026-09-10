;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; DECLARATIVE element segments, in both of their binary forms, once the
;; segment model records entries that are not function indices.
;;
;; Two things are at stake.
;;
;; 1. A declarative segment declares function indices without putting anything
;;    in a table, and declaring is what makes a body `ref.func` on an index
;;    legal. The wrapper-discovery pass reads the same entry list as the two
;;    table-writing paths, so a function index has to survive the widened
;;    model on THIS path too -- and here it does not arrive alone: a
;;    `ref.null` entry precedes it. `onRefFunc` refuses a module whose
;;    `ref.func` operand has no canonical wrapper, so a lost declarative
;;    entry does not compile at all; the `call` results below then say that
;;    the wrapper reaching script is a real Exported Function rather than
;;    something merely non-null, since call_indirect consults the interned
;;    type id that the slot funnel derives from the wrapper it is handed.
;;
;; 2. `elem declare` has two encodings. The funcidx form is flags 3; the
;;    EXPRESSION form is flags 7, and the reader used to test `flags == 3`,
;;    so it filed the expression form under Passive. A passive segment keeps
;;    its contents for table.init while a declarative one is dropped before
;;    the module starts, and wabt's validator lets `table.init` name a
;;    declared segment, so the misfiling is reachable. The first refusal
;;    below is the one it changed -- that table.init used to succeed and
;;    populate the table; the second is the funcidx form, which was already
;;    right, and is here so that the two encodings are seen to agree.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-elem-declare-items-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (table (export "tbl") 4 funcref)

  ;; Neither is exported and neither is in a table. The declarative segments
  ;; below are their only declaration.
  (func $hidden (result i32) (i32.const 7))
  (func $hidden2 (result i32) (i32.const 9))

  ;; Expression form: flags 7. The ref.null entry comes first.
  (elem declare funcref (item (ref.null func)) (item (ref.func $hidden)))
  ;; Funcidx form: flags 3.
  (elem declare func $hidden2)

  ;; Mutable, exported, so `.value` runs the module's getter over the frame
  ;; slot global.set wrote rather than an instantiation-time snapshot.
  (global $g (export "g") (mut funcref) (ref.null func))

  (func (export "stash") (global.set $g (ref.func $hidden)))
  (func (export "stash2") (global.set $g (ref.func $hidden2)))
  (func (export "put") (param i32) (table.set (local.get 0) (global.get $g)))

  (type $sig (func (result i32)))
  (func (export "call") (param i32) (result i32)
    (call_indirect (type $sig) (local.get 0)))

  ;; Both target slots 2 and 3, which nothing else writes.
  (func (export "init_expr_form")
    (table.init 0 (i32.const 2) (i32.const 0) (i32.const 2)))
  (func (export "init_funcidx_form")
    (table.init 1 (i32.const 2) (i32.const 0) (i32.const 1))))

;; The global starts null, so the assertions below end in a state they did
;; not start in.
;; CHECK: g starts null: true
;; CHECK-NEXT: after stash, g is an object: true

;; The value from the expression-form declarative segment, through the table
;; funnel and call_indirect.
;; CHECK-NEXT: call(0): 7
;; CHECK-NEXT: stash hands out the same object twice: true
;; The value from the funcidx-form one.
;; CHECK-NEXT: call(1): 9

;; Both segments are declarative, so both are dropped before the module
;; starts and neither has contents for table.init to copy.
;; CHECK-NEXT: init_expr_form: table.init: out of bounds element segment access
;; CHECK-NEXT: init_funcidx_form: table.init: out of bounds element segment access
;; CHECK-NEXT: slots 2 and 3 are still null: true true
;; CHECK-NEXT: done
