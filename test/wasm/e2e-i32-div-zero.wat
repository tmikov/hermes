;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test that i32.div_s traps on division by zero (F.2).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm -emit-binary -out %t.hbc %t.wasm
;; RUN: ! %hermes %t.hbc 2>&1 | %FileCheck %s

;; CHECK: Error: integer divide by zero

(module
  (func $start
    (i32.div_s (i32.const 10) (i32.const 0))
    drop)
  (start $start)
)
