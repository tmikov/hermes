;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test module with mixed imports (functions, table, memory, global)
;; from different modules, and a start function.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Import a function from "env".
  (import "env" "log" (func $log (param i32)))

  ;; Import a global from "config".
  (import "config" "max_size" (global $max_size i32))

  ;; Import a memory from "env".
  (import "env" "memory" (memory 1 10))

  ;; Import a function from a different module "math".
  (import "math" "square" (func $square (param i32) (result i32)))

  ;; Table declared in this module (not imported).
  (table 4 funcref)

  ;; The start function is the first defined function (func index 2).
  (func $init
    i32.const 0
    call $log
  )
  (start $init)
;; Import trampoline for $log (void return).
;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK: %BB0:
;; CHECK:   LoadFrameInst (:any) %{{.*}}: environment, [%VS0.import_func_0]: any
;; CHECK:   LoadParamInst (:any) %p0: any
;; CHECK:   CallInst (:any)
;; CHECK:   ReturnInst undefined: undefined
;; CHECK-NEXT: function_end

;; Import trampoline for $square (i32 return).
;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; CHECK: %BB0:
;; CHECK:   LoadFrameInst (:any) %{{.*}}: environment, [%VS0.import_func_1]: any
;; CHECK:   LoadParamInst (:any) %p0: any
;; CHECK:   %[[CALL:.*]] = CallInst (:any)
;; CHECK:   %[[RESULT:.*]] = AsInt32Inst (:number) %[[CALL]]: any
;; CHECK:   ReturnInst %[[RESULT]]: number
;; CHECK-NEXT: function_end

;; $init: calls $log(0).
;; CHECK-LABEL: function wasm_func_2(): any
;; CHECK: %BB0:
;; CHECK:   %[[LOG:.*]] = LoadFrameInst (:any) %{{.*}}: environment, [%VS0.closure_0]: any
;; CHECK-NEXT: %{{.*}} = CallInst (:any) %[[LOG]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, 0: number
;; CHECK-NEXT:           BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: ReturnInst undefined: undefined
;; CHECK-NEXT: function_end

  ;; Exported functions.
  (func (export "run") (result i32)
    global.get $max_size
  )
;; "run": global.get loads the imported global.
;; CHECK-LABEL: function wasm_func_3(): any
;; CHECK: %BB0:
;; CHECK:   %[[P:.*]] = GetParentScopeInst
;; CHECK:   %[[G:.*]] = LoadFrameInst (:any) %[[P]]{{.*}}, [%VS0.global_0]: any
;; CHECK:   BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %{{.*}} = PhiInst (:any) %[[G]]: any, %BB0
;; CHECK-NEXT:           ReturnInst
;; CHECK-NEXT: function_end

  (func (export "helper") (param i32) (result i32)
    local.get 0
    call $square
  )
;; "helper": loads param, calls imported $square, returns result.
;; CHECK-LABEL: function wasm_func_4(p0: any): any
;; CHECK: %BB0:
;; CHECK:   %[[L0:.*]] = AllocStackInst (:any) $local_0: any
;; CHECK-NEXT: %[[P0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK-NEXT:              StoreStackInst %[[P0]]: any, %[[L0]]: any
;; CHECK:   %[[V:.*]] = LoadStackInst (:any) %[[L0]]: any
;; CHECK-NEXT: %[[SQ:.*]] = LoadFrameInst (:any) %{{.*}}: environment, [%VS0.closure_1]: any
;; CHECK-NEXT: %[[RES:.*]] = CallInst (:any) %[[SQ]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %[[V]]: any
;; CHECK-NEXT:               BranchInst %BB1
;; CHECK: %BB1:
;; CHECK-NEXT: %[[PHI:.*]] = PhiInst (:any) %[[RES]]: any, %BB0
;; CHECK-NEXT:               ReturnInst %[[PHI]]: any
;; CHECK-NEXT: function_end
)
