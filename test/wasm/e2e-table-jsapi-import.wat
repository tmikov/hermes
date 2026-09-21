;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A table imported from the JS API must be SHARED with the module, the way
;; a Wasm-exported table already was. The module used to build fresh arrays
;; and never look at the supplied WebAssembly.Table again: element segments
;; were invisible to tbl.get, tbl.grow was invisible to table.size, and the
;; module's table.grow was invisible to tbl.length -- and tbl.grow REPLACED
;; its backing array, disconnecting the two even if they had started out
;; shared.
;;
;; The sharing is now established by BRAND CHECK, not by publication. The
;; import wiring calls wasmLinkTable, which dyn_vmcasts the supplied object to
;; a JSWebAssemblyTable and hands back its three internal arrays AS THEY STAND
;; -- the very objects get/set/grow/length operate on -- and grow lengthens
;; them in place rather than replacing them.
;;
;; An earlier form of this fix instead had the Table constructor PUBLISH those
;; arrays as __wasm_funcs__/__wasm_types__ properties for the import wiring to
;; read, and this paragraph used to say so. That publication is gone -- and
;; e2e-table-abi-private.wat asserts in this same suite that a JS-API Table has
;; no own properties at all, so the two files contradicted each other. Only the
;; rationale was stale; what this test measures is unchanged.
;;
;; An entry set from JS is a funcref like any other: a plain JS function is
;; refused outright, and an Exported Function is stored WITH its interned type
;; id, so call_indirect accepts it when the signature matches and traps when it
;; does not. It used to be stored with no type id at all, which made every
;; JS-set entry uncallable, and before that with the previous occupant's type
;; id, which made the wrong ones callable.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>/dev/null && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-table-jsapi-import-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "t" (table 2 funcref))
  (func $f42 (result i32) (i32.const 42))
  (elem (i32.const 0) $f42)
  (func (export "size") (result i32) table.size)
  (func (export "call0") (result i32) (call_indirect (result i32) (i32.const 0)))
  (func (export "call1") (result i32) (call_indirect (result i32) (i32.const 1)))
  (func (export "grow2") (result i32) (table.grow (ref.null func) (i32.const 2)))
  ;; A DIFFERENT signature from every other export here, so that a slot which
  ;; kept its old type id can be told apart from one that carried the new one.
  (func (export "id") (param i32) (result i32) (local.get 0)))

;; The module observes the supplied table's actual size, and its element
;; segment lands in the array tbl.get reads.
;; CHECK: initial length: 2
;; CHECK-NEXT: module size: 2
;; CHECK-NEXT: elem entry via tbl.get: function
;; CHECK-NEXT: call0: 42

;; A JS-set entry is visible to the module, and only a real funcref gets in.
;; CHECK-NEXT: JS set of a plain function: TypeError
;; CHECK-NEXT: call1 after JS set: 42
;; CHECK-NEXT: call1 after mismatching JS set: Error: call_indirect: type mismatch

;; Growth is visible in both directions, through the same array objects.
;; CHECK-NEXT: JS grow -> 2, module size: 5
;; CHECK-NEXT: module grow -> 5, tbl.length: 7
;; CHECK-NEXT: done
