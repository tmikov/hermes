;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A MUTABLE imported global is genuinely shared state with the host's
;; WebAssembly.Global -- the module and the host must see each other's writes
;; (that is H12, and e2e-imported-mutable-global.wat is where it is pinned).
;; The way that sharing was implemented was to keep the object and read and
;; write `.value` on it at every global.get and global.set, plus once at
;; instantiation for the link-time snapshot the constant expressions use.
;;
;; `value` is a CONFIGURABLE accessor on WebAssembly.Global.prototype, so all
;; three of those were script-replaceable:
;;
;;   link-time / honest read:                              77
;;   MUTABLE global.get through hijacked accessor:        999
;;   after wasm global.set(5), real global.value:          77   <- swallowed
;;
;; The module observed values the host never wrote and its own writes were
;; discarded. The fix is NOT to snapshot the value -- that is H12 again -- but
;; to reach the same shared internal field without going through a replaceable
;; property: the wasmGlobalGet/wasmGlobalSet builtins brand-check the object
;; and read or write value_/i64Value_ directly. The last four lines of this
;; file are the H12 guard: the sharing must still work in both directions.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-global-import-mutable-hijack-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "g" (global $g (mut i32)))
  (import "e" "b" (global $b (mut i64)))
  (import "e" "f" (global $f (mut f64)))

  (func (export "get") (result i32) global.get $g)
  (func (export "set") (param i32) local.get 0 global.set $g)

  (func (export "get_big_lo") (result i32) global.get $b i32.wrap_i64)
  (func (export "get_big_hi") (result i32)
    global.get $b
    i64.const 32
    i64.shr_u
    i32.wrap_i64)
  (func (export "set_big") (param i64) local.get 0 global.set $b)

  (func (export "get_f") (result f64) global.get $f)
  (func (export "set_f") (param f64) local.get 0 global.set $f))

;; The hijack was installed before instantiation and really was in force: a
;; property read of `.value` answered 999. Without this line every assertion
;; below could hold because nothing was ever replaced.
;; CHECK: hijacked accessor was in force: 999

;; The init read. initializeGlobals() took the link-time snapshot for each
;; mutable import through `.value`, so instantiating the module ran the
;; replaced getter once per mutable global import -- three user-JS calls
;; inside instantiation, each free to answer with anything.
;; CHECK-NEXT: value getter calls during instantiate: 0
;; CHECK-NEXT: value setter calls during instantiate: 0

;; global.get must read the shared internal field, not the accessor.
;; CHECK-NEXT: i32 global.get with hijacked getter: 77
;; CHECK-NEXT: f64 global.get with hijacked getter: 2.5
;; CHECK-NEXT: i64 global.get with hijacked getter lo/hi: 0/1

;; global.set must write the shared internal field. The values below are read
;; back with the accessor RESTORED, so the hijack cannot flatter them.
;; CHECK-NEXT: value getter calls for four global.gets: 0
;; CHECK-NEXT: after wasm set(5) under hijack, real value: 5
;; CHECK-NEXT: after wasm set_f(0.25) under hijack, real value: 0.25
;; CHECK-NEXT: after wasm set_big(-1) under hijack, real value: -1
;; CHECK-NEXT: value setter calls for three global.sets: 0

;; H12 guard: with nothing replaced, the sharing still works both ways.
;; CHECK-NEXT: honest: host writes 100, wasm reads: 100
;; CHECK-NEXT: honest: wasm writes 7, host reads: 7
;; CHECK-NEXT: honest i64: host writes 2^33, wasm reads lo/hi: 0/2
;; CHECK-NEXT: honest i64: wasm writes -2, host reads: -2
;; CHECK-NEXT: done
