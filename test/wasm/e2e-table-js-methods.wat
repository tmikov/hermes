;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; WebAssembly.Table.prototype.get/set/grow on the three-array representation.
;;
;; `set` is ToWebAssemblyValue for funcref (js-api "ToWebAssemblyValue", the
;; `ref null heaptype` case): null, or an object with a [[FunctionAddress]]
;; internal slot -- an Exported Function -- and nothing else. Every other value
;; falls through to the host-reference branch, whose type does not match
;; `ref null func`, so it is a TypeError. A plain JS function is therefore
;; REFUSED; it used to be accepted and stored, and call_indirect then called it
;; through whatever signature the slot's stale type id claimed.
;;
;; A legitimate Exported Function of the WRONG signature is accepted -- it is a
;; funcref -- but it must arrive with its OWN type id, so call_indirect traps
;; instead of calling it. That is the security assertion here: before this
;; change `tbl.set(0, tbl.get(1))` left slot 0's old type id in place and
;; `callAsT0(0)` ran $b, which reads param 0, with no arguments at all.
;;
;; The value argument is `optional any` in WebIDL with no default, so OMITTING
;; it is DefaultValue(funcref) = null, while passing `undefined` explicitly is
;; an ordinary non-funcref value and a TypeError. wpt wasm/jsapi/table/
;; get-set.any.js pins both ("Setting non-function" lists `undefined`;
;; "Arguments for anyfunc table set" calls `table.set(0)` and reads back null).
;;
;; The frozen-array cases this file used to end with are GONE, and were not
;; replaced here. They reached a JS-API table's backing arrays through
;; `tbl.__wasm_funcs__` and its two siblings; those publications are deleted
;; and a WebAssembly.Table's storage is now internal fields that script cannot
;; name, so no JS-API table can be given an array that refuses writes. The
;; rollback in Table.prototype.grow and the checked store under
;; Table.prototype.set are therefore unreachable from the JS API and keep no
;; test here. The same checked store IS still reachable, on an EXTERNREF
;; table whose arrays come from globalThis.Array, and it is exercised there --
;; see the frozen-storage cases at the end of e2e-table-slot-invariant.wat.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-table-js-methods-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (type $t0 (func (result i32)))
  (table (export "tbl") 2 funcref)
  (elem (i32.const 0) $a $b)
  (func $a (export "a") (result i32) (i32.const 7))
  (func $b (export "b") (param i32 i32 i32) (result i32) (local.get 0))
  (func (export "callAsT0") (param i32) (result i32)
    (call_indirect (type $t0) (local.get 0))))

;; The element segment armed both slots, and `get` hands out the canonical
;; Exported Function rather than the internal closure.
;; CHECK: tbl.get(0) === a: true
;; CHECK-NEXT: tbl.get(1) === b: true
;; CHECK-NEXT: tbl.get(0)(): 7
;; CHECK-NEXT: callAsT0(0): 7

;; Everything that is not null and not an Exported Function is refused, and a
;; refused set leaves the slot exactly as it was. The MESSAGE is pinned on the
;; first one: the shared table-write funnel refuses the same values with a
;; message of its own, so only the message shows that this method checked the
;; value itself -- which is what puts the check before the bounds check below.
;; CHECK-NEXT: set(0, plainFn): TypeError: WebAssembly.Table.prototype.set: value must be null or a WebAssembly exported function
;; CHECK-NEXT: set(0, arrowFn): TypeError
;; CHECK-NEXT: set(0, undefined): TypeError
;; CHECK-NEXT: set(0, 42): TypeError
;; CHECK-NEXT: set(0, {}): TypeError
;; CHECK-NEXT: set(0, Math.max): TypeError
;; CHECK-NEXT: after refusals, tbl.get(0) === a: true
;; CHECK-NEXT: after refusals, callAsT0(0): 7

;; Spec order: the value is converted in the method and the range failure comes
;; out of the write that follows, so a bad value beats a bad index.
;; CHECK-NEXT: set(oob, plainFn): TypeError
;; CHECK-NEXT: set(oob, a): RangeError

;; The security case. $b is a genuine Exported Function, so storing it is
;; legal, but it has a different signature and must arrive with its own type
;; id: call_indirect through $t0 has to trap rather than run $b with no
;; arguments.
;; CHECK-NEXT: set(0, tbl.get(1)): accepted
;; CHECK-NEXT: tbl.get(0) === b: true
;; CHECK-NEXT: callAsT0(0): trap: call_indirect: type mismatch

;; Setting a matching Exported Function back re-arms the slot, so the trap
;; above was the type id doing its job and not the slot being broken.
;; CHECK-NEXT: set(0, a): accepted
;; CHECK-NEXT: callAsT0(0): 7

;; null clears the slot; the slot is then uninitialized, not merely untyped.
;; CHECK-NEXT: set(0, null): accepted
;; CHECK-NEXT: tbl.get(0): null
;; CHECK-NEXT: callAsT0(0): trap: call_indirect: uninitialized element

;; An OMITTED value is the element type's default, which for funcref is null.
;; It must not be confused with an explicit `undefined`, refused above.
;; CHECK-NEXT: set(0, a) then set(0) [omitted]: accepted, tbl.get(0): null

;; A JS-set slot is the module's own storage, so a second instance's Exported
;; Function is callable through this instance's call_indirect: the wrapper
;; carries its own closure and its own INTERNED type id, which is shared
;; across instances of the same signature.
;; CHECK-NEXT: set(0, inst2.a); callAsT0(0): 7
;; CHECK-NEXT: set(0, inst2.b); callAsT0(0): trap: call_indirect: type mismatch

;; grow appends cleared slots to all three arrays: `get` reads null (not
;; undefined from past the end of a short array, and not a stale closure), and
;; the new slot is a real slot that a later set can arm.
;; CHECK-NEXT: grow(2) -> 2, length: 4
;; CHECK-NEXT: tbl.get(2): null
;; CHECK-NEXT: tbl.get(3): null
;; CHECK-NEXT: callAsT0(2): trap: call_indirect: uninitialized element
;; CHECK-NEXT: set(2, a); callAsT0(2): 7
;; CHECK-NEXT: entries below the growth point survived: true

;; The same rules on a table that no module ever touched.
;; CHECK-NEXT: jsTbl.get(0): null
;; CHECK-NEXT: jsTbl.set(0, plainFn): TypeError
;; CHECK-NEXT: jsTbl.set(0, a): accepted, jsTbl.get(0) === a: true
;; CHECK-NEXT: jsTbl.grow(1) -> 1, jsTbl.get(1): null
;; CHECK-NEXT: done
