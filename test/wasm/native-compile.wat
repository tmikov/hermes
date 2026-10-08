;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; shermes compiles a .wasm the way hermesc does: the Wasm frontend populates
;; a Module, which then goes through the ordinary optimization and SH
;; pipeline. Nothing about the IR is backend-specific.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=addmod -emit-c -o - %t.wasm | %FileCheck %s
;;
;; This test suite has no "not" tool wired into its lit substitutions, and
;; the generated script runs under "set -o pipefail", so a plain pipe into
;; FileCheck would fail the whole line on shermes's own nonzero exit even
;; when FileCheck matches. Capture to a file and check it separately
;; instead, so shermes's exit code doesn't gate the line and FileCheck's
;; match still does.
;; RUN: %shermes -emit-c -o - %t.wasm > %t.nounit.out 2>&1; true
;; RUN: %FileCheck --check-prefix=NOUNIT %s < %t.nounit.out
;; RUN: %shermes -exported-unit=a -dump-ast -o - %t.wasm > %t.noast.out 2>&1; true
;; RUN: %FileCheck --check-prefix=NOAST %s < %t.noast.out

(module
  (func (export "add") (param i32 i32) (result i32)
    (i32.add (local.get 0) (local.get 1))))

;; The Wasm top level is emitted, and the unit is named. Function bodies are
;; emitted BEFORE the CREATE_THIS_UNIT define (SH.cpp:3126 then 3148), so the
;; instantiate function must be checked first.
;; CHECK: __wasm_instantiate__
;; CHECK: #define CREATE_THIS_UNIT sh_export_addmod

;; Wasm input without --exported-unit is refused: the only supported output
;; is a linkable unit.
;; NOUNIT: --exported-unit

;; And an AST/sema-only output mode is refused rather than falling through to
;; a backend that asserts on it.
;; NOAST: not supported for WebAssembly
