;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; An exported table must BE the module's table, not a disconnected copy.
;; A defined table was backed by plain arrays while the export constructed a
;; fresh WebAssembly.Table and only copied the module's arrays onto its
;; __wasm_funcs__/__wasm_types__ properties, leaving the object's own
;; storage empty: exports.tbl.get returned null where the module had a
;; function, and exports.tbl.grow and the module's table.grow each moved
;; only their own side. The module's defined funcref table is now backed by
;; the very WebAssembly.Table it exports, so get/set/grow/length and the
;; module's own table ops operate on one shared storage.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>/dev/null && %hermes -Xhermes-internal-test-methods %S/e2e-table-export-alias-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (table (export "tbl") 3 funcref)
  (func $f42 (result i32) (i32.const 42))
  (elem (i32.const 0) $f42)
  (func (export "call") (param i32) (result i32)
    (call_indirect (result i32) (local.get 0)))
  (func (export "size") (result i32) table.size)
  (func (export "grow2") (result i32)
    (table.grow (ref.null func) (i32.const 2))))

;; The exported object is the module's table: its element segment is visible
;; through tbl.get, and its length matches the module's table.size.
;; CHECK: exported is a Table: true
;; CHECK-NEXT: tbl.get(0): function, module call(0): 42
;; CHECK-NEXT: tbl.length: 3, module size: 3

;; Growth is visible in both directions across the shared storage.
;; CHECK-NEXT: after JS grow(2): tbl.length 5, module size 5
;; CHECK-NEXT: after module grow2: tbl.length 7, module size 7

;; An entry never set is uninitialized (null in a WebAssembly.Table), which
;; call_indirect traps as "uninitialized element", not "type mismatch".
;; CHECK-NEXT: call(1) on empty slot: uninitialized element
;; CHECK-NEXT: done
