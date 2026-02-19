;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for data segment runtime bounds check with GlobalGet.
;; The offset comes from global.get, so its value is unknown at compile time.
;; This should emit a conditional branch: if (offset >>> 0 + size > length) trap.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (import "env" "g" (global i32))
  (memory 1)
  (data (global.get 0) "ab")
)

;; CHECK-LABEL: function global()
;; Load the global value (offset).
;; CHECK:       LoadFrameInst {{.*}}[%VS0.global_0]
;; Load HEAPU8 and get its .length for the runtime bounds check.
;; CHECK:       LoadFrameInst {{.*}}[%VS0.HEAPU8]
;; CHECK:       LoadPropertyInst {{.*}} "length"
;; Unsigned conversion: offset >>> 0
;; CHECK:       BinaryUnsignedRightShiftInst {{.*}} 0
;; End position: offsetU + 2 (data is "ab", 2 bytes)
;; CHECK:       BinaryAddInst {{.*}} 2
;; Compare: end > memLength
;; CHECK:       BinaryGreaterThanInst
;; Conditional branch to trap or continue
;; CHECK:       CondBranchInst
;; Trap block
;; CHECK:       CallBuiltinInst {{.*}}[HermesBuiltin.wasmTrap]
;; CHECK-NEXT:  UnreachableInst
;; OK block: stores data bytes ('a' = 97, 'b' = 98)
;; CHECK:       StorePropertyStrictInst 97
;; CHECK:       StorePropertyStrictInst 98
