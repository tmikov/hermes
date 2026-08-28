;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The shape of the memory and global import link paths, pinned at the IR
;; level. (The table equivalent is irgen-table-import-link.wat.)
;;
;; Both used to be chains of ordinary property reads on the import value:
;;
;;   * a memory was `instanceof WebAssembly.Memory` -- which an object merely
;;     INHERITING from a real memory satisfies -- plus a __wasm_min__ and a
;;     __wasm_max__, each an AsNumberInst away from reaching a native builtin
;;     that calls getNumber(); and then, separately, a `.buffer` read through
;;     a replaceable prototype accessor, so the buffer the module ran on was
;;     not necessarily the one whose size had been validated.
;;   * a global was a __wasm_type__ string compared against
;;     "global:i32:const", and then a `.value` read. Both are ordinary
;;     properties, so an object literal carrying them linked outright.
;;
;; Each is now ONE brand-checking builtin. The --implicit-check-not flags on
;; the RUN line are the real assertion: none of those property names, and no
;; instanceof, appears anywhere in the generated module. Without them a
;; regression that re-added a read alongside the builtin would still pass
;; every positive check below.
;;
;; (`__wasm_type__` is deliberately NOT forbidden: it is still stamped on
;; export wrappers, which is a different mechanism and a different change.)

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s --implicit-check-not=__wasm_min__ --implicit-check-not=__wasm_max__ --implicit-check-not=BinaryInstanceOfInst --implicit-check-not=buffer --implicit-check-not=value --implicit-check-not="global:i32"
;; REQUIRES: wasm

(module
  (import "e" "m" (memory 1 4))
  (import "e" "g" (global i32))
  (func (export "probe") (result i32) global.get 0))

;; -- Memory --
;; The import value goes straight to wasmLinkMemory; null is "not a
;; WebAssembly.Memory" and gets the message that says so.
;; CHECK: %12 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkMemory]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %7: any
;; CHECK-NEXT: %13 = BinaryStrictlyEqualInst (:any) %12: any, null: null
;; CHECK-NEXT: CondBranchInst %13: any, %BB5, %BB8
;; CHECK: [HermesBuiltin.wasmLinkError]{{.*}}"import e.m is not a WebAssembly.Memory": string

;; A memory that IS a memory but does not fit the declaration gets its own
;; message, so the two failures cannot be confused.
;; CHECK: [HermesBuiltin.wasmLinkError]{{.*}}"import e.m does not satisfy the declared memory limits": string

;; The three results are recorded, and all three come from the ONE call: the
;; buffer in particular, so the views below are built over the very buffer
;; whose page count satisfied the declaration.
;; CHECK: StoreFrameInst {{.*}}, %7: any, [%VS0.mem_obj]: any
;; CHECK-NEXT: StoreFrameInst {{.*}}, %26: any, [%VS0.imported_mem_max]: any
;; CHECK-NEXT: StoreFrameInst {{.*}}, %27: any, [%VS0.imported_mem_buf]: any

;; [currentPages, max, buffer]. The page count is compared with no
;; AsNumberInst in the way -- the builtin returns numbers, so there is nothing
;; left to coerce.
;; CHECK: %25 = LoadPropertyInst (:any) %12: any, 0: number
;; CHECK-NEXT: %26 = LoadPropertyInst (:any) %12: any, 1: number
;; CHECK-NEXT: %27 = LoadPropertyInst (:any) %12: any, 2: number
;; CHECK-NEXT: %28 = BinaryGreaterThanOrEqualInst (:any) %25: any, 1: number

;; -1 is the "declares no maximum" sentinel, and this module declares one, so
;; a memory without one fails.
;; CHECK: %30 = BinaryStrictlyEqualInst (:any) %26: any, -1: number
;; CHECK: %32 = BinaryLessThanOrEqualInst (:any) %26: any, 4: number

;; -- Global --
;; One builtin, carrying the DECLARED type code (0 = i32) and mutability
;; (false). Neither is read off the supplied object any more.
;; CHECK: %36 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkGlobal]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %22: any, 0: number, false: boolean

;; Three outcomes, three destinations. null -> the raw-value path (%BB17),
;; undefined -> "is a Global that does not match", anything else -> the value.
;; CHECK-NEXT: %37 = BinaryStrictlyEqualInst (:any) %36: any, null: null
;; CHECK-NEXT: CondBranchInst %37: any, %BB17, %BB13
;; CHECK: %39 = BinaryStrictlyEqualInst (:any) %36: any, undefined: undefined
;; CHECK-NEXT: CondBranchInst %39: any, %BB15, %BB14

;; The value stored is the builtin's result, not a re-read of anything.
;; CHECK: %41 = PhiInst (:any) %22: any, %BB17, %36: any, %BB13
;; CHECK-NEXT: StoreFrameInst {{.*}}, %41: any, [%VS0.import_global_val_0]: any

;; The views are built over the recorded buffer.
;; CHECK: %45 = LoadFrameInst (:any) {{.*}}, [%VS0.imported_mem_buf]: any

;; The two global diagnostics, each naming what was actually wrong.
;; CHECK: [HermesBuiltin.wasmLinkError]{{.*}}"import e.g is a WebAssembly.Global that does not match the declared immutable i32 global import": string
;; CHECK: [HermesBuiltin.wasmLinkError]{{.*}}"import e.g must be a Number to satisfy an i32 global import": string
