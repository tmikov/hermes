;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test peephole optimization of CondBranchInst through AsInt32Inst.
;;
;; Wasm comparisons produce i32 (0 or 1), so WasmIRGen wraps the boolean
;; result in AsInt32Inst.  When the i32 feeds into br_if or if, the
;; CondBranchInst should use the boolean comparison result directly,
;; bypassing the AsInt32Inst.
;;
;; At -O0 the AsInt32Inst is still emitted (just bypassed).
;; At -O  DCE removes the dead AsInt32Inst entirely.

;; REQUIRES: wasm

;; -O0: AsInt32Inst present but CondBranchInst uses boolean directly.
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck --check-prefix=CHKO0 %s

;; -O: Dead AsInt32Inst removed by DCE.
;; RUN: %hermesc --wasm --dump-ir -O %t.wasm | %FileCheck --check-prefix=CHKOPT %s

(module
  ;; comparison + br_if
  (func $br_if_cmp (export "br_if_cmp") (param i32) (param i32) (result i32)
    (block $exit (result i32)
      i32.const 10
      local.get 0
      local.get 1
      i32.lt_s
      br_if $exit
      drop
      i32.const 20
    ))

  ;; comparison + if
  (func $if_cmp (export "if_cmp") (param i32) (param i32) (result i32)
    local.get 0
    local.get 1
    i32.eq
    if (result i32)
      i32.const 1
    else
      i32.const 0
    end)
)

;; --- -O0 checks: AsInt32Inst present but bypassed by CondBranchInst ---

;; CHKO0-LABEL: function wasm_func_0(p0: number, p1: number): number
;; CHKO0:   %[[CMP0:.*]] = FLessThanInst (:boolean)
;; CHKO0:   %{{.*}} = AsInt32Inst (:number) %[[CMP0]]: boolean
;; CHKO0:        CondBranchInst %[[CMP0]]: boolean,

;; CHKO0-LABEL: function wasm_func_1(p0: number, p1: number): number
;; CHKO0:   %[[CMP1:.*]] = FEqualInst (:boolean)
;; CHKO0:   %{{.*}} = AsInt32Inst (:number) %[[CMP1]]: boolean
;; CHKO0:        CondBranchInst %[[CMP1]]: boolean,

;; --- -O checks: Dead AsInt32Inst removed, CondBranchInst uses boolean ---

;; CHKOPT-LABEL: function wasm_export_br_if_cmp
;; CHKOPT:   %[[CMP2:.*]] = FLessThanInst (:boolean)
;; CHKOPT-NOT: AsInt32Inst {{.*}} %[[CMP2]]
;; CHKOPT:        CondBranchInst %[[CMP2]]: boolean,

;; CHKOPT-LABEL: function wasm_export_if_cmp
;; CHKOPT:   %[[CMP3:.*]] = FEqualInst (:boolean)
;; CHKOPT-NOT: AsInt32Inst {{.*}} %[[CMP3]]
;; CHKOPT:        CondBranchInst %[[CMP3]]: boolean,
