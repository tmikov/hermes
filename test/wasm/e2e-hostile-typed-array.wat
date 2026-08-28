;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The Wasm builtins receive their linear-memory view and i64 return buffer
;; as arguments the compiler emits, but those objects are built in generated
;; IR through globalThis.Uint8Array / Uint32Array / ArrayBuffer, which a
;; script can replace. A hostile replacement made the builtins vmcast a
;; non-typed-array -- an assertion failure in a Debug build, a wild pointer
;; write in Release. Every such cast now goes through a checked helper that
;; raises a TypeError instead.

;; REQUIRES: wasm
;; RUN: %wat2wasm %S/e2e-hostile-typed-array-i64.wat_ -o %t-i64.wasm && %hermesc --wasm -emit-binary -out %t-i64.hbc %t-i64.wasm && %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-hostile-typed-array-driver.js_ -- %t-i64.hbc %t.hbc | %FileCheck --match-full-lines %s

(module
  (memory 1)
  (func (export "fill")
    (memory.fill (i32.const 0) (i32.const 7) (i32.const 4))))

;; A forged i64 return buffer is refused rather than written through.
;; CHECK: i64 add with hostile Uint32Array: TypeError
;; A forged linear-memory view is refused rather than written through.
;; CHECK-NEXT: memory.fill with hostile Uint8Array: TypeError
;; CHECK-NEXT: done
