;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Structure of the return buffer's parallel reference array.
;;
;; The buffer proper is an ArrayBuffer with a Uint32Array view (retBufI) and a
;; Float64Array view (retBufF). A funcref is a JS closure and an externref an
;; arbitrary JS value, so neither can be stored in it. References therefore go
;; to retBufR, a plain JS Array indexed IDENTICALLY to the Uint32Array view:
;; computeRetBufLayout gives a reference the same 4 bytes an i32 gets, and the
;; slot index is the same byteOff/4. That identity is what this test pins --
;; if the two indexings ever diverge, the offsets below stop agreeing.
;;
;; retBufR is created only when some function type that needs a return buffer
;; actually has a funcref or externref result. The negative half of that gate
;; is pinned by the existing golden IR tests (irgen-call.wat,
;; irgen-call-mutual.wat, irgen-export-wrappers.wat), whose scope lines spell
;; out the full variable list with retBufF followed directly by closure_0:
;; creating the array unconditionally makes all of them fail.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (table 1 funcref)
  (elem (i32.const 0) $f)
  (func $f (result i32) (i32.const 7))

  ;; (i32, funcref): i32 -> integer slot 0, funcref -> reference slot 1.
  (func $mv (result i32 funcref)
    (i32.const 42)
    (table.get (i32.const 0)))

  ;; Reads $mv's results back on the wasm->wasm path.
  (func (export "mvGet") (result funcref)
    (local $r funcref)
    (call $mv)
    (local.set $r)
    (drop)
    (local.get $r))

  ;; Re-returns them, so the export wrapper has to marshal them to JS.
  (func (export "mv") (result i32 funcref)
    (call $mv)))

;; --- The reference array exists and sits alongside the two views ---

;; CHECK: scope %VS0 [{{.*}}retBufI: any, retBufF: any, retBufR: any, closure_0: any{{.*}}]

;; --- Store side: emitRetBufStores splits i32 and funcref across the views ---

;; The i32 goes to the Uint32Array parameter at index 0; the funcref goes to
;; retBufR at index 1, NOT to the Uint32Array (which would coerce the closure
;; to NaN and then 0, destroying it at the store).
;; CHECK-LABEL: function wasm_func_1(retbuf_I: object, retbuf_F: object): number
;; CHECK:         StorePropertyStrictInst %{{[0-9]+}}: number, %1: object, 0: number
;; CHECK-NEXT:  %[[R:[0-9]+]] = LoadFrameInst (:any) %0: environment, [%VS0.retBufR]: any
;; CHECK-NEXT:         StorePropertyStrictInst %{{[0-9]+}}: any, %[[R]]: any, 1: number

;; --- Load side: emitRetBufLoads reads the reference back unnarrowed ---

;; The i32 is narrowed with AsInt32Inst, because the Uint32Array reads back
;; unsigned. The reference is not: AsInt32Inst on a closure yields 0. The
;; CHECK-NEXT after the reference load is what pins the absence -- an
;; AsInt32Inst inserted there would break it.
;; CHECK-LABEL: function wasm_func_2(): object
;; CHECK:       %[[I:[0-9]+]] = LoadPropertyInst (:any) %{{[0-9]+}}: any, 0: number
;; CHECK-NEXT:  %{{[0-9]+}} = AsInt32Inst (:number) %[[I]]: any
;; CHECK-NEXT:  %[[RA:[0-9]+]] = LoadFrameInst (:any) %0: environment, [%VS0.retBufR]: any
;; CHECK-NEXT:  %[[REF:[0-9]+]] = LoadPropertyInst (:any) %[[RA]]: any, 1: number
;; CHECK-NEXT:         StoreStackInst %[[REF]]: any, %{{[0-9]+}}: object

;; --- Allocation: a JS Array of retBufSize/4 slots, next to the two views ---

;; CHECK-LABEL: function __wasm_instantiate__(imports: any): object
;; CHECK:         StoreFrameInst %0: environment, %{{[0-9]+}}: object, [%VS0.retBufI]: any
;; CHECK-NEXT:         StoreFrameInst %0: environment, %{{[0-9]+}}: object, [%VS0.retBufF]: any
;; CHECK-NEXT:  %{{[0-9]+}} = TryLoadGlobalPropertyInst (:any) globalObject: object, "Array": string
;; CHECK:         StoreFrameInst %0: environment, %{{[0-9]+}}: object, [%VS0.retBufR]: any

;; --- Export wrapper: the real reference reaches the JS result array ---

;; The funcref half is loaded from retBufR and stored into the returned JS
;; Array as-is. Before the fix this arm warned and substituted `undefined`,
;; because the slot in the Uint32Array only ever held 0.
;; CHECK-LABEL: function wasm_export_mv(): any
;; CHECK:       %[[RB:[0-9]+]] = LoadFrameInst (:any) %0: environment, [%VS0.retBufR]: any
;; CHECK-NEXT:  %[[V:[0-9]+]] = LoadPropertyInst (:any) %[[RB]]: any, 1: number
;; CHECK-NEXT:         StorePropertyStrictInst %[[V]]: any, %{{[0-9]+}}: object, 1: number
