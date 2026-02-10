;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Verify that unsupported (but wired) Wasm opcodes emit warnings
;; rather than being silently ignored. This ensures D.13's requirement
;; that no MVP opcode falls through to BinaryReaderNop's no-op.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm 2>&1 | %FileCheck %s

(module
  (global $g (mut i32) (i32.const 0))

  ;; Exercise global.get and global.set in function body.
  (func $global_test
    global.get $g
    global.set $g))

;; CHECK: warning: unsupported Wasm opcode: global.get
;; CHECK: warning: unsupported Wasm opcode: global.set
