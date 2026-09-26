;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A Wasm unit registers itself under its --exported-unit name, so that
;; WebAssembly.Module.fromNativeUnit() can find it with no embedder code.
;;
;; The name comes from the exported-unit option, NOT from SHUnit::unit_name,
;; which the backend hardcodes to "sh_compiled" (SH.cpp:3184).

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=mymod -emit-c -o - %t.wasm | %FileCheck %s

(module
  (func (export "nop")))

;; CHECK: static SHWasmUnitReg s_wasm_reg
;; CHECK-SAME: "mymod"
;; CHECK-SAME: sh_export_mymod
;; CHECK: _sh_wasm_register_unit(&s_wasm_reg)
