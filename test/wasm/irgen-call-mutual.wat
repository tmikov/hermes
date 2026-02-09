;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s
;; REQUIRES: wasm

;; Test: Mutual recursion between two functions.
;; is_even(n) = if n == 0 then 1 else is_odd(n - 1)
;; is_odd(n)  = if n == 0 then 0 else is_even(n - 1)
(module
  (func $is_even (param i32) (result i32)
    local.get 0
    i32.eqz
    if (result i32)
      i32.const 1
    else
      local.get 0
      i32.const 1
      i32.sub
      call $is_odd
    end
  )

  (func $is_odd (param i32) (result i32)
    local.get 0
    i32.eqz
    if (result i32)
      i32.const 0
    else
      local.get 0
      i32.const 1
      i32.sub
      call $is_even
    end
  )
)

;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; is_even calls is_odd (wasm_func_1)
;; CHECK: LoadFrameInst (:any) {{.*}}[%VS0.closure_1]: any
;; CHECK: CallInst (:any) %{{[0-9]+}}: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined,

;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; is_odd calls is_even (wasm_func_0)
;; CHECK: LoadFrameInst (:any) {{.*}}[%VS0.closure_0]: any
;; CHECK: CallInst (:any) %{{[0-9]+}}: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined,
