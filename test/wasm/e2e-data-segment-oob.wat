;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test that an OOB active data segment (I32Const offset, locally-defined
;; memory) traps at instantiation with "unreachable executed".

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && ! %hermes -Xhermes-internal-test-methods %S/instantiate-hbc.js_ -- %t.hbc 2>&1 | %FileCheck %s

(module
  (memory 1)
  ;; Offset 65536 + 1 byte exceeds the 65536-byte (1 page) memory.
  (data (i32.const 65536) "a")
)

;; CHECK: Error: unreachable executed
