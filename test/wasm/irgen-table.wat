;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s
;; REQUIRES: wasm

;; Test: Table initialization with element segments, table.size, table.get.

(module
  (type $void_to_i32 (func (result i32)))

  (table 3 funcref)

  (elem (i32.const 0) $f0 $f1)

  (func $f0 (result i32)
    i32.const 10
  )

  (func $f1 (result i32)
    i32.const 20
  )

  ;; Function that returns table.size
  (func $get_table_size (result i32)
    table.size 0
  )

  ;; Function that gets a table entry
  (func $get_table_entry (param i32) (result funcref)
    local.get 0
    table.get 0
  )
)

;; table.size: loads the funcs array and reads .length
;; CHECK-LABEL: function wasm_func_2(): number 
;; CHECK: GetParentScopeInst
;; CHECK: LoadFrameInst (:any) %{{.*}}: environment, [%VS0.table_0_funcs]
;; CHECK: LoadPropertyInst (:any) %{{.*}}: any, "length": string

;; table.get: bounds-checks against the funcs array, then reads the slot's
;; Exported Function through the builtin. Not a LoadPropertyInst: the array can
;; come from a table import, and an accessor at an index would run user JS
;; inside a Wasm function body.
;; CHECK-LABEL: function wasm_func_3(p0: number): object
;; CHECK: LoadFrameInst (:any) %{{.*}}: environment, [%VS0.table_0_funcs]
;; CHECK: LoadFrameInst (:any) %{{.*}}: environment, [%VS0.table_0_exported]
;; CHECK: CallBuiltinInst (:any) [HermesBuiltin.wasmTableGetSlot]
;; CHECK-NOT: LoadPropertyInst (:any) %{{.*}}: any, %{{.*}}: number
