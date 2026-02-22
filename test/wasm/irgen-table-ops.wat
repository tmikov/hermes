;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm 2>&1 | %FileCheck %s
;; REQUIRES: wasm

;; Test: table.set and table.grow IR generation.

(module
  (table 3 funcref)

  (func $f0 (result i32) i32.const 42)

  ;; table.set: store a value at an index
  (func $set_entry (param i32 funcref)
    local.get 0
    local.get 1
    table.set 0
  )

  ;; table.grow: grows the table, returns old size or -1 on failure
  (func $grow_table (param i32) (result i32)
    ref.null func
    local.get 0
    table.grow 0
  )
)

;; table.set: loads funcs array, stores value at index
;; CHECK-LABEL: function wasm_func_1(p0: number, p1: object): undefined 
;; CHECK: LoadFrameInst (:any) %{{.*}}: environment, [%VS0.table_0_funcs]
;; CHECK: StorePropertyStrictInst %{{.*}}: object, %{{.*}}: any, %{{.*}}: number

;; table.grow: calls the wasmTableGrow builtin
;; CHECK-LABEL: function wasm_func_2(p0: number): number 
;; CHECK: LoadFrameInst (:any) %{{.*}}: environment, [%VS0.table_0_funcs]
;; CHECK: LoadFrameInst (:any) %{{.*}}: environment, [%VS0.table_0_types]
;; CHECK: CallBuiltinInst (:number) [HermesBuiltin.wasmTableGrow]
