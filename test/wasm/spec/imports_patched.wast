;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Patched version of imports.wast. The patched file (imports_patched.wast_)
;; changes two i32.load instructions from default alignment (align=4) to
;; align=1 to work around Hermes trusting alignment hints. See the comments
;; in imports_patched.wast_ and "Alignment Hints Trusted" in
;; doc/WasmSpecTestStatus.md for details.

;; REQUIRES: wasm
;; RUN: python3 %S/run-spec-test.py --wast2json %wast2json --hermes %hermes %S/imports_patched.wast_ | %FileCheck %s
;; CHECK: SPEC TEST PASSED
