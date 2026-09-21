;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for linear memory load/store operations.
;; Compiles to .hbc and runs via hermescli.loadHBC to verify results.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-memory-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (memory 1)

  ;; Store i32 at address 0, load it back.
  (func $store_load_i32 (export "store_load_i32") (param i32) (result i32)
    i32.const 0
    local.get 0
    i32.store
    i32.const 0
    i32.load
  )

  ;; Store i32 at address 0, load back the lower byte unsigned.
  (func $store_load8_u (export "store_load8_u") (param i32) (result i32)
    i32.const 0
    local.get 0
    i32.store
    i32.const 0
    i32.load8_u
  )

  ;; Store i32 at address 0, load back the lower byte signed.
  (func $store_load8_s (export "store_load8_s") (param i32) (result i32)
    i32.const 0
    local.get 0
    i32.store
    i32.const 0
    i32.load8_s
  )

  ;; Store i32 at address 0, load16_u (lower 2 bytes unsigned).
  (func $store_load16_u (export "store_load16_u") (param i32) (result i32)
    i32.const 0
    local.get 0
    i32.store
    i32.const 0
    i32.load16_u
  )

  ;; Store i32 at address 0, load16_s (lower 2 bytes signed).
  (func $store_load16_s (export "store_load16_s") (param i32) (result i32)
    i32.const 0
    local.get 0
    i32.store
    i32.const 0
    i32.load16_s
  )

  ;; Store via i32.store8 then load8_u.
  (func $store8_load8 (export "store8_load8") (param i32) (result i32)
    i32.const 0
    local.get 0
    i32.store8
    i32.const 0
    i32.load8_u
  )

  ;; Store via i32.store16 then load16_u.
  (func $store16_load16 (export "store16_load16") (param i32) (result i32)
    i32.const 0
    local.get 0
    i32.store16
    i32.const 0
    i32.load16_u
  )

  ;; f64 store and load: preserves full precision.
  (func $store_load_f64 (export "store_load_f64") (param f64) (result f64)
    i32.const 0
    local.get 0
    f64.store
    i32.const 0
    f64.load
  )

  ;; f32 store and load.
  (func $store_load_f32 (export "store_load_f32") (param f32) (result f32)
    i32.const 0
    local.get 0
    f32.store
    i32.const 0
    f32.load
  )

  ;; Access at a non-zero offset.
  (func $load_at_offset (export "load_at_offset") (param i32 i32) (result i32)
    ;; Store param 0 at address 100.
    i32.const 100
    local.get 0
    i32.store
    ;; Store param 1 at address 104.
    i32.const 104
    local.get 1
    i32.store
    ;; Load from address 104.
    i32.const 104
    i32.load
  )

  ;; Memory boundary test: write/read at last valid i32 address.
  ;; 1 page = 65536 bytes. Last valid i32 address = 65532.
  (func $boundary (export "boundary") (param i32) (result i32)
    i32.const 65532
    local.get 0
    i32.store
    i32.const 65532
    i32.load
  )
)

;; CHECK: store_load_i32(42) = 42
;; CHECK-NEXT: store_load_i32(-1) = -1
;; CHECK-NEXT: store_load8_u(0xFF) = 255
;; CHECK-NEXT: store_load8_s(0xFF) = -1
;; CHECK-NEXT: store_load8_u(0x80) = 128
;; CHECK-NEXT: store_load8_s(0x80) = -128
;; CHECK-NEXT: store_load16_u(0xFFFF) = 65535
;; CHECK-NEXT: store_load16_s(0xFFFF) = -1
;; CHECK-NEXT: store_load16_u(0x8000) = 32768
;; CHECK-NEXT: store_load16_s(0x8000) = -32768
;; CHECK-NEXT: store8_load8(0xFF) = 255
;; CHECK-NEXT: store8_load8(0x100) = 0
;; CHECK-NEXT: store16_load16(0xFFFF) = 65535
;; CHECK-NEXT: store16_load16(0x10000) = 0
;; CHECK-NEXT: f64 = 3.141592653589793
;; CHECK-NEXT: f32 round-trip ok
;; CHECK-NEXT: load_at_offset = 99
;; CHECK-NEXT: boundary = 12345
