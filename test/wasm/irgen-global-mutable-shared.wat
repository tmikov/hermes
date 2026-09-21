;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The shape of an imported MUTABLE global's use paths, pinned at the IR
;; level. (The link path itself is irgen-memory-global-import-link.wat; this
;; file is about what happens afterwards.)
;;
;; A mutable global import is genuinely shared with the host's
;; WebAssembly.Global -- both sides must see each other's writes -- so the
;; object is kept rather than snapshotted, and consulted at every global.get
;; and global.set. Both used to be ordinary `.value` accesses, and `value` is a
;; CONFIGURABLE accessor pair on WebAssembly.Global.prototype: a replaced
;; getter fed the module 999 for a global holding 77 and a replaced setter
;; swallowed every write the module made.
;;
;; `--implicit-check-not=value` on the RUN line is the real assertion here.
;; e2e-global-import-mutable-hijack.wat counts accessor invocations, so it
;; would notice a property access that ran; this file additionally forbids one
;; being EMITTED next to the builtin in a path no test happens to execute.

;; (`StorePropertyStrictInst` is deliberately NOT forbidden: the export
;; descriptors and the export wrappers are built with property stores, which
;; is a different mechanism. `value` alone is enough -- the store that used to
;; be here named it.)

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s --implicit-check-not=value
;; REQUIRES: wasm

(module
  (import "e" "g" (global $g (mut i32)))
  (func (export "get") (result i32) global.get $g)
  (func (export "set") (param i32) local.get 0 global.set $g))

;; global.get: load the object out of its hidden frame Variable and read the
;; internal field through the builtin. The AsInt32Inst that follows is
;; coerceImportedGlobalValue, which is a NO-OP on this path -- the value came
;; out of value_, whose only writer canonicalises it -- and is kept only so
;; that its retirement happens once, with Task 6's J4 work. Measured: deleting
;; it leaves every behavioural test green and fails this line alone, which is
;; why the line is here: it makes the removal a deliberate act rather than a
;; silent one.
;; CHECK-LABEL: function wasm_func_0(): number
;; CHECK: [[G:%[0-9]+]] = LoadFrameInst (:any) {{.*}}, [%VS0.import_global_val_0]: any
;; CHECK-NEXT: [[V:%[0-9]+]] = CallBuiltinInst (:any) [HermesBuiltin.wasmGlobalGet]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, [[G]]: any
;; CHECK-NEXT: AsInt32Inst (:number) [[V]]: any

;; global.set: the same object, and the value written straight into the
;; internal field.
;; CHECK-LABEL: function wasm_func_1(p0: number): undefined
;; CHECK: [[G2:%[0-9]+]] = LoadFrameInst (:any) {{.*}}, [%VS0.import_global_val_0]: any
;; CHECK-NEXT: CallBuiltinInst (:any) [HermesBuiltin.wasmGlobalSet]: number, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, [[G2]]: any

;; The instantiate body links the import and then does NOT read its value.
;; The frame slot used to be seeded with a link-time snapshot, which cost a
;; wasmGlobalGet per mutable global import; nothing reads that slot, because
;; every reader (global.get, global.set, the export loop) takes the object
;; path above. The call also has to go: once an exporting module publishes a
;; closure-backed Global, it would run the EXPORTER's getter inside the
;; IMPORTER's instantiation.
;; CHECK-LABEL: function __wasm_instantiate__(imports: any): object
;; CHECK: [HermesBuiltin.wasmLinkGlobal]
;; CHECK-NOT: [HermesBuiltin.wasmGlobalGet]
