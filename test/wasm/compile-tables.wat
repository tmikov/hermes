;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test module with a table and element segments compiles to IR.
;; The two table functions return constant i32 values via phi.
;; call_indirect emits wasmCallIndirect validation + CallInst.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; A function table with minimum size 3.
  (table 3 funcref)

  ;; Two simple functions to put in the table.
  (func $f0 (result i32) (i32.const 10))
;; CHECK-LABEL: function wasm_func_0(): number 
;; CHECK: %BB0:
;; CHECK:   BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI0:.*]] = PhiInst (:number) 10: number, %BB0
;; CHECK-NEXT:                ReturnInst %[[PHI0]]: number
;; CHECK-NEXT: function_end

  (func $f1 (result i32) (i32.const 20))
;; CHECK-LABEL: function wasm_func_1(): number 
;; CHECK: %BB0:
;; CHECK:   BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI1:.*]] = PhiInst (:number) 20: number, %BB0
;; CHECK-NEXT:                ReturnInst %[[PHI1]]: number
;; CHECK-NEXT: function_end

  ;; Active element segment that initializes table[0..1] with $f0 and $f1.
  (elem (i32.const 0) $f0 $f1)

  ;; A function that calls indirectly via call_indirect.
  (func (export "call_indirect") (param i32) (result i32)
    local.get 0
    call_indirect (result i32)
  )
;; CHECK-LABEL: function wasm_func_2(p0: number): number 
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:number) $local_0: any
;; CHECK-NEXT: %[[P0:.*]] = LoadParamInst (:number) %p0: number
;; CHECK-NEXT:              StoreStackInst %[[P0]]: number, %[[L0]]: number
;; CHECK:   %[[IDX:.*]] = LoadStackInst (:number) %[[L0]]: number
;; CHECK-NEXT: %[[FUNCS:.*]] = LoadFrameInst (:any) %{{.*}}: environment, [%VS0.table_0_funcs]: any
;; CHECK-NEXT: %[[TYPES:.*]] = LoadFrameInst (:any) %{{.*}}: environment, [%VS0.table_0_types]: any
;; The expected type is the interned id for this signature, loaded from the
;; frame, not a module-local index literal.
;; CHECK-NEXT: %[[TYPEID:.*]] = LoadFrameInst (:any) %{{.*}}: environment, [%VS0.wasm_type_id_0]: any
;; CHECK-NEXT: %[[CLOSURE:.*]] = CallBuiltinInst (:any) [HermesBuiltin.wasmCallIndirect]
;; CHECK-NEXT: %[[RES:.*]] = CallInst (:number) %[[CLOSURE]]: any
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %{{.*}} = PhiInst (:number) %[[RES]]: number, %BB0
;; CHECK-NEXT:           ReturnInst
;; CHECK-NEXT: function_end
)
