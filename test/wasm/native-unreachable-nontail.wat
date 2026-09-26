;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; `unreachable` in non-tail position must compile natively. createResultPhis
;; creates a continuation block's result phis when a control entry is set up,
;; before it is known whether anything will branch there; if the body ends in
;; `unreachable` and nothing branches to the continuation, that block is left
;; with no predecessors and its phis have zero operands, which shermes's
;; pre-optimization verify rejects (hermesc --wasm survives only because DCE
;; deletes the block before its later verify). See dz 01a0d820-0df6.
;;
;; Three shapes, each exercising a different way the dead continuation block
;; can be produced:
;;   tail    -- unreachable followed by dead code in the same block.
;;   inBlock -- unreachable inside a block whose continuation nothing
;;              branches to.
;;   mixed   -- a live path alongside the unreachable one, so the phi ends up
;;              with one real operand instead of zero.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=nontailmod -c -o %t-mod.o %t.wasm
;; RUN: %shermes -o %t.exe -Wc,%t-mod.o %S/native-unreachable-nontail-driver.js_
;; RUN: %t.exe | %FileCheck --match-full-lines %s
;; RUN: %hermesc --wasm -emit-binary -out %t.hbc %t.wasm
;; RUN: %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/native-unreachable-nontail-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; unreachable followed by dead code in the same block.
  (func (export "tail") (result i32) (unreachable) (i32.const 1))
  ;; unreachable inside a block whose continuation nothing branches to.
  (func (export "inBlock") (result i32)
    (block (result i32) (unreachable) (i32.const 2)))
  ;; a live path alongside the unreachable one, so the phi has one real
  ;; operand.
  (func (export "mixed") (param i32) (result i32)
    (block (result i32)
      (local.get 0)
      (if (result i32) (then (i32.const 7)) (else (unreachable) (i32.const 8))))))

;; CHECK: tail(): trapped
;; CHECK-NEXT: inBlock(): trapped
;; CHECK-NEXT: mixed(1): 7
;; CHECK-NEXT: mixed(0): trapped
;; CHECK-NEXT: done
