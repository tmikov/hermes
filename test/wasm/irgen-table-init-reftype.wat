;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm 2>&1 | %FileCheck %s
;; REQUIRES: wasm

;; Test: the two places that write table slots from an element segment pass
;; the DESTINATION TABLE's reference type as their trailing operand -- 1 for a
;; funcref table, 0 for an externref one. The slot funnel reads it to decide
;; whether to brand-check the value: for a funcref table it takes null or a
;; WebAssembly Exported Function and derives the closure and the interned type
;; id from it, and for an externref table it stores whatever it is given.
;;
;; Both used to hardcode 1, which was true only while an element segment could
;; hold nothing but function indices. Now that a segment can hold `ref.null`
;; and `global.get` entries, an externref entry that is not null would have
;; been refused with a TypeError.
;;
;; This file exists to pin those operands, which nothing did when it was
;; written: `grep -rn "HermesBuiltin.wasmTableInit" test/` found no other
;; dumped call, and the only dumped `wasmTableSetSlot` from an element segment
;; was on a funcref table, where the operand is 1 either way. Behaviour is
;; covered by e2e-elem-mixed-items.wat and e2e-elem-passive-items.wat; this
;; file pins the operand each path emits.

(module
  (table $ft 4 funcref)
  (table $et 4 externref)

  (func $f)

  ;; Passive, one per table type, for the two table.init calls below.
  (elem $fseg func $f)
  (elem $eseg externref (item (ref.null extern)))

  ;; Active, one per table type, applied by the instantiate body.
  (elem (table $ft) (i32.const 0) func $f)
  (elem (table $et) (i32.const 0) externref (item (ref.null extern)))

  (func (export "init_funcref")
    (table.init $ft $fseg (i32.const 0) (i32.const 0) (i32.const 1)))
  (func (export "init_externref")
    (table.init $et $eseg (i32.const 0) (i32.const 0) (i32.const 1))))

;; No element expression here is unrecognised, so nothing may trip the guard
;; in BinaryReaderHermesIRGen::EndElemExpr(), which substitutes a null entry
;; and warns when an element expression records none. This line is what makes
;; the `null: null` checks below distinguish a `ref.null` the reader recorded
;; from one the guard supplied: the emitted IR is identical either way, and
;; the warning is the only difference. It runs first, so it scopes from the
;; start of the output, where the warnings would be.
;; CHECK-NOT: warning:

;; table.init into the FUNCREF table. The operand tail is
;; segIdx, dst, src, count, isFuncRef -- pinned as a whole so that the final
;; literal cannot be confused with `count`, which is also 1 here.
;; CHECK-LABEL: function wasm_func_1(): undefined
;; CHECK: CallBuiltinInst (:any) [HermesBuiltin.wasmTableInit]{{.*}}, 0: number, 0: number, 0: number, 1: number, 1: number

;; ...and into the EXTERNREF table, whose segment index is 1 and whose
;; isFuncRef is 0.
;; CHECK-LABEL: function wasm_func_2(): undefined
;; CHECK: CallBuiltinInst (:any) [HermesBuiltin.wasmTableInit]{{.*}}, 1: number, 0: number, 0: number, 1: number, 0: number

;; The active segments, in declaration order, in the instantiate body. The
;; operand tail is idx, value, isFuncRef.
;; CHECK-LABEL: function __wasm_instantiate__(imports: any): object
;; The funcref segment hands over the canonical Exported Function it loaded.
;; CHECK: CallBuiltinInst (:any) [HermesBuiltin.wasmTableSetSlot]{{.*}}, 0: number, %{{[0-9]+}}: any, 1: number
;; The externref segment's `ref.null extern` entry: written, not skipped, so
;; that it erases whatever the slot held, and written without a brand check.
;; CHECK: CallBuiltinInst (:any) [HermesBuiltin.wasmTableSetSlot]{{.*}}, 0: number, null: null, 0: number
