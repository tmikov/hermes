;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for data segment compile-time OOB bounds check.
;; A data segment at offset 65536 with 1 byte exceeds 1 page (65536 bytes).
;; This should emit an unconditional trap with no conditional branch.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (memory 1)
  (data (i32.const 65536) "a")
)

;; CHECK-LABEL: function global()
;; Unconditional trap for compile-time OOB data segment.
;; CHECK:       CallBuiltinInst {{.*}}[HermesBuiltin.wasmTrap]
;; CHECK-NEXT:  UnreachableInst
;; No data stores should follow — the trap prevents further initialization.
;; CHECK-NOT:   StorePropertyStrictInst
;; CHECK:       function_end
