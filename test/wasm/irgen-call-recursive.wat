;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s
;; REQUIRES: wasm

;; Test: Recursive factorial function.
(module
  ;; factorial(n) = if n == 0 then 1 else n * factorial(n - 1)
  (func $factorial (param i32) (result i32)
    local.get 0
    i32.eqz
    if (result i32)
      i32.const 1
    else
      local.get 0
      local.get 0
      i32.const 1
      i32.sub
      call $factorial
      i32.mul
    end
  )
)

;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; Verify there is a recursive CallInst targeting wasm_func_0.
;; CHECK: LoadFrameInst (:any) {{.*}}[%VS0.closure_0]: any
;; CHECK: CallInst (:any) %{{[0-9]+}}: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined,
;; Verify multiplication via Math.imul (n * factorial(n-1)).
;; CHECK: CallBuiltinInst (:any) [Math.imul]
