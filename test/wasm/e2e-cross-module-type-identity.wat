;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; call_indirect must compare type identity ACROSS modules once tables are
;; shared. It compared each module's own dense type-section numbering, which is
;; meaningless in another module, so both directions were wrong:
;;
;;   - the same signature numbered differently in two modules trapped, even
;;     though the call was perfectly valid;
;;   - two different signatures that happened to land on the same ordinal
;;     matched, so call_indirect invoked a function with the wrong signature
;;     instead of trapping -- defeating the check it exists to perform.
;;
;; This module deliberately declares an unused type FIRST, so its numbering is
;; shifted relative to the exporter's. Identity is now an interned id derived
;; from the structural type string, which agrees regardless of declaration
;; order.

;; REQUIRES: wasm
;; RUN: %wat2wasm %S/e2e-cross-module-type-identity-exporter.wat_ -o %t-exp.wasm && %hermesc --wasm -emit-binary -out %t-exp.hbc %t-exp.wasm && %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-cross-module-type-identity-driver.js_ -- %t-exp.hbc %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; Unused, but declared first: shifts this module's numbering so that
  ;; $v_i32 is index 1 here and index 0 in the exporter.
  (type $dummy (func (param f64) (result f64)))
  (type $v_i32 (func (result i32)))
  (import "exporter" "tbl" (table 2 funcref))
  (func (export "call_at") (param i32) (result i32)
    (call_indirect (type $v_i32) (local.get 0))))

;; Same signature, different local numbering: must succeed, not trap.
;; CHECK: exporter call_at(0): 100
;; CHECK-NEXT: importer call_at(0): 100

;; Slot 1 holds (i32,i32)->i64. Calling it through a ()->i32 call site must
;; trap. Before the fix the ordinals collided and the wrong function ran.
;; CHECK-NEXT: importer call_at(1): trapped
;; CHECK-NEXT: done
