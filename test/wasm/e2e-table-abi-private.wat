;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The table linking ABI was PUBLISHED to script. A WebAssembly.Table carried
;; six ordinary own properties -- __wasm_funcs__ / __wasm_types__ /
;; __wasm_exported__ (its backing storage) and __wasm_type__ / __wasm_min__ /
;; __wasm_max__ (the metadata the link path compared against) -- all writable
;; and all enumerable. Three separate holes followed from that:
;;
;;   * __wasm_funcs__ handed script the module's INTERNAL closures, whose
;;     calling convention is not the JS one. `tbl.__wasm_funcs__[0](5n)`
;;     reached an i64 parameter as a raw double and aborted the VM
;;     ("Assertion `isDouble()' failed"), reachable from ordinary JavaScript.
;;   * __wasm_types__ was writable, so script could stamp any interned type id
;;     onto any slot and make call_indirect's check pass for a function of a
;;     different signature.
;;   * a plain object literal carrying those six names LINKED as a table, so
;;     script chose the storage a module ran on outright.
;;
;; All three close the same way: nothing is published, and the link path
;; brand-checks with dyn_vmcast<JSWebAssemblyTable> and reads the internal
;; fields. A brand check is strictly stronger than `instanceof`, which a forged
;; prototype chain satisfies -- pinned below.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %wat2wasm %S/e2e-table-abi-private-externref.wat_ -o %t-ext.wasm && %hermesc --wasm -emit-binary -out %t-ext.hbc %t-ext.wasm && %wat2wasm %S/e2e-table-abi-private-consumer.wat_ -o %t-cons.wasm && %hermesc --wasm -emit-binary -out %t-cons.hbc %t-cons.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-table-abi-private-driver.js_ -- %t.hbc %t-ext.hbc %t-cons.hbc | %FileCheck --match-full-lines %s

(module
  (type $i64_i64 (func (param i64) (result i64)))
  ;; Slot 2 is deliberately left unwritten by the element segment.
  (table (export "tbl") 3 funcref)
  (elem (i32.const 0) $dbl $seven)
  ;; An i64 parameter is what made the published closure lethal: the internal
  ;; convention passes it as a lo/hi pair, so a JS BigInt arriving there was
  ;; read as a double.
  (func $dbl (param i64) (result i64) (local.get 0) (local.get 0) (i64.add))
  (func $seven (result i32) (i32.const 7))
  (func (export "callAsI64") (param i32) (param i64) (result i64)
    (local.get 1) (local.get 0) (call_indirect (type $i64_i64))))

;; A WebAssembly.Table has no own properties at all -- which is also what the
;; spec says of it, so this is a conformance fix as much as a security one.
;; CHECK: exported tbl own props: []
;; CHECK-NEXT: exported tbl JSON: {}
;; CHECK-NEXT: exported tbl symbols: []
;; CHECK-NEXT: JS-API tbl own props: []

;; The three arrays are gone, so the abort is not merely fixed but unreachable:
;; the property read yields undefined and indexing it is an ordinary TypeError.
;; CHECK-NEXT: tbl.__wasm_funcs__: undefined
;; CHECK-NEXT: tbl.__wasm_funcs__[0](5n): TypeError

;; And the spec route works, which is the whole point of having closed the
;; other one: tbl.get hands out the Exported Function, which takes a BigInt.
;; CHECK-NEXT: tbl.get(0)(5n): 10

;; Forging __wasm_types__ was the other half of the hole. There is no longer a
;; types array to write, and a slot's type id can only be derived from the
;; Exported Function in it, so a wrong-signature call still traps.
;; CHECK-NEXT: callAsI64(1, 5n): trap: call_indirect: type mismatch

;; A plain object shaped exactly like the old published ABI must not link --
;; both an empty-shaped one and one carrying a genuine table's own arrays,
;; which is the shape that used to give script the module's real storage.
;; CHECK-NEXT: forged literal: LinkError
;; CHECK-NEXT: forged literal, arrays borrowed from a real table: LinkError

;; Nor an object that INHERITS from a genuine Table: `instanceof` says yes to
;; this one, so `instanceof` would not have been enough.
;; CHECK-NEXT: Object.create(realTable) instanceof WebAssembly.Table: true
;; CHECK-NEXT: Object.create(realTable) as import: LinkError

;; Nor a Proxy wrapping a genuine Table.
;; CHECK-NEXT: Proxy(realTable) as import: LinkError

;; A genuine table still links, both from a module and from the JS API, so the
;; check is not simply refusing everything.
;; CHECK-NEXT: genuine module table: linked, size = 3
;; CHECK-NEXT: genuine JS-API table: linked, size = 4

;; A module declaring an EXTERNREF table import cannot be satisfied by a
;; funcref table. The element type used to be decided by a __wasm_type__ string
;; on the supplied object, while the STORAGE came from whatever satisfied the
;; import -- so pairing 'table:externref' with a genuine funcref table's arrays
;; skipped the funcref brand check on every write, and table.get then handed
;; out an arbitrary object as a funcref. The declaration is now checked against
;; what the engine can actually build, and nothing can build an externref
;; table, so the declaration is unsatisfiable.
;; The message says which of the two things went wrong. Reporting a genuine
;; table as "is not a WebAssembly.Table" would be the same false-message class
;; as reporting a limits failure that way.
;; CHECK-NEXT: externref-declared import of a funcref table: LinkError: import e.t declares a non-funcref table, which nothing can satisfy: WebAssembly.Table builds only funcref tables
;; CHECK-NEXT: externref-declared import of a forged externref literal: LinkError

;; A slot no one has written reads as null, the spec's DefaultValue(funcref).
;; This is the answer that used to depend on whether the slot held an explicit
;; null or a never-written hole -- distinguishable only through the published
;; arrays. With those gone both spell the same thing and only this is
;; observable, on a module-defined table and on a JS-API one alike.
;;
;; Note what these two lines do NOT pin. On a WebAssembly.Table every slot in
;; range holds an explicit null (the constructor clears them through the
;; funnel; grow fills the new ones), so neither line reaches the
;; empty-slot-reads-as-null mapping -- the two are a matched pair and each
;; alone would compensate for the other's absence. That mapping has its own
;; test, on an externref table, whose storage is holes throughout: see the
;; frozen-storage cases in e2e-table-slot-invariant.wat. It is the same code in
;; both places on purpose (readWasmTableSlot), which is what makes that test
;; cover this path.
;; CHECK-NEXT: module tbl.get(2) [never written]: null
;; CHECK-NEXT: fresh JS-API tbl.get(0): null

;; A module builds its own funcref table with globalThis.WebAssembly.Table,
;; which script can replace -- so the brand check runs on that too, and is
;; branched on for the DIAGNOSTIC: without the branch the null result reaches
;; an indexed load and reports "Cannot read property 0 of null" from inside
;; generated code, naming nothing.
;; CHECK-NEXT: replaced WebAssembly.Table: LinkError: WebAssembly.Table did not construct a table for this module's table 0
;; CHECK-NEXT: done
