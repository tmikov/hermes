;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for bulk memory operations: memory.fill, memory.copy,
;; memory.init, and data.drop.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-bulk-memory-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (memory 1)
  ;; Segment 0: active, copied to offset 0 during instantiation.
  (data (i32.const 0) "Hello, World!")
  ;; Segment 1: passive, available for memory.init.
  (data "\01\02\03\04\05")

  ;; Read a byte from memory.
  (func $get_byte (export "get_byte") (param i32) (result i32)
    (i32.load8_u (local.get 0))
  )

  ;; memory.fill: fill 10 bytes at dest with value.
  (func $fill (export "fill") (param i32 i32 i32)
    (memory.fill (local.get 0) (local.get 1) (local.get 2))
  )

  ;; memory.copy: copy n bytes from src to dest.
  (func $copy (export "copy") (param i32 i32 i32)
    (memory.copy (local.get 0) (local.get 1) (local.get 2))
  )

  ;; memory.init from passive segment 1.
  (func $init_seg1 (export "init_seg1") (param i32 i32 i32)
    (memory.init 1 (local.get 0) (local.get 1) (local.get 2))
  )

  ;; data.drop segment 1.
  (func $drop_seg1 (export "drop_seg1")
    (data.drop 1)
  )

  ;; memory.init with n=0 from segment 0 (which is active, thus dropped after
  ;; instantiation). Per spec, n=0 should succeed even for dropped segments.
  (func $init_seg0_zero (export "init_seg0_zero")
    (memory.init 0 (i32.const 0) (i32.const 0) (i32.const 0))
  )
)

;; CHECK: === memory.fill ===
;; CHECK-NEXT: byte 20: 255
;; CHECK-NEXT: byte 21: 255
;; CHECK-NEXT: byte 29: 255
;; CHECK-NEXT: byte 30: 0
;; CHECK-NEXT: === memory.copy ===
;; CHECK-NEXT: byte 100: 72
;; CHECK-NEXT: byte 101: 101
;; CHECK-NEXT: byte 104: 111
;; CHECK-NEXT: === memory.copy overlap (src < dest) ===
;; CHECK-NEXT: byte 50: 1
;; CHECK-NEXT: byte 51: 2
;; CHECK-NEXT: byte 52: 1
;; CHECK-NEXT: byte 53: 2
;; CHECK-NEXT: byte 54: 3
;; CHECK-NEXT: byte 55: 4
;; CHECK-NEXT: === memory.init ===
;; CHECK-NEXT: byte 200: 2
;; CHECK-NEXT: byte 201: 3
;; CHECK-NEXT: byte 202: 4
;; CHECK-NEXT: === data.drop + init n=0 ===
;; CHECK-NEXT: drop_seg1 ok
;; CHECK-NEXT: init_seg1 n=0 ok
;; CHECK-NEXT: init_seg1 n=1 trapped: memory.init: out of bounds data segment access
;; CHECK-NEXT: === active segment dropped ===
;; CHECK-NEXT: init_seg0_zero ok
;; CHECK-NEXT: done
