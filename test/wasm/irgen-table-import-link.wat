;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The shape of the table import link path, pinned at the IR level.
;;
;; It used to be a chain of ordinary property reads on the import value: a
;; __wasm_type__ string compared against "table:funcref", a __wasm_min__ or a
;; __wasm_funcs__.length for the current size, a __wasm_max__ for the maximum,
;; and then __wasm_funcs__/__wasm_types__/__wasm_exported__ adopted as the
;; module's storage -- with a second branch that built fresh arrays when the
;; first was absent. Every one of those is a value script chooses, which is
;; what made a plain object literal linkable as a table.
;;
;; It is now ONE brand-checking builtin. The --implicit-check-not flags on the
;; RUN line are the real assertion: no __wasm_* storage or limit property is
;; read anywhere in the generated module. Without them a regression that
;; re-added a publication read alongside the builtin would still pass every
;; positive check below.
;;
;; (`__wasm_type__` is deliberately NOT forbidden: it is still stamped on
;; export wrappers, which is a different mechanism and a different change.)

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s --implicit-check-not=__wasm_funcs__ --implicit-check-not=__wasm_types__ --implicit-check-not=__wasm_exported__ --implicit-check-not=__wasm_min__ --implicit-check-not=__wasm_max__ --implicit-check-not="table:funcref"
;; REQUIRES: wasm

(module
  (import "e" "t" (table 2 10 funcref))
  (func (export "size") (result i32) (table.size 0)))

;; The import value goes straight to wasmLinkTable, with the module's DECLARED
;; element type as the second argument -- the element type is no longer read
;; off the supplied object, which is what let an externref declaration borrow
;; a funcref table's storage.
;; CHECK: %12 = CallBuiltinInst (:any) [HermesBuiltin.wasmLinkTable]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %7: any, true: boolean

;; Null means "not a WebAssembly.Table", and gets the message that says so.
;; CHECK-NEXT: %13 = BinaryStrictlyEqualInst (:any) %12: any, null: null
;; CHECK-NEXT: CondBranchInst %13: any, %BB5, %BB7
;; CHECK: [HermesBuiltin.wasmLinkError]{{.*}}"import e.t is not a WebAssembly.Table": string

;; A table that IS a table but does not fit the declaration gets its own
;; message, so the two failures cannot be confused.
;; CHECK: [HermesBuiltin.wasmLinkError]{{.*}}"import e.t is a WebAssembly.Table that does not satisfy the declared limits": string

;; [funcs, types, exported, max]. The current size is the storage's own
;; length, so it reflects every grow and cannot go stale the way a recorded
;; __wasm_min__ did.
;; CHECK: %19 = LoadPropertyInst (:any) %12: any, 0: number
;; CHECK-NEXT: %20 = LoadPropertyInst (:any) %12: any, 1: number
;; CHECK-NEXT: %21 = LoadPropertyInst (:any) %12: any, 2: number
;; CHECK-NEXT: %22 = LoadPropertyInst (:any) %12: any, 3: number
;; CHECK-NEXT: %23 = LoadPropertyInst (:any) %19: any, "length": string
;; CHECK-NEXT: %24 = BinaryGreaterThanOrEqualInst (:any) %23: any, 2: number

;; The maximum that table.grow will respect is the TABLE'S, not the
;; declaration's -- the declaration is only an upper bound on it. The imported
;; object itself is recorded too, because a re-export publishes that very
;; object; there is nothing left to copy onto a fresh one.
;; CHECK: StoreFrameInst %0: environment, %22: any, [%VS0.imported_table_max_0]: any
;; CHECK-NEXT: StoreFrameInst %0: environment, %7: any, [%VS0.table_0_obj]: any
;; CHECK-NEXT: StoreFrameInst %0: environment, %19: any, [%VS0.table_0_funcs]: any
;; CHECK-NEXT: StoreFrameInst %0: environment, %20: any, [%VS0.table_0_types]: any
;; CHECK-NEXT: StoreFrameInst %0: environment, %21: any, [%VS0.table_0_exported]: any

;; The declared maximum is checked against the table's own, and "no maximum"
;; (-1) does not satisfy a declaration that has one.
;; CHECK: %59 = BinaryStrictlyEqualInst (:any) %22: any, -1: number
;; CHECK: %61 = BinaryLessThanOrEqualInst (:any) %22: any, 10: number
