;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; An exported memory must BE the module's linear memory, not a copy of it.
;;
;; A defined memory used to be a bare ArrayBuffer, and the export loop
;; constructed a fresh WebAssembly.Memory with the same declared limits. The
;; two were unrelated: writes through exports.mem.buffer were invisible to the
;; module and vice versa, and memory.grow replaced the module's buffer while
;; the exported object kept the old, smaller one. Real toolchain output does
;; not survive that -- it hands out pointers into the memory and expects the
;; embedder to read them back, and it grows the memory while doing so.
;;
;; The module's typed-array views are now built over a real
;; WebAssembly.Memory's buffer, the export publishes that same object, and
;; memory.grow installs the grown buffer back onto it.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-memory-alias-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (memory (export "mem") 1 4)
  (func (export "poke") (param i32 i32)
    local.get 0
    local.get 1
    i32.store)
  (func (export "peek") (param i32) (result i32)
    local.get 0
    i32.load)
  (func (export "grow") (param i32) (result i32)
    local.get 0
    memory.grow))

;; Both directions must be visible: one buffer, not two.
;; CHECK: wasm write visible to JS: true
;; CHECK-NEXT: JS write visible to wasm: true

;; And the exported object must follow memory.grow rather than keeping the
;; buffer it was created with.
;; CHECK-NEXT: grow returned old pages: 1
;; CHECK-NEXT: exported buffer followed grow: true
;; CHECK-NEXT: data survived grow: true
;; CHECK-NEXT: done
