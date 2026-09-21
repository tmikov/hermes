;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The Wasm builtins receive their linear-memory view and i64 return buffer as
;; arguments the compiler emits. Those objects used to be built in generated IR
;; through globalThis.Uint8Array / Uint32Array / ArrayBuffer, so a hostile
;; replacement made the builtins vmcast a non-typed-array -- an assertion
;; failure in a Debug build, a wild pointer write in Release -- and the fix
;; then was to route every such cast through a checked helper that raises a
;; TypeError.
;;
;; Those checked helpers are still there, but nothing reaches them by this
;; route any more: the views and the return buffer are built from the pristine
;; constructors under HermesInternal.intrinsics, so a replaced global is simply
;; not consulted and both modules run normally. This file now pins THAT, which
;; is the stronger property -- a refused call is still a module that cannot
;; run in a program that reassigned a global for its own reasons.

;; REQUIRES: wasm
;; RUN: %wat2wasm %S/e2e-hostile-typed-array-i64.wat_ -o %t-i64.wasm && %hermesc --wasm -emit-binary -out %t-i64.hbc %t-i64.wasm && %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-hostile-typed-array-driver.js_ -- %t-i64.hbc %t.hbc | %FileCheck --match-full-lines %s

(module
  (memory 1)
  (func (export "fill")
    (memory.fill (i32.const 0) (i32.const 7) (i32.const 4)))
  (func (export "peek") (param i32) (result i32)
    (i32.load8_u (local.get 0))))

;; CHECK: replacements are live: true

;; A real i64 result, not a TypeError and not a substituted number.
;; CHECK-NEXT: i64 add with hostile Uint32Array: 3

;; memory.fill wrote 7 into the first four bytes of the module's own linear
;; memory, and stopped there.
;; CHECK-NEXT: memory.fill with hostile Uint8Array: byte 0 is 7, byte 4 is 0

;; And neither module ever consulted the replacements.
;; CHECK-NEXT: modules called the replaced constructors: 0
;; CHECK-NEXT: done
