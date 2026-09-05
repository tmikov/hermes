;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation: f32 and f64 constants.
;; Verifies LiteralNumber for floating-point values.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (func (result f32)
    f32.const 3.14)

;; CHECK-LABEL: function wasm_func_0(): number 
;; CHECK-NEXT: %BB0:
;; CHECK:              BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %[[PHI:.*]] = PhiInst (:number) 3.140000{{.*}}: number, %BB0
;; CHECK-NEXT:                 ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end

  (func (result f64)
    f64.const 2.718281828459045))

;; CHECK-LABEL: function wasm_func_1(): number 
;; CHECK-NEXT: %BB0:
;; CHECK:              BranchInst %BB1
;; CHECK-NEXT: %BB1:
;; CHECK-NEXT:   %[[PHI:.*]] = PhiInst (:number) 2.718281{{.*}}: number, %BB0
;; CHECK-NEXT:                 ReturnInst %[[PHI]]: number
;; CHECK-NEXT: function_end
