;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; An i64 global was exported by wrapping it in a WebAssembly.Global, which
;; stored its value as a double and so could only carry the low 32 bits. The
;; upper word was discarded silently: a global holding 0x100000000 exported as
;; 0. The import direction was worse -- it read .value into the lo slot and
;; hard-coded hi to 0, truncating every imported i64 the same way.
;;
;; WebAssembly.Global now stores i64 exactly and exposes it as a BigInt, which
;; is also what the spec requires, and both directions round-trip.

;; REQUIRES: wasm
;; RUN: %wat2wasm %S/e2e-i64-global-export.wat_ -o %t-exp.wasm && %hermesc --wasm -emit-binary -out %t-exp.hbc %t-exp.wasm && %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-i64-global-driver.js_ -- %t-exp.hbc %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "exporter" "big" (global $big i64))
  ;; Return the halves separately: a wrong hi word is then visible directly
  ;; rather than only through 64-bit arithmetic.
  (func (export "lo") (result i32) global.get $big i32.wrap_i64)
  (func (export "hi") (result i32)
    global.get $big (i64.const 32) i64.shr_u i32.wrap_i64))

;; Exported values keep their upper word and are BigInts.
;; CHECK: big: 4294967296 bigint
;; CHECK-NEXT: neg: -1099511627776 bigint
;; CHECK-NEXT: small: 42 bigint

;; And an imported i64 global keeps both halves: 0x100000000 is lo=0, hi=1.
;; Before the fix hi was hard-coded to 0.
;; CHECK-NEXT: imported lo: 0
;; CHECK-NEXT: imported hi: 1
;; CHECK-NEXT: done
