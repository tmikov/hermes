;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Marshalling through the per-module return buffer, which had four defects.
;;
;; The buffer is a Uint32Array, so everything read back out of it is
;; unsigned and untyped, and every read in this commit narrows it with
;; AsInt32Inst -- except the export wrapper's i64 parameter unmarshal, which
;; was missed. That made i32.wrap_i64(-1n) yield 4294967295 instead of -1,
;; and fed an `any` into type-checked i32 arithmetic so that a perfectly
;; valid module failed to compile at all.
;;
;; The multi-value import trampoline stored each result at its own offset,
;; but converting a BigInt writes lo/hi through buffer slots 0 and 1 as
;; scratch. Any i64 result at a non-zero offset therefore destroyed whatever
;; was already stored at bytes 0-7 -- for (result i32 i64), result 0, which
;; then read back as the i64's low word.
;;
;; And the buffer's float view was loaded only for functions that return
;; through the buffer themselves, though the reads it feeds are of a
;; CALLEE's results: a caller returning a single f64 while calling a
;; multi-value f64 callee dereferenced a null.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-i64-retbuf-marshal-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "mv" (func $mv (result i32 i64)))

  ;; i64 parameter arriving from JS as a BigInt.
  (func (export "wrap") (param i64) (result i32)
    local.get 0
    i32.wrap_i64)

  ;; Same, with arithmetic after the wrap: this is what needs the unmarshal
  ;; to produce a typed i32 rather than an `any`.
  (func (export "wrap_add") (param i64) (result i32)
    local.get 0
    i32.wrap_i64
    i32.const 1
    i32.add)

  ;; Both halves of a multi-value import result.
  (func (export "mv_i32") (result i32)
    call $mv
    drop)
  (func (export "mv_i64") (result i64) (local $x i64)
    call $mv
    local.set $x
    drop
    local.get $x)

  ;; A caller that does NOT return through the buffer, calling a callee that
  ;; returns two f64s through it.
  (func $mvf (result f64 f64)
    f64.const 1.5
    f64.const 2.5)
  (func (export "mvf_sum") (result f64)
    call $mvf
    f64.add)

  ;; Multi-value returned to JS -- the capability the return buffer exists
  ;; for, and which the thread-local it replaced silently got wrong.
  (func (export "pair") (result i32 i32)
    i32.const 11
    i32.const 22))

;; An i64 parameter is signed once wrapped: -1n must not come back as
;; 4294967295, and 2^32-1 is the same i64 low word, so it must agree.
;; CHECK: wrap(-1n) = -1
;; CHECK-NEXT: wrap(0xFFFFFFFFn) = -1
;; CHECK-NEXT: wrap(5n) = 5

;; The arithmetic form has to compile at all, and agree.
;; CHECK-NEXT: wrap_add(-1n) = 0
;; CHECK-NEXT: wrap_add(5n) = 6

;; Result 0 must survive the conversion of result 1.
;; CHECK-NEXT: mv_i32() = 42
;; CHECK-NEXT: mv_i64() = 7

;; The callee's float results must be readable from a caller that has no
;; buffer of its own.
;; CHECK-NEXT: mvf_sum() = 4

;; CHECK-NEXT: pair() = [11,22]
;; CHECK-NEXT: done
