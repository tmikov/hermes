;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A mutable global that this module DEFINES and EXPORTS is shared state: the
;; exported WebAssembly.Global and the module's own storage are one cell, so a
;; global.set inside Wasm must be visible through `.value` and a host write to
;; `.value` must be visible to the next global.get. The export used to be a
;; snapshot taken at instantiation, so writes were silently lost in both
;; directions. An immutable export is still a snapshot, which is correct
;; because its value cannot change.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-exported-mutable-global-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (global $counter (export "counter") (mut i32) (i32.const 5))
  (global $ratio (export "ratio") (mut f64) (f64.const 2.5))
  ;; f32 exercises the setter's narrowing arm (emitFround(AsNumberInst)),
  ;; which no other exported-global test drives: a JS write must be rounded
  ;; to float32 precision before either side observes it, not kept as a
  ;; double.
  (global $scale (export "scale") (mut f32) (f32.const 1.5))
  (global $big (export "big") (mut i64) (i64.const 4294967296))
  ;; An immutable export stays a snapshot.
  (global $konst (export "konst") i32 (i32.const 7))
  ;; An immutable i64 export goes through wasmMakeGlobal's snapshot arm,
  ;; which recombines the lo/hi pair into a BigInt and stores it with
  ;; setI64Value -- a different path than the live i64 accessor above, and
  ;; one no other test in this file exercises.
  (global $bigkonst (export "bigkonst") i64 (i64.const 8589934593))
  ;; A module-local mutable global is not exported and must be unaffected.
  (global $local (mut i32) (i32.const 1))
  ;; The SAME global under a second export name. The loop builds one Global
  ;; per export ENTRY, so these are two objects; they must still name one
  ;; storage cell. (Whether they should be the SAME object is a separate,
  ;; deferred spec discrepancy that Node shares -- do not assert `===`.)
  (export "counter2" (global $counter))

  (func (export "get_counter") (result i32) global.get $counter)
  (func (export "bump_counter") (param i32)
    global.get $counter
    local.get 0
    i32.add
    global.set $counter)

  (func (export "get_ratio") (result f64) global.get $ratio)
  (func (export "scale_ratio") (param f64)
    global.get $ratio
    local.get 0
    f64.mul
    global.set $ratio)

  (func (export "get_scale") (result f32) global.get $scale)
  (func (export "bump_scale") (param f32)
    global.get $scale
    local.get 0
    f32.add
    global.set $scale)

  ;; Return the i64 halves separately, so a lost upper word is visible
  ;; directly rather than only through 64-bit arithmetic.
  (func (export "get_big_lo") (result i32) global.get $big i32.wrap_i64)
  (func (export "get_big_hi") (result i32)
    global.get $big
    i64.const 32
    i64.shr_u
    i32.wrap_i64)
  (func (export "add_big") (param i64)
    global.get $big
    local.get 0
    i64.add
    global.set $big)

  (func (export "get_konst") (result i32) global.get $konst)

  (func (export "get_local") (result i32) global.get $local)
  (func (export "bump_local") (param i32)
    global.get $local
    local.get 0
    i32.add
    global.set $local))

;; CHECK: counter starts at 5
;; CHECK-NEXT: after wasm bump, host sees 15
;; CHECK-NEXT: after host set, wasm sees 100
;; CHECK-NEXT: after another wasm bump, host sees 101
;; CHECK-NEXT: ratio starts at 2.5
;; CHECK-NEXT: after wasm scale, host sees 10
;; CHECK-NEXT: after host set, wasm sees 0.5
;; CHECK-NEXT: scale starts at 1.5
;; CHECK-NEXT: after wasm bump, host sees 4
;; CHECK-NEXT: after host set, host sees 1.100000023841858
;; CHECK-NEXT: after host set, wasm sees 1.100000023841858
;; CHECK-NEXT: host value narrowed like Math.fround = true
;; CHECK-NEXT: big starts at 4294967296 bigint
;; CHECK-NEXT: after wasm add, host sees 4294967297
;; CHECK-NEXT: after host set, wasm sees lo/hi = -1/-1
;; CHECK-NEXT: konst is 7
;; CHECK-NEXT: writing konst threw TypeError
;; CHECK-NEXT: bigkonst is 8589934593 bigint
;; CHECK-NEXT: local after bump = 4
;; CHECK-NEXT: counter is a WebAssembly.Global = true
;; CHECK-NEXT: counter has no own properties = true
;; CHECK-NEXT: two export names, one storage cell = true true true
;; CHECK-NEXT: valueOf that mutates then throws: caught TypeError
;; CHECK-NEXT: mutation from the throwing valueOf survived = 777
;; CHECK-NEXT: done
