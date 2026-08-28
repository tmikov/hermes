;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s
;; REQUIRES: wasm

;; Test: Verify that tables are created and element segments are applied
;; in the top-level (global) function.

(module
  (type $void_to_i32 (func (result i32)))

  (table 4 funcref)

  ;; Element segment: place f0 at index 1, f1 at index 2.
  (elem (i32.const 1) $f0 $f1)

  (func $f0 (result i32)
    i32.const 42
  )

  (func $f1 (result i32)
    i32.const 99
  )

  (export "f0" (func $f0))
)

;; The top-level function creates table arrays and applies elem segments.
;; CHECK-LABEL: function global(): any
;; CHECK: CreateScopeInst (:environment)

;; Table arrays creation: new Array(4) for functions and types.
;; CHECK: TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK: CallInst (:any) %{{.*}}: any, {{.*}}4: number
;; CHECK: StoreFrameInst %{{.*}}: environment, %{{.*}}: object, [%VS0.table_0_funcs]: any
;; CHECK: CallInst (:any) %{{.*}}: any, {{.*}}4: number
;; CHECK: StoreFrameInst %{{.*}}: environment, %{{.*}}: object, [%VS0.table_0_types]: any

;; Element segment: load table arrays and store closures at offset 1.
;; CHECK: LoadFrameInst (:any) %{{.*}}: environment, [%VS0.table_0_funcs]: any
;; CHECK: LoadFrameInst (:any) %{{.*}}: environment, [%VS0.table_0_types]: any
;; CHECK: BinaryAddInst (:any) 1: number, 0: number
;; CHECK: LoadFrameInst (:any) %{{.*}}: environment, [%VS0.closure_0]: any
;; CHECK: StorePropertyStrictInst %{{.*}}: any, %{{.*}}: any, %{{.*}}: any
;; CHECK: StorePropertyStrictInst {{.*}}: number, %{{.*}}: any, %{{.*}}: any
;; Second entry at offset 1 + 1 = 2:
;; CHECK: BinaryAddInst (:any) 1: number, 1: number
;; CHECK: LoadFrameInst (:any) %{{.*}}: environment, [%VS0.closure_1]: any
;; CHECK: StorePropertyStrictInst %{{.*}}: any, %{{.*}}: any, %{{.*}}: any
;; CHECK: StorePropertyStrictInst {{.*}}: number, %{{.*}}: any, %{{.*}}: any
