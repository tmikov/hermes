;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for memory.size and memory.grow instructions.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-memory-size-grow-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (memory 1 4)

  ;; Return current memory size in pages.
  (func (export "size") (result i32)
    memory.size
  )

  ;; Grow memory by delta pages. Return old page count or -1 on failure.
  (func (export "grow") (param i32) (result i32)
    local.get 0
    memory.grow
  )

  ;; Store an i32 value at a given byte address.
  (func (export "store") (param i32 i32)
    local.get 0
    local.get 1
    i32.store
  )

  ;; Load an i32 value from a given byte address.
  (func (export "load") (param i32) (result i32)
    local.get 0
    i32.load
  )
)

;; CHECK: initial size = 1
;; CHECK-NEXT: grow(1) = 1
;; CHECK-NEXT: size after grow = 2
;; CHECK-NEXT: store at 65536 ok
;; CHECK-NEXT: load from 65536 = 42
;; CHECK-NEXT: grow(1) = 2
;; CHECK-NEXT: size after second grow = 3
;; CHECK-NEXT: grow(2) = -1
;; CHECK-NEXT: size unchanged = 3
;; CHECK-NEXT: grow(1) = 3
;; CHECK-NEXT: size = 4
;; CHECK-NEXT: grow(1) beyond max = -1
