;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-unaligned-access-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s
;; CHECK: i32 odd addr: PASS
;; CHECK: i32 align2: PASS
;; CHECK: f64 odd addr: PASS
;; CHECK: aligned-store unaligned-load: PASS
;; CHECK: unaligned-store aligned-load: PASS
;; CHECK: i16s unaligned: PASS
;; CHECK: i16u unaligned: PASS
;; CHECK: f32 odd addr: PASS

(module
  (memory (export "memory") 1)

  ;; Store an i32 at a given byte address using unaligned store (align=1).
  (func (export "store_i32_u1") (param i32 i32)
    (i32.store align=1 (local.get 0) (local.get 1))
  )

  ;; Load an i32 from a given byte address using unaligned load (align=1).
  (func (export "load_i32_u1") (param i32) (result i32)
    (i32.load align=1 (local.get 0))
  )

  ;; Store an i32 using align=2 (2-byte aligned).
  (func (export "store_i32_u2") (param i32 i32)
    (i32.store align=2 (local.get 0) (local.get 1))
  )

  ;; Load an i32 using align=2 (2-byte aligned).
  (func (export "load_i32_u2") (param i32) (result i32)
    (i32.load align=2 (local.get 0))
  )

  ;; Store an f64 at a given byte address using unaligned store (align=1).
  (func (export "store_f64_u1") (param i32 f64)
    (f64.store align=1 (local.get 0) (local.get 1))
  )

  ;; Load an f64 from a given byte address using unaligned load (align=1).
  (func (export "load_f64_u1") (param i32) (result f64)
    (f64.load align=1 (local.get 0))
  )

  ;; Store an i32 using aligned (natural) store for reference.
  (func (export "store_i32_aligned") (param i32 i32)
    (i32.store (local.get 0) (local.get 1))
  )

  ;; Load an i32 using aligned (natural) load for reference.
  (func (export "load_i32_aligned") (param i32) (result i32)
    (i32.load (local.get 0))
  )

  ;; Store an i16 using unaligned store (align=1).
  (func (export "store_i16_u1") (param i32 i32)
    (i32.store16 align=1 (local.get 0) (local.get 1))
  )

  ;; Load a signed i16 using unaligned load (align=1).
  (func (export "load_i16s_u1") (param i32) (result i32)
    (i32.load16_s align=1 (local.get 0))
  )

  ;; Load an unsigned i16 using unaligned load (align=1).
  (func (export "load_i16u_u1") (param i32) (result i32)
    (i32.load16_u align=1 (local.get 0))
  )

  ;; Store an f32 using unaligned store (align=1).
  (func (export "store_f32_u1") (param i32 f32)
    (f32.store align=1 (local.get 0) (local.get 1))
  )

  ;; Load an f32 using unaligned load (align=1).
  (func (export "load_f32_u1") (param i32) (result f32)
    (f32.load align=1 (local.get 0))
  )
)
