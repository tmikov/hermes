;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A module with a v128 tag parameter is refused at compile time.
;;
;; Same shape as compile-invalid-v128-global-export.wat, and for the same
;; reason: Hermes has no v128 value, so JSWebAssemblyTag::ValType has no v128
;; member and globalValTypeCode -- which the tag parameter codes now come
;; from -- maps v128 to 0xFF. wasmMakeTag rejects that code.
;;
;; What this diagnostic MOVES, rather than what it prevents. Without it the
;; module compiles and dies at INSTANTIATION with
;; "TypeError: wasmMakeTag: bad value type code", an internal builtin's
;; message naming nothing the author wrote. That is exactly what the global
;; export case did before its own check existed.
;;
;; Every tag is checked, not only exported ones: createTagObjects() builds an
;; object for each DEFINED tag whether or not it is exported, and the import
;; check passes the same codes for each IMPORTED one, so either kind reaches
;; the builtin.

;; REQUIRES: wasm

;; RUN: %wat2wasm --enable-exceptions %s -o %t.wasm
;; RUN: (! %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>&1) | %FileCheck --match-full-lines %s

(module
  (tag (param v128)))

;; The whole line is pinned, with --match-full-lines and a single directive.
;; Matching "Error" alone would pass on any refusal, including a parse
;; failure, and splitting the message into CHECK-SAME fragments admits
;; arbitrary text between and after them.
;;
;; The tag is not exported and has no name, so the diagnostic names its index
;; -- which is what an author reading it has to go on.
;; CHECK: Error: tag 0 has a v128 parameter, and SIMD is not supported
