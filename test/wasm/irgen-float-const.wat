;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: f32 and f64 constants.
;; Verifies LiteralNumber for floating-point values.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir %t.wasm | %FileCheck %s

;; CHECK-LABEL: function wasm_func_0(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:        ReturnInst 3.140000{{.*}}: number
;; CHECK-NEXT: function_end

;; CHECK-LABEL: function wasm_func_1(): any
;; CHECK-NEXT: %BB0:
;; CHECK-NEXT:        ReturnInst 2.718281{{.*}}: number
;; CHECK-NEXT: function_end

(module
  (func (result f32)
    f32.const 3.14)
  (func (result f64)
    f64.const 2.718281828459045))
