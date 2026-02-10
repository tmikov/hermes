;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; REQUIRES: wasm
;; XFAIL: *
;; RUN: python3 %S/run-spec-test.py --wast2json %wast2json --hermes %hermes %wasm_testsuite/func.wast | %FileCheck %s
;; CHECK: SPEC TEST PASSED
