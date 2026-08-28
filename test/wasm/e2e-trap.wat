;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test that the Wasm unreachable instruction causes a runtime trap.
;; F.1: WasmHelpers infrastructure -- wasmTrap builtin.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck --check-prefix=IRCHECK %s
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && ! %hermes -Xhermes-internal-test-methods %S/instantiate-hbc.js_ -- %t.hbc 2>&1 | %FileCheck --check-prefix=RUNCHECK %s

(module
  (func $trap_func
    unreachable)
  (start $trap_func))

;; Verify IR: unreachable emits CallBuiltinInst(wasmTrap) + UnreachableInst.
;; IRCHECK-LABEL: function wasm_func_0(): any
;; IRCHECK:        CallBuiltinInst {{.*}}[HermesBuiltin.wasmTrap]
;; IRCHECK-NEXT:   UnreachableInst

;; Verify runtime: executing unreachable throws an Error.
;; RUNCHECK: Error: unreachable executed
