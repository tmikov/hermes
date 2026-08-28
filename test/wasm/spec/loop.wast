;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; REQUIRES: wasm
;; XFAIL: *
;; RUN: python3 %S/run-spec-test.py --wast2json %wast2json --hermes %hermes %wasm_testsuite/loop.wast | %FileCheck %s
;; CHECK: SPEC TEST PASSED

;; Remaining failures: f32 precision in "nesting" tests (lines 519-520),
;; and missing assert_invalid validation (lines 601+).
