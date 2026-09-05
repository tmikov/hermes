;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (memory 1 4)

  ;; memory.size: should load HEAPU8 view, get .length, shift right by 16.
  (func (export "size") (result i32)
    memory.size
  )

;; CHECK-LABEL: function wasm_func_0(): number 
;; CHECK:   LoadFrameInst (:any) %{{.*}}, [%VS0.HEAPU8]: any
;; CHECK:   LoadPropertyInst (:any) %{{.*}}, "length": string
;; CHECK:   BinaryUnsignedRightShiftInst (:number) %{{.*}}, 16: number

  ;; memory.grow: pops delta, calls wasmMemoryGrow builtin, conditionally
  ;; creates new views on success.
  (func (export "grow") (param i32) (result i32)
    local.get 0
    memory.grow
  )

;; CHECK-LABEL: function wasm_func_1(p0: number): number 
;; Load HEAPU8 and compute old page count.
;; CHECK:   LoadFrameInst (:any) %{{.*}}, [%VS0.HEAPU8]: any
;; CHECK:   LoadPropertyInst (:any) %{{.*}}, "length": string
;; CHECK:   BinaryUnsignedRightShiftInst (:number) %{{.*}}, 16: number
;; Call the grow builtin with heapu8, delta, maxPages=4.
;; CHECK:   CallBuiltinInst (:number|object) [HermesBuiltin.wasmMemoryGrow]{{.*}}4: number
;; Compare result to -1.
;; CHECK:   BinaryStrictlyEqualInst (:any) %{{.*}}, -1: number
;; CHECK:   CondBranchInst
;; On success, create new typed array views from the returned ArrayBuffer.
;; CHECK:   TryLoadGlobalPropertyInst (:any) globalObject: object, "Int8Array": string
;; CHECK:   StoreFrameInst %{{.*}}, %{{.*}}, [%VS0.HEAP8]: any
;; CHECK:   TryLoadGlobalPropertyInst (:any) globalObject: object, "Uint8Array": string
;; CHECK:   StoreFrameInst %{{.*}}, %{{.*}}, [%VS0.HEAPU8]: any
;; The phi merges -1 (failure) with oldPages (success).
;; CHECK:   PhiInst (:number) -1: number, %BB{{[0-9]+}}, %{{.*}}, %BB{{[0-9]+}}
)
