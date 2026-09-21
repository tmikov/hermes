;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The transport for the REFERENCE results of a multi-value call.
;;
;; The numeric results travel in an ArrayBuffer with a Uint32Array view
;; (retBufI) and a Float64Array view (retBufF). A funcref is a JS closure and
;; an externref an arbitrary JS value, so neither can be stored in either view.
;; References therefore travel in a container allocated by wasmAllocRefBuf,
;; which the caller passes to the callee as a hidden third argument and reads
;; back out of afterwards.
;;
;; Two things this pins that the arithmetic depends on:
;;
;;   - The slot index is the SAME byteOff/4 an i32 at that offset would use.
;;     computeRetBufLayout gives a reference the same 4 bytes as an i32, so the
;;     (i32, funcref) signature below puts the i32 at integer index 0 and the
;;     funcref at reference index 1.
;;   - Because that layout is sparse -- indices are byte offsets over four, not
;;     reference ordinals -- the container is sized by the TOTAL layout size
;;     over four, which is 2 here, and not by the one reference the signature
;;     has.
;;
;; And the property the container exists for: the value passed to a call is the
;; value read back after it. The CHECK lines below bind one FileCheck variable
;; to the wasmAllocRefBuf result and require that same variable at the call and
;; at the wasmRefBufGet, so reaching for a container anywhere else -- the
;; module frame, this function's own incoming parameter -- fails the test.

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

;; --- No per-module reference array in the top-level scope ---

;; The container is per call, so there is no module-level variable for it. The
;; scope line is spelled out from retBufI to closure_0, which is where a
;; variable for a shared array would have to sit.
;; CHECK: scope %VS0 [{{.*}}retBufI: any, retBufF: any, closure_0: any{{.*}}]

;; --- Store side: emitRetBufStores splits i32 and funcref ---

;; The callee's signature carries the container as its third hidden parameter,
;; after the two numeric views.
;; CHECK-LABEL: function wasm_func_1(retbuf_I: object, retbuf_F: object, retbuf_R: any): number
;; CHECK: %[[RECV:[0-9]+]] = LoadParamInst (:any) %retbuf_R: any

;; The i32 goes to the Uint32Array parameter at index 0; the funcref goes
;; through wasmRefBufSet at reference index 1, into the container the CALLER
;; supplied -- not into the Uint32Array, which would coerce the closure to NaN
;; and then 0, destroying it at the store.
;; CHECK: StorePropertyStrictInst %{{[0-9]+}}: number, %{{[0-9]+}}: object, 0: number
;; CHECK-NEXT: %{{[0-9]+}} = CallBuiltinInst (:any) [HermesBuiltin.wasmRefBufSet]{{.*}}, %[[RECV]]: any, 1: number, %{{[0-9]+}}: any

;; --- Load side: the caller allocates, passes, and reads back the same value ---

;; Two slots, because the layout is byte offsets over four and the signature
;; lays out 8 bytes; the signature has one reference.
;; CHECK-LABEL: function wasm_func_2(): null|object
;; CHECK: %[[BUF:[0-9]+]] = CallBuiltinInst (:any) [HermesBuiltin.wasmAllocRefBuf]{{.*}}, 2: number
;; CHECK: %{{[0-9]+}} = CallInst (:number) %{{[0-9]+}}: any, %wasm_func_1(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %{{[0-9]+}}: any, %{{[0-9]+}}: any, %[[BUF]]: any

;; The i32 is narrowed with AsInt32Inst, because the Uint32Array reads back
;; unsigned. The reference is not: AsInt32Inst on a closure yields 0. The
;; CHECK-NEXT after the reference read is what pins the absence -- an
;; AsInt32Inst inserted there would break it.
;; CHECK: %[[I:[0-9]+]] = LoadPropertyInst (:any) %{{[0-9]+}}: any, 0: number
;; CHECK-NEXT: %{{[0-9]+}} = AsInt32Inst (:number) %[[I]]: any
;; CHECK-NEXT: %[[REF:[0-9]+]] = CallBuiltinInst (:any) [HermesBuiltin.wasmRefBufGet]{{.*}}, %[[BUF]]: any, 1: number
;; CHECK-NEXT: StoreStackInst %[[REF]]: any, %{{[0-9]+}}: null|object

;; --- Export wrapper: the same discipline, plus an intrinsic result array ---

;; CHECK-LABEL: function wasm_export_mv(): any
;; CHECK: %[[WBUF:[0-9]+]] = CallBuiltinInst (:any) [HermesBuiltin.wasmAllocRefBuf]{{.*}}, 2: number
;; CHECK: %{{[0-9]+}} = CallInst (:any) %{{[0-9]+}}: any, %wasm_func_3(): functionCode, true: boolean, empty: any, undefined: undefined, undefined: undefined, %{{[0-9]+}}: any, %{{[0-9]+}}: any, %[[WBUF]]: any
;; CHECK: %[[NUM:[0-9]+]] = AsInt32Inst (:number) %{{[0-9]+}}: any
;; CHECK-NEXT: %[[WREF:[0-9]+]] = CallBuiltinInst (:any) [HermesBuiltin.wasmRefBufGet]{{.*}}, %[[WBUF]]: any, 1: number

;; The public array is built by a builtin from values already computed, so no
;; TryLoadGlobalPropertyInst of "Array" and no indexed property store stands
;; between a result being read and the array being returned.
;; CHECK-NEXT: %[[ARR:[0-9]+]] = CallBuiltinInst (:object) [HermesBuiltin.wasmMakeResultArray]{{.*}}, %[[NUM]]: number, %[[WREF]]: any
;; CHECK-NEXT: ReturnInst %[[ARR]]: object
