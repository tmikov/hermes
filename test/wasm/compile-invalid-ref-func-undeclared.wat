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
;; RUN: (! %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>&1) | %FileCheck --match-full-lines %s

(module
  (func $g (result i32) (i32.const 1))
  (func (export "f")
    ref.func $g
    drop))

;; The WHOLE line is pinned, with --match-full-lines and a single CHECK, so
;; that a refusal for the wrong reason cannot pass. The message is what says
;; this was the DECLARATION rule and not, say, an index out of range.
;;
;; This used to be CHECK plus CHECK-SAME, which is barely stronger than
;; "Error:" alone -- CHECK-SAME allows arbitrary text between and after its
;; fragments, so a line reading `Error: parse failure at byte 27; unrelated:
;; function 0 is not declared, BUT the real cause was something else` blames
;; something else and still satisfies it. Measured, not reasoned: that line
;; was run through both forms.
;;
;; The byte offset stays a regex. wabt reports the position in the module, so
;; pinning it literally would tie this test to the encoding rather than to the
;; diagnostic -- the same reason the v128 export tests skip wabt's preceding
;; line.
;; CHECK: Error: <validate>:{{[0-9a-f]+}}: error: function 0 is not declared in any elem sections
