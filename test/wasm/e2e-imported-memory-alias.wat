;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; An imported memory must BE the embedder's memory, not a copy sized from
;; the metadata it advertises.
;;
;; The module used to allocate a private ArrayBuffer of __wasm_min__ pages
;; and operate on that, so the embedder's WebAssembly.Memory and the module's
;; linear memory were unrelated storage: neither side saw the other's writes,
;; and the size came from a forgeable property rather than from the memory
;; actually supplied. The module's views are now built over the imported
;; object's buffer, memory.grow installs the grown buffer back onto it, and
;; re-exporting the import yields the same object.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-imported-memory-alias-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "m" (memory 1 4))
  (export "m2" (memory 0))
  (func (export "poke") (param i32 i32)
    local.get 0
    local.get 1
    i32.store)
  (func (export "peek") (param i32) (result i32)
    local.get 0
    i32.load)
  (func (export "size") (result i32) memory.size)
  (func (export "grow") (param i32) (result i32)
    local.get 0
    memory.grow))

;; The module must see the memory it was actually given -- both its size and
;; its contents -- not a private buffer.
;; CHECK: module sees 2 pages: true
;; CHECK-NEXT: JS write visible to wasm: true
;; CHECK-NEXT: wasm write visible to JS: true

;; Growth is growth of the embedder's memory.
;; CHECK-NEXT: grow returned old pages: 2
;; CHECK-NEXT: imported object followed grow: true
;; CHECK-NEXT: data survived grow: true

;; And re-exporting an import gives back the very same object.
;; CHECK-NEXT: re-export is the same object: true

;; The buffer is taken from the memory's internal field by the same
;; wasmLinkMemory call that measured it, not from the `buffer` accessor. That
;; accessor is a configurable property of WebAssembly.Memory.prototype, so a
;; module that re-read it could be handed different storage from the one whose
;; page count satisfied the declaration -- and would then write where the
;; embedder cannot see.
;; CHECK-NEXT: hijacked buffer accessor was in force: true
;; CHECK-NEXT: wasm wrote to the real buffer: true
;; CHECK-NEXT: wasm did not write to the decoy: true
;; CHECK-NEXT: done
