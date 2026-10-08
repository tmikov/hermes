;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; An i64 global's value used to be a scalar `int64_t i64Value_` field, so
;; every write to it was an unconditional, infallible, non-allocating store.
;; It is now a BigIntPrimitive in the single traced `value_` slot, so all four
;; writers -- the WebAssembly.Global constructor, the public `.value` setter,
;; wasmMakeGlobal's export snapshot and wasmGlobalSet's internal setter --
;; MATERIALIZE a BigInt. That is a safepoint: the destination global must be
;; rooted across it, and the slot must be registered in the cell's metadata or
;; the BigInt is collected out from under it.
;;
;; Existing i64 coverage checks values, which a broken store can still get
;; right by accident on a heap that does not move. This test drives each store
;; MANY times with other allocation interleaved, so under
;; -gc-sanitize-handles=1 -- where every allocation relocates the heap -- an
;; unrooted destination or an untraced slot is a wrong read-back, not a
;; coincidence. Arithmetic wrapping (2n**100n + 5n -> 5n) is asserted
;; separately at the bottom: it exercises truncation, not allocation.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js -gc-sanitize-handles=1 %S/e2e-i64-global-store-path-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; A MUTABLE i64 import must be satisfied by a real WebAssembly.Global, and
  ;; the host hands over a snapshot one, so the module's global.set below
  ;; reaches wasmGlobalSet's non-live i64 branch -- the internal allocating
  ;; store -- rather than a live global's setter closure.
  (import "e" "acc" (global $acc (mut i64)))

  ;; An IMMUTABLE i64 import, which is the only shape in which the SNAPSHOT
  ;; READER's answer is observable from inside a module. A mutable import
  ;; keeps the Global OBJECT and fetches no value at link time, so it would
  ;; pass even if the snapshot reader handed back a wrong value; an immutable
  ;; one fetches with wasmGlobalGet and snapshots the answer into the frame
  ;; slot the module reads.
  (import "e" "konst" (global $konst i64))

  ;; A module-local i64 global that is exported: each instantiation wraps it
  ;; with wasmMakeGlobal, whose snapshot i64 arm is the fourth allocating
  ;; store.
  (global $seed i64 (i64.const 0x0123456789abcdef))
  (export "seed" (global $seed))

  (func (export "add") (param i64)
    global.get $acc
    local.get 0
    i64.add
    global.set $acc)

  ;; Return the halves separately, so a lost or stale upper word shows up
  ;; directly rather than only through 64-bit arithmetic.
  (func (export "lo") (result i32) global.get $acc i32.wrap_i64)
  (func (export "hi") (result i32)
    global.get $acc
    i64.const 32
    i64.shr_u
    i32.wrap_i64)

  (func (export "konst_lo") (result i32) global.get $konst i32.wrap_i64)
  (func (export "konst_hi") (result i32)
    global.get $konst
    i64.const 32
    i64.shr_u
    i32.wrap_i64))

;; Each of the four allocating stores, driven repeatedly.
;; CHECK: constructor stores intact: true
;; CHECK-NEXT: public setter stores intact: true
;; CHECK-NEXT: public setter final value: 1073741824250
;; CHECK-NEXT: internal setter stores intact: true
;; CHECK-NEXT: internal setter final lo/hi: 250/250
;; CHECK-NEXT: wasmMakeGlobal snapshot stores intact: true
;; CHECK-NEXT: wasmMakeGlobal snapshot value: 81985529216486895

;; The immutable import: what the link-time wasmGlobalGet read out of the
;; slot, snapshotted into the module's frame. 0x0123456789abcdef is
;; lo=0x89abcdef (-1985229329 as a signed i32) and hi=0x01234567.
;; CHECK-NEXT: immutable import lo/hi: -1985229329/19088743

;; Wrapping to 64 bits, which happens at store time so the slot stays
;; canonical. Separate from the store-path assertions above: it exercises
;; truncation and not allocation or rooting.
;; CHECK-NEXT: constructor wraps: 5
;; CHECK-NEXT: public setter wraps: 7
;; CHECK-NEXT: public setter wraps negative: -1
;; CHECK-NEXT: done
