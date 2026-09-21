;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for data segment offsets with GlobalGet and extended
;; constant expressions. GlobalGet offset should emit a runtime bounds check.
;; Extended const expr (i32.add) should emit BinaryAddInst + BinaryOrInst.

;; REQUIRES: wasm
;; RUN: %wat2wasm --enable-extended-const %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (import "env" "g" (global i32))
  (memory 1)
  (data (global.get 0) "ab")
  (data (i32.add (i32.const 10) (i32.const 5)) "cd")
)

;; CHECK-LABEL: function __wasm_instantiate__(imports: any)
;; --- Segment 0: global.get offset ---
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
;; OK block: bulk-copy data segment via wasmDataSegmentInit(heapu8, 0, 2, offset)
;; CHECK:       LoadFrameInst {{.*}}[%VS0.HEAPU8]
;; CHECK:       CallBuiltinInst {{.*}}[HermesBuiltin.wasmDataSegmentInit]

;; --- Segment 1: extended const expr (i32.add) ---
;; CHECK:       BinaryAddInst {{.*}} 10: number, 5: number
;; Truncate to i32: result | 0
;; CHECK:       BinaryOrInst {{.*}} 0: number
;; Runtime bounds check for extended const expr
;; CHECK:       LoadFrameInst {{.*}}[%VS0.HEAPU8]
;; CHECK:       LoadPropertyInst {{.*}} "length"
;; CHECK:       BinaryUnsignedRightShiftInst {{.*}} 0
;; CHECK:       BinaryAddInst {{.*}} 2
;; CHECK:       BinaryGreaterThanInst
;; CHECK:       CondBranchInst
;; Bulk-copy data segment via wasmDataSegmentInit(heapu8, 2, 2, offset)
;; CHECK:       LoadFrameInst {{.*}}[%VS0.HEAPU8]
;; CHECK:       CallBuiltinInst {{.*}}[HermesBuiltin.wasmDataSegmentInit]
