;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; ref.is_null is not implemented. Its sibling ref.func routes through
;; warnUnsupported(), which reports the opcode and keeps the value stack
;; consistent by popping the inputs and pushing placeholder outputs. (ref.null
;; is implemented -- it pushes null.)
;; ref.is_null had no override at all, so wabt's default no-op ran: the operand
;; was left on the stack and the reference itself was returned in place of the
;; i32 result, with no diagnostic. A module using it compiled and produced a
;; value of the wrong type, silently.
;;
;; It must at least be reported, and leave the stack consistent, until it is
;; actually implemented.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>&1 | %FileCheck --match-full-lines %s
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>/dev/null && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/ref-is-null-unsupported-driver.js_ -- %t.hbc | %FileCheck --check-prefix=EXEC --match-full-lines %s

(module
  (func (export "is_null_local") (result i32)
    (local funcref)
    local.get 0
    ref.is_null))

;; The opcode is reported rather than silently ignored.
;; CHECK: warning: unsupported Wasm opcode: ref.is_null

;; And the placeholder output is pushed, so the funcref operand is not returned
;; in its place. `undefined` is wrong for an i32 result, but it is the
;; documented placeholder for an unsupported opcode -- the point is that it is
;; no longer the reference itself.
;; EXEC: is_null_local: undefined
