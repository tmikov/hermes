;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Advanced tests for cross-module table import wiring:
;;   1. Element segment applied to an imported table (importer writes
;;      to the shared table, both modules see the result).
;;   2. Imported table (index 0) coexists with a locally-defined table
;;      (index 1) — correct indexing, no off-by-one.
;;   3. Grow from exporter, visible in importer (with multiple tables).

;; REQUIRES: wasm

;; RUN: %wat2wasm %S/e2e-table-import-advanced-exporter.wat_ -o %t-exporter.wasm && %hermesc --wasm -emit-binary -out %t-exporter.hbc %t-exporter.wasm && %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-table-import-advanced-driver.js_ -- %t-exporter.hbc %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; Type used by call_indirect.
  (type $void_to_i32 (func (result i32)))

  ;; Table 0: imported from exporter (initial=3, max=10).
  (import "exporter" "tbl" (table 3 10 funcref))

  ;; Table 1: locally defined (initial=2).
  (table $local_tbl 2 funcref)

  ;; Functions defined in this module.
  (func $f300 (result i32) i32.const 300)
  (func $f400 (result i32) i32.const 400)

  ;; Active element segment: place $f300 at index 2 of the IMPORTED table.
  ;; This overwrites what was in the exporter's slot [2] (which was empty).
  ;; Tests that element segments work on imported tables whose arrays
  ;; were wired during import validation (not freshly created in createTables).
  (elem (table 0) (i32.const 2) func $f300)

  ;; Active element segment: place $f400 at index 0 of the LOCAL table.
  (elem (table $local_tbl) (i32.const 0) func $f400)

  ;; --- Imported table operations (table 0) ---

  (func (export "imported_size") (result i32)
    table.size 0)

  (func (export "imported_call_at") (param i32) (result i32)
    (call_indirect 0 (type $void_to_i32) (local.get 0)))

  ;; --- Local table operations (table 1) ---

  (func (export "local_size") (result i32)
    table.size $local_tbl)

  (func (export "local_call_at") (param i32) (result i32)
    (call_indirect $local_tbl (type $void_to_i32) (local.get 0)))
)

;; -- Test 1: element segment on imported table --
;; Exporter placed f100=100 at [0], f200=200 at [1]. Slot [2] was empty.
;; Importer's element segment placed f300=300 at [2].
;; CHECK: exporter size: 3
;; CHECK-NEXT: importer imported_size: 3
;; CHECK-NEXT: exporter call_at(0): 100
;; CHECK-NEXT: exporter call_at(1): 200
;; CHECK-NEXT: exporter call_at(2): 300
;; CHECK-NEXT: imported_call_at(2): 300
;;
;; -- Test 2: local table is independent --
;; CHECK-NEXT: local_size: 2
;; CHECK-NEXT: local_call_at(0): 400
;;
;; -- Test 3: grow from exporter, visible in importer --
;; CHECK-NEXT: exporter grow 2: 3
;; CHECK-NEXT: exporter size: 5
;; CHECK-NEXT: importer imported_size: 5
