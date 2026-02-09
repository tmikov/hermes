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

  ;; Exercise unsupported binary op (i64 is still deferred).
  (func $rotl_test (param i64 i64) (result i64)
    local.get 0
    local.get 1
    i64.rotl)

  ;; Exercise unsupported unary op (i64 is still deferred).
  (func $clz_test (param i64) (result i64)
    local.get 0
    i64.clz)

  ;; Exercise global.get and global.set in function body.
  (func $global_test
    global.get $g
    global.set $g))

;; CHECK: warning: unsupported Wasm opcode: i64.rotl
;; CHECK: warning: unsupported Wasm opcode: i64.clz
;; CHECK: warning: unsupported Wasm opcode: global.get
;; CHECK: warning: unsupported Wasm opcode: global.set
