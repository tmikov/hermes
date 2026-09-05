;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The export wrapper reads i64 results out of retBufI, which is a Uint32Array,
;; so the halves come back UNSIGNED. The split i64 convention used everywhere
;; else in this file is a pair of *signed* int32 halves -- readI64FromRetBuf(),
;; emitRetBufLoads() and the wrapper's own i32 result case all narrow with
;; AsInt32Inst. The two i64 return paths in createExportWrapper did not.
;;
;; This is a structural test on purpose, and it is the only kind that can hold
;; here. Runtime behavior is IDENTICAL with or without the narrowing, because
;; the sole consumer of these halves is the wasmI64ToBigInt builtin, whose
;; argsToI64 does `static_cast<uint32_t>(truncateToInt32(arg))` on each half and
;; therefore re-truncates. No runtime test can distinguish the two forms. What
;; is being pinned is that the invariant is enforced at the read site rather
;; than resting on a distant builtin's implementation detail -- delete either
;; AsInt32Inst and this test fails.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Single i64 result: wrapper reads retBufI[0]/[1] and builds a BigInt.
  (func (export "one") (result i64)
    i64.const -1)

  ;; Multi-value with an i64: wrapper reads the i64 inside the result loop,
  ;; immediately next to the i32 case that already narrows.
  (func (export "two") (result i32 i64)
    i32.const -1
    i64.const -1))

;; --- Single-i64 wrapper: both halves narrowed before wasmI64ToBigInt ---

;; CHECK-LABEL: function wasm_export_one(): any
;; CHECK: %[[LO:[0-9]+]] = LoadPropertyInst (:any) %{{[0-9]+}}: any, 0: number
;; CHECK-NEXT: %[[LON:[0-9]+]] = AsInt32Inst (:number) %[[LO]]: any
;; CHECK-NEXT: %[[HI:[0-9]+]] = LoadPropertyInst (:any) %{{[0-9]+}}: any, 1: number
;; CHECK-NEXT: %[[HIN:[0-9]+]] = AsInt32Inst (:number) %[[HI]]: any
;; CHECK-NEXT: %{{[0-9]+}} = CallBuiltinInst (:bigint) [HermesBuiltin.wasmI64ToBigInt]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %[[LON]]: number, %[[HIN]]: number

;; --- Multi-value wrapper: the i32 result narrows (it always did) and so does
;; --- each half of the i64 result.

;; CHECK-LABEL: function wasm_export_two(): any
;; CHECK: %[[MI32:[0-9]+]] = LoadPropertyInst (:any) %{{[0-9]+}}: any, 0: number
;; CHECK-NEXT: %[[MI32N:[0-9]+]] = AsInt32Inst (:number) %[[MI32]]: any
;; CHECK-NEXT: StorePropertyStrictInst %[[MI32N]]: number, %{{[0-9]+}}: object, 0: number
;; CHECK-NEXT: %[[MLO:[0-9]+]] = LoadPropertyInst (:any) %{{[0-9]+}}: any, 1: number
;; CHECK-NEXT: %[[MLON:[0-9]+]] = AsInt32Inst (:number) %[[MLO]]: any
;; CHECK-NEXT: %[[MHI:[0-9]+]] = LoadPropertyInst (:any) %{{[0-9]+}}: any, 2: number
;; CHECK-NEXT: %[[MHIN:[0-9]+]] = AsInt32Inst (:number) %[[MHI]]: any
;; CHECK-NEXT: %{{[0-9]+}} = CallBuiltinInst (:bigint) [HermesBuiltin.wasmI64ToBigInt]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %[[MLON]]: number, %[[MHIN]]: number
