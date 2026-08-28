;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; An immutable i64 global import is snapshotted at link time by splitting
;; its BigInt value into a lo/hi pair via retBufI, a Uint32Array. Reading a
;; Uint32Array slot back without narrowing comes back unsigned, so both
;; halves must be wrapped in AsInt32Inst -- the same convention every other
;; retBufI read in this pipeline follows (see the C2 export-wrapper fix and
;; the mutable-global-import path below, which already does this). This
;; test pins the IR shape so a regression (e.g. only narrowing one half)
;; is caught structurally, not just through i32.wrap_i64 on the lo half:
;; the hi half's sign has no Wasm operation that observes it directly,
;; since every consumer re-narrows it independently downstream.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  (import "e" "g" (global i64))
  (func (export "f") (result i32) global.get 0 i32.wrap_i64))

;; CHECK-LABEL: function __wasm_instantiate__(imports: any)
;; CHECK: %[[RBI:.*]] = LoadFrameInst (:any) {{.*}}, [%VS0.retBufI]: any
;; CHECK-NEXT: CallBuiltinInst {{.*}}[HermesBuiltin.wasmBigIntToI64]{{.*}}, %[[RBI]]: any,
;; Lo half: narrowed with AsInt32Inst before landing in the global's slot.
;; CHECK-NEXT: %[[LO:.*]] = LoadPropertyInst (:any) %[[RBI]]: any, 0: number
;; CHECK-NEXT: %[[LOI:.*]] = AsInt32Inst (:number) %[[LO]]: any
;; CHECK-NEXT: StoreFrameInst {{.*}}, %[[LOI]]: number, [%VS0.global_0]: any
;; Hi half: same narrowing.
;; CHECK-NEXT: %[[HI:.*]] = LoadPropertyInst (:any) %[[RBI]]: any, 1: number
;; CHECK-NEXT: %[[HII:.*]] = AsInt32Inst (:number) %[[HI]]: any
;; CHECK-NEXT: StoreFrameInst {{.*}}, %[[HII]]: number, [%VS0.global_0_hi]: any
