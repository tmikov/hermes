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
;;     makes a body `ref.func` on it legal in the first place. There is no
;;     canonical object to compare it against, so the driver brands it with
;;     the table oracle and stashes it twice to show the route hands out one
;;     object rather than a fresh wrapper per execution.
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

  ;; ref.func feeding ref.is_null. This is not one of the five explicit
  ;; wasmValTypeToIRType annotation sites ref-is-null.wat pins -- the value
  ;; comes out of a frame variable, which createFunctions() declares `any`, so
  ;; this row is about the two opcodes meeting, not about that annotation.
  ;; (TypeInference does infer the variable's type from its stores and
  ;; propagate it through the LoadFrameInst, so a fold is possible here; it
  ;; just is not the fold ref-is-null.wat is about.)
  (func (export "is_null_of_ref_func") (result i32)
    (ref.is_null (ref.func $target)))

  (func $unused
    ref.func $target
    drop))

;; The oracle can say no, so the brand check below is not free. First, so
;; that a broken oracle cannot be masked by the lines it vouches for.
;; CHECK: oracle refuses a plain JS function: not an Exported Function (TypeError)
;; CHECK-NEXT: instantiated: true
;; The starting state, so that "it holds the wrapper" is a change and not the
;; initial value.
;; CHECK-NEXT: g starts null: true
;; CHECK-NEXT: after stash_target, g.value === target: true
;; ToInt32('42') is 42 and ToInt32({}) is 0, so both answers are coercion
;; results and neither is the argument passed through.
;; CHECK-NEXT: non-number arguments are coerced: 43 1
;; CHECK-NEXT: get_g() === target: true
;; A live global: the reference was taken before stash_target ran.
;; CHECK-NEXT: a retained live Global saw the global.set: true
;; CHECK-NEXT: after clear, g.value is null: true
;; The slot funnel accepts it, and the object it stored is the same one.
;; CHECK-NEXT: table.set of ref.func, read back: true
;; CHECK-NEXT: hidden is a function, not target: function true
;; CHECK-NEXT: hidden is an Exported Function: wrapper
;; CHECK-NEXT: hidden calls: 7
;; Executed twice, so this says the route hands out one object rather than
;; building a fresh wrapper each time.
;; CHECK-NEXT: hidden is the same object each time: true
;; CHECK-NEXT: ref.is_null of a ref.func: 0 number
;; CHECK-NEXT: done
