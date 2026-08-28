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
;; shared. The constructor now publishes its backing arrays under
;; __wasm_funcs__/__wasm_types__ -- the very objects get/set/grow/length
;; operate on -- so the import wiring picks them up, and grow grows in
;; place like the module-side table.grow does.
;;
;; An entry set from JS carries no interned Wasm type, so call_indirect
;; refuses it ("type mismatch", the fail-closed behavior table.set already
;; has module-side).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>/dev/null && %hermes -Xhermes-internal-test-methods %S/e2e-table-jsapi-import-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "t" (table 2 funcref))
  (func $f42 (result i32) (i32.const 42))
  (elem (i32.const 0) $f42)
  (func (export "size") (result i32) table.size)
  (func (export "call0") (result i32) (call_indirect (result i32) (i32.const 0)))
  (func (export "call1") (result i32) (call_indirect (result i32) (i32.const 1)))
  (func (export "grow2") (result i32) (table.grow (ref.null func) (i32.const 2))))

;; The module observes the supplied table's actual size, and its element
;; segment lands in the array tbl.get reads.
;; CHECK: initial length: 2
;; CHECK-NEXT: module size: 2
;; CHECK-NEXT: elem entry via tbl.get: function
;; CHECK-NEXT: call0: 42

;; A JS-set entry is visible to the module; carrying no type id, it fails
;; closed under call_indirect.
;; CHECK-NEXT: call1 after JS set: Error: call_indirect: type mismatch

;; Growth is visible in both directions, through the same array objects.
;; CHECK-NEXT: JS grow -> 2, module size: 5
;; CHECK-NEXT: module grow -> 5, tbl.length: 7
;; CHECK-NEXT: done
