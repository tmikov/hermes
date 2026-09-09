;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The executable form of computeEscapableFuncs()' reasoning about
;; function-body `ref.func`.
;;
;; That function collects no indices for bodies. It relies on Wasm validation
;; instead: the operand of `ref.func` must be in the module's `refs` set -- the
;; function indices that occur outside function bodies -- so a body cannot
;; name a function that does not also appear in an element segment, in a
;; `ref.func` global initializer, or in an export, all three of which already
;; get a canonical Exported Function. If that stops being enforced, an index
;; reaches onRefFunc() with no wrapper variable to load.
;;
;; This module names $g from a body and nowhere else. The Wasm validator
;; rejects it, so `wat2wasm` itself would refuse to emit it; --no-check makes
;; it emit the binary anyway, and `compileWasmModule` refuses it through
;; `validateWasmBinary` before any IR is built.

;; REQUIRES: wasm

;; RUN: %wat2wasm --no-check %s -o %t.wasm
;; RUN: (! %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>&1) | %FileCheck %s

(module
  (func $g (result i32) (i32.const 1))
  (func (export "f")
    ref.func $g
    drop))

;; The message is checked, not just a non-zero exit: a refusal for the wrong
;; reason would pass a check for "Error:" alone. In particular, the second
;; line is what says this was the DECLARATION rule and not, say, an index out
;; of range.
;; CHECK: Error:
;; CHECK-SAME: function 0 is not declared
