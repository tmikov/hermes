;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for bulk table operations: table.fill, table.copy,
;; table.init, and elem.drop.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-bulk-table-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (type $void_to_i32 (func (result i32)))

  (table 10 funcref)

  ;; Segment 0: active, placed at table offset 0 during instantiation.
  (elem (i32.const 0) $f10 $f20)

  ;; Segment 1: passive, available for table.init.
  (elem func $f30 $f40 $f50)

  (func $f10 (result i32) i32.const 10)
  (func $f20 (result i32) i32.const 20)
  (func $f30 (result i32) i32.const 30)
  (func $f40 (result i32) i32.const 40)
  (func $f50 (result i32) i32.const 50)

  ;; Call the function at a given table index via call_indirect.
  (func $call_at (export "call_at") (param i32) (result i32)
    (call_indirect (type $void_to_i32) (local.get 0))
  )

  ;; table.fill: fill count entries at idx with the function at table[srcIdx].
  ;; Uses table.get to obtain a funcref from the table.
  (func $fill_from (export "fill_from") (param i32 i32 i32)
    (table.fill 0 (local.get 0) (table.get 0 (local.get 1)) (local.get 2))
  )

  ;; table.copy within same table: dst, src, count.
  (func $copy (export "copy") (param i32 i32 i32)
    (table.copy 0 0 (local.get 0) (local.get 1) (local.get 2))
  )

  ;; table.init from passive segment 1: dst, src, count.
  (func $init_seg1 (export "init_seg1") (param i32 i32 i32)
    (table.init 1 (local.get 0) (local.get 1) (local.get 2))
  )

  ;; elem.drop segment 1.
  (func $drop_seg1 (export "drop_seg1")
    (elem.drop 1)
  )

  ;; table.init with n=0 from segment 0 (active, thus dropped after
  ;; instantiation). Per spec, n=0 should succeed even for dropped segments.
  (func $init_seg0_zero (export "init_seg0_zero")
    (table.init 0 (i32.const 0) (i32.const 0) (i32.const 0))
  )

  ;; table.size.
  (func $get_size (export "get_size") (result i32)
    table.size 0
  )
)

;; CHECK: === table.copy ===
;; CHECK-NEXT: copy 4,0,2: ok
;; CHECK-NEXT: call_at 4: 10
;; CHECK-NEXT: call_at 5: 20
;; CHECK-NEXT: === table.init ===
;; CHECK-NEXT: init_seg1 7,0,3: ok
;; CHECK-NEXT: call_at 7: 30
;; CHECK-NEXT: call_at 8: 40
;; CHECK-NEXT: call_at 9: 50
;; CHECK-NEXT: === table.fill (no call_indirect) ===
;; CHECK-NEXT: fill_from 2,0,3: ok
;; CHECK-NEXT: size: 10
;; CHECK-NEXT: === elem.drop + init n=0 ===
;; CHECK-NEXT: drop_seg1 ok
;; CHECK-NEXT: init_seg1 n=0 ok
;; CHECK-NEXT: init_seg1 n=1 trapped
;; CHECK-NEXT: === active segment dropped ===
;; CHECK-NEXT: init_seg0_zero ok
;; CHECK-NEXT: === OOB checks ===
;; CHECK-NEXT: fill oob trapped
;; CHECK-NEXT: copy oob trapped
;; CHECK-NEXT: init oob trapped
;; CHECK-NEXT: done
