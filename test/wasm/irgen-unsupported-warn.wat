;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Verify that unsupported (but wired) Wasm opcodes emit warnings
;; rather than being silently ignored. This ensures D.13's requirement
;; that no MVP opcode falls through to BinaryReaderNop's no-op.
;;
;; ref.null is NOT one of them any more: a null reference is JS null, which
;; needs nothing synthesized, so it has a real handler and warns about nothing.
;; ref.func still does -- there is no way to materialize a funcref for an
;; arbitrary function dynamically.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm 2>&1 | %FileCheck %s

(module
  (table 1 funcref)
  (elem declare func $ref_test)

  ;; Exercise ref.null and ref.func in function body.
  (func $ref_test
    ref.null func
    drop
    ref.func $ref_test
    drop))

;; ref.null is supported and produces null; only ref.func is reported.
;; CHECK-NOT: warning: unsupported Wasm opcode: ref.null
;; CHECK: warning: unsupported Wasm opcode: ref.func
