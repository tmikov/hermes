;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A multi-value call reads its results back out of the very buffer views it
;; handed to that call.
;;
;; Multi-value results travel in a scratch buffer: the caller tells the callee
;; where to write, the callee writes, the caller reads back. There are two
;; numeric views of it, one for integers and one for floats, and a function
;; meets them in two different roles:
;;
;;   - as PARAMETERS, the views its own caller gave it, which is where its own
;;     results go; and
;;   - loaded from module scope, which is what it hands to the functions IT
;;     calls.
;;
;; onCall and onCallIndirect passed the module-scope pair and emitRetBufLoads
;; then read the results back out of the PARAMETER pair. So a callee wrote one
;; object and its caller read another. Inside one module those are the same
;; object; across modules they are not, and the results came back as zeros.
;; test/wasm/e2e-cross-module-retbuf.wat is that, measured.
;;
;; This file checks the same property on the IR, for both call sites, which an
;; answer cannot distinguish within a single module. Both are worth having:
;; the e2e test says the defect is gone, this one says the shape that caused
;; it is gone, and it fails for either call site independently.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Three result kinds, so the check covers every read in emitRetBufLoads
  ;; that takes a view: i32 and i64 through the integer view, f64 through the
  ;; float one.
  (func $inner (result i32 f64 i64)
    (i32.const 11)
    (f64.const 2.5)
    (i64.const 7))

  (func (export "outer") (result i32 f64 i64)
    (call $inner))

  ;; The same through call_indirect, because onCallIndirect is a separate call
  ;; site with its own copy of the argument handling. Changing only that one
  ;; back would escape a test that checked the direct call alone.
  (type $t (func (result i32 f64 i64)))
  (table 1 funcref)
  (elem (i32.const 0) $inner)
  (func (export "outerIndirect") (result i32 f64 i64)
    (call_indirect (type $t) (i32.const 0))))

;; CHECK-LABEL: function wasm_func_1(retbuf_I: object, retbuf_F: object): number

;; The two roles. INCOMING is what this function's own caller supplied;
;; PASSED is what it hands to $inner.
;; CHECK: %[[INCOMING_I:[0-9]+]] = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT: %[[INCOMING_F:[0-9]+]] = LoadParamInst (:object) %retbuf_F: object
;; CHECK-NEXT: %[[PASSED_I:[0-9]+]] = LoadFrameInst (:any) {{.*}}[%VS0.retBufI]: any
;; CHECK-NEXT: %[[PASSED_F:[0-9]+]] = LoadFrameInst (:any) {{.*}}[%VS0.retBufF]: any

;; The call gets PASSED, and every read back names PASSED. Before the fix
;; these four lines named INCOMING instead.
;; CHECK: CallInst {{.*}} %[[PASSED_I]]: any, %[[PASSED_F]]: any
;; CHECK-NEXT: LoadPropertyInst (:any) %[[PASSED_I]]: any, 0: number
;; CHECK-NEXT: AsInt32Inst
;; CHECK-NEXT: LoadPropertyInst (:any) %[[PASSED_F]]: any, 1: number
;; CHECK-NEXT: LoadPropertyInst (:any) %[[PASSED_I]]: any, 4: number
;; CHECK-NEXT: LoadPropertyInst (:any) %[[PASSED_I]]: any, 5: number

;; And this function's OWN results still go to INCOMING, which is the half
;; that was always right: a function publishes into the buffer its caller
;; gave it. A fix that pointed everything at one pair would break this.
;; CHECK: StorePropertyStrictInst {{.*}} %[[INCOMING_I]]: object, 0: number
;; CHECK-NEXT: StorePropertyStrictInst {{.*}} %[[INCOMING_F]]: object, 1: number
;; CHECK-NEXT: StorePropertyStrictInst {{.*}} %[[INCOMING_I]]: object, 4: number
;; CHECK-NEXT: StorePropertyStrictInst {{.*}} %[[INCOMING_I]]: object, 5: number

;; And the same for onCallIndirect, whose argument handling is its own copy.
;; CHECK-LABEL: function wasm_func_2(retbuf_I: object, retbuf_F: object): number
;; CHECK: %[[IN_I2:[0-9]+]] = LoadParamInst (:object) %retbuf_I: object
;; CHECK-NEXT: %[[IN_F2:[0-9]+]] = LoadParamInst (:object) %retbuf_F: object
;; CHECK: %[[PASSED_I2:[0-9]+]] = LoadFrameInst (:any) {{.*}}[%VS0.retBufI]: any
;; CHECK-NEXT: %[[PASSED_F2:[0-9]+]] = LoadFrameInst (:any) {{.*}}[%VS0.retBufF]: any
;; CHECK: CallInst {{.*}} %[[PASSED_I2]]: any, %[[PASSED_F2]]: any
;; CHECK-NEXT: LoadPropertyInst (:any) %[[PASSED_I2]]: any, 0: number
;; CHECK-NEXT: AsInt32Inst
;; CHECK-NEXT: LoadPropertyInst (:any) %[[PASSED_F2]]: any, 1: number
;; CHECK-NEXT: LoadPropertyInst (:any) %[[PASSED_I2]]: any, 4: number
;; CHECK-NEXT: LoadPropertyInst (:any) %[[PASSED_I2]]: any, 5: number
;; CHECK: StorePropertyStrictInst {{.*}} %[[IN_I2]]: object, 0: number
;; CHECK-NEXT: StorePropertyStrictInst {{.*}} %[[IN_F2]]: object, 1: number
;; CHECK-NEXT: StorePropertyStrictInst {{.*}} %[[IN_I2]]: object, 4: number
;; CHECK-NEXT: StorePropertyStrictInst {{.*}} %[[IN_I2]]: object, 5: number
