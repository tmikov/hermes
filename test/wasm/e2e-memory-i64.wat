;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for i64 linear memory load/store operations.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-memory-i64-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (memory 1)

  ;; i64 store and load round-trip. Returns lo32 of the loaded i64.
  ;; Store 0x0000000100000002 at address 0, load it back.
  (func $i64_store_load_lo (export "i64_store_load_lo") (result i32)
    i32.const 0
    i64.const 0x0000000100000002
    i64.store
    i32.const 0
    i64.load
    ;; Now we have an i64 on the stack. Wrap to get the lo32.
    i32.wrap_i64
  )

  ;; Same as above but extract hi32 via shift.
  (func $i64_store_load_hi (export "i64_store_load_hi") (result i32)
    i32.const 0
    i64.const 0x0000000100000002
    i64.store
    i32.const 0
    i64.load
    ;; Shift right by 32 to get hi32 in the lo position.
    i64.const 32
    i64.shr_u
    i32.wrap_i64
  )

  ;; i64.store8: store lowest byte of i64.
  (func $i64_store8_load8 (export "i64_store8_load8") (result i32)
    i32.const 0
    i64.const 0xAB
    i64.store8
    i32.const 0
    i32.load8_u
  )

  ;; i64.store16: store lowest 16 bits of i64.
  (func $i64_store16_load16 (export "i64_store16_load16") (result i32)
    i32.const 0
    i64.const 0xABCD
    i64.store16
    i32.const 0
    i32.load16_u
  )

  ;; i64.store32: store lowest 32 bits of i64.
  (func $i64_store32_load32 (export "i64_store32_load32") (result i32)
    i32.const 0
    i64.const 0xDEADBEEF
    i64.store32
    i32.const 0
    i32.load
  )

  ;; i64.load8_u: load a byte, zero-extended to i64, return lo32.
  (func $i64_load8_u (export "i64_load8_u") (result i32)
    ;; Store 0xFF at address 0 via i32.store8.
    i32.const 0
    i32.const 255
    i32.store8
    ;; Load as i64 (zero-extended).
    i32.const 0
    i64.load8_u
    i32.wrap_i64
  )

  ;; i64.load8_s: load a byte, sign-extended to i64, return lo32.
  (func $i64_load8_s (export "i64_load8_s") (result i32)
    i32.const 0
    i32.const 255
    i32.store8
    i32.const 0
    i64.load8_s
    i32.wrap_i64
  )

  ;; i64.load16_u: load 16 bits, zero-extended to i64, return lo32.
  (func $i64_load16_u (export "i64_load16_u") (result i32)
    i32.const 0
    i32.const 0x8000
    i32.store16
    i32.const 0
    i64.load16_u
    i32.wrap_i64
  )

  ;; i64.load16_s: load 16 bits, sign-extended to i64, return lo32.
  (func $i64_load16_s (export "i64_load16_s") (result i32)
    i32.const 0
    i32.const 0x8000
    i32.store16
    i32.const 0
    i64.load16_s
    i32.wrap_i64
  )

  ;; i64.load32_u: load 32 bits, zero-extended to i64, return lo32.
  (func $i64_load32_u (export "i64_load32_u") (result i32)
    i32.const 0
    i32.const -1
    i32.store
    i32.const 0
    i64.load32_u
    i32.wrap_i64
  )

  ;; i64.load32_s: load 32 bits, sign-extended to i64. Check hi32 is -1.
  (func $i64_load32_s_hi (export "i64_load32_s_hi") (result i32)
    i32.const 0
    i32.const -1
    i32.store
    i32.const 0
    i64.load32_s
    ;; Shift right by 32 to get hi32.
    i64.const 32
    i64.shr_u
    i32.wrap_i64
  )
)

;; CHECK: i64_store_load_lo = 2
;; CHECK-NEXT: i64_store_load_hi = 1
;; CHECK-NEXT: i64_store8_load8 = 171
;; CHECK-NEXT: i64_store16_load16 = 43981
;; CHECK-NEXT: i64_store32_load32 = -559038737
;; CHECK-NEXT: i64_load8_u = 255
;; CHECK-NEXT: i64_load8_s = -1
;; CHECK-NEXT: i64_load16_u = 32768
;; CHECK-NEXT: i64_load16_s = -32768
;; CHECK-NEXT: i64_load32_u = 4294967295
;; CHECK-NEXT: i64_load32_s_hi = -1
