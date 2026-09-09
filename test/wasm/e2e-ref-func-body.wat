;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; `ref.func` in a FUNCTION BODY. It used to call warnUnsupported(), which
;; pushed `undefined`; the global.set below then stored that in a funcref
;; global and a live getter handed it back to script.
;;
;; The load-bearing assertion is identity: the value that comes back out is
;; `===` the Exported Function this module exports under the name "target".
;; Nothing but the canonical wrapper satisfies that -- not the internal
;; closure, not `undefined`, not a fresh wrapper over the same function --
;; which is why identity is asked rather than "is it callable" or "does it
;; return 42".
;;
;; The global starts as `ref.null func` and the driver prints that null before
;; calling anything, so the state the assertion ends in is not the state it
;; started in. It is `(mut funcref)` and exported, so it is exported LIVE:
;; `.value` runs the module's getter closure over the frame slot that
;; global.set wrote, rather than a snapshot taken at instantiation.
;;
;; Three more things the wrapper is needed for, each a different consumer:
;;
;;   - `get_g` reads the same slot from inside the module and returns it as a
;;     funcref, so the value crosses the export boundary as a result too.
;;   - `put_in_table` hands it to the table slot funnel, which derives the
;;     closure and the interned type id from an Exported Function and refuses
;;     anything else -- so a body pushing the internal closure fails here even
;;     if it satisfied the identity check by accident.
;;   - `$hidden` is named by no export and sits in no table. Its wrapper
;;     exists because `(elem declare)` is a declaration site, which is what
;;     makes a body `ref.func` on it legal in the first place; script reaches
;;     it only through this route, so `typeof` and a call are all that can be
;;     asked of it.
;;
;; `unused` is never called. `ref.func; drop` in an uncalled function is the
;; shape that worked before the opcode was implemented -- the placeholder was
;; pushed and immediately discarded -- and it has to keep compiling.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-ref-func-body-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (table (export "tbl") 2 funcref)

  ;; Exported by name: the object every assertion below compares against.
  (func $target (export "target") (param i32) (result i32)
    (i32.add (local.get 0) (i32.const 1)))

  ;; Not exported, not in any table. The declarative segment below is its
  ;; only declaration.
  (func $hidden (result i32)
    (i32.const 7))

  (elem declare func $hidden)

  ;; Mutable and exported, so this is a live export over the module's own
  ;; frame slot. It starts null.
  (global $g (export "g") (mut funcref) (ref.null func))

  (func (export "stash_target")
    (global.set $g (ref.func $target)))

  (func (export "stash_hidden")
    (global.set $g (ref.func $hidden)))

  (func (export "clear")
    (global.set $g (ref.null func)))

  ;; Reads the frame slot from inside the module.
  (func (export "get_g") (result funcref)
    (global.get $g))

  ;; Straight into the table slot funnel, with no global in between.
  (func (export "put_in_table") (param i32)
    (table.set 0 (local.get 0) (ref.func $target)))

  ;; ref.func feeding ref.is_null. The value it pushes is typed `any` by the
  ;; frame variable it comes out of, so no fold is possible here the way it
  ;; was for the annotated sites ref-is-null.wat pins; this row is about the
  ;; two opcodes meeting at all.
  (func (export "is_null_of_ref_func") (result i32)
    (ref.is_null (ref.func $target)))

  (func $unused
    ref.func $target
    drop))

;; CHECK: instantiated: true
;; The starting state, so that "it holds the wrapper" is a change and not the
;; initial value.
;; CHECK-NEXT: g starts null: true
;; CHECK-NEXT: after stash_target, g.value === target: true
;; CHECK-NEXT: and it is not the internal closure: 43
;; CHECK-NEXT: get_g() === target: true
;; A live global: the reference was taken before stash_target ran.
;; CHECK-NEXT: a retained live Global saw the global.set: true
;; CHECK-NEXT: after clear, g.value is null: true
;; The slot funnel accepts it, and the object it stored is the same one.
;; CHECK-NEXT: table.set of ref.func, read back: true
;; CHECK-NEXT: hidden is a function, not target: function true
;; CHECK-NEXT: hidden calls: 7
;; CHECK-NEXT: ref.is_null of a ref.func: 0 number
;; The oracle can say no, so the identity lines above are not free.
;; CHECK-NEXT: Table.prototype.set refuses a plain JS function: TypeError
;; CHECK-NEXT: done
