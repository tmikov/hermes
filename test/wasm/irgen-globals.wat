;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for Wasm globals (K.1).

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Global 0: immutable i32 initialized to 42.
  (global $g_imm i32 (i32.const 42))

  ;; Global 1: mutable i32 initialized to 100.
  (global $g_mut (mut i32) (i32.const 100))

  ;; Global 2: mutable f64 initialized to 3.14.
  (global $g_f64 (mut f64) (f64.const 3.14))

  ;; Function 0: read immutable global.
  (func $get_imm (result i32)
    global.get $g_imm)

;; CHECK-LABEL: function wasm_func_0(): number 
;; CHECK: %BB0:
;; CHECK:   %[[PARENT:.*]] = GetParentScopeInst
;; CHECK:   %[[VAL:.*]] = LoadFrameInst (:any) %[[PARENT]]{{.*}}, [%VS0.global_0]: any
;; CHECK:   BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %{{.*}} = PhiInst (:any) %[[VAL]]: any, %BB0
;; CHECK-NEXT:           ReturnInst

  ;; Function 1: read and write mutable global.
  (func $set_and_get (param i32) (result i32)
    local.get 0
    global.set $g_mut
    global.get $g_mut)

;; CHECK-LABEL: function wasm_func_1(p0: number): number 
;; CHECK: %BB0:
;; CHECK:   %[[PARENT:.*]] = GetParentScopeInst
;; CHECK:   %[[P0:.*]] = LoadStackInst (:number)
;; CHECK:   StoreFrameInst %[[PARENT]]{{.*}}, %[[P0]]: number, [%VS0.global_1]: any
;; CHECK:   %[[LOADED:.*]] = LoadFrameInst (:any) %[[PARENT]]{{.*}}, [%VS0.global_1]: any
;; CHECK:   BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %{{.*}} = PhiInst (:any) %[[LOADED]]: any, %BB0
;; CHECK-NEXT:           ReturnInst

  ;; Function 2: read/write f64 global.
  (func $f64_global (result f64)
    f64.const 6.28
    global.set $g_f64
    global.get $g_f64)

;; CHECK-LABEL: function wasm_func_2(): number 
;; CHECK: %BB0:
;; CHECK:   %[[PARENT:.*]] = GetParentScopeInst
;; CHECK:   StoreFrameInst %[[PARENT]]{{.*}}, 6.28{{.*}}: number, [%VS0.global_2]: any
;; CHECK:   %[[LOADED:.*]] = LoadFrameInst (:any) %[[PARENT]]{{.*}}, [%VS0.global_2]: any
;; CHECK:   BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %{{.*}} = PhiInst (:any) %[[LOADED]]: any, %BB0
;; CHECK-NEXT:           ReturnInst
)
