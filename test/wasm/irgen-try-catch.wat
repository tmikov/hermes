;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s --enable-exceptions -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (tag $tag_i32 (param i32))

  ;; Basic try/catch with matching tag
  (func $try_catch_basic (result i32)
    (try (result i32)
      (do
        (throw $tag_i32 (i32.const 42))
        (i32.const 0)
      )
      (catch $tag_i32
        ;; Caught value (i32) is on the stack
      )
    )
  )
)

;; CHECK-LABEL: function wasm_func_0(
;; CHECK: TryStartInst
;; CHECK: CatchInst
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmMatchException]
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmCreateException]
;; CHECK: ThrowInst
