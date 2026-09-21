;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Re-exporting an IMPORTED mutable v128 global is refused too.
;;
;; This is a separate path from the one in
;; compile-invalid-v128-global-export.wat, not a second dressing of it. The
;; export loop in finalizeModule() publishes an imported mutable global by
;; storing the object it arrived as and skipping the rest of the iteration,
;; above the point where it derives the global's type -- so a check written
;; next to that derivation would let this module through.
;; validateGlobalExportTypes() runs before the loop and derives the type for
;; each exported global index, imported or defined, which is why this one is
;; refused as well.
;;
;; Emitted without --no-check: a SIMD global is standard Wasm and wabt enables
;; the feature by default, so this module is valid and the refusal is Hermes
;; IRGen's own.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: (! %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>&1) | %FileCheck --match-full-lines %s

(module
  (import "e" "g" (global $g (mut v128)))
  (export "g" (global $g)))

;; The WHOLE line is pinned, with --match-full-lines and a single CHECK, for
;; the reason spelled out in compile-invalid-v128-global-export.wat: neither a
;; bare "Error:" check nor a CHECK plus CHECK-SAME fragments distinguishes
;; this refusal from one that blames something else on the same line.
;;
;; Measured: with validateGlobalExportTypes() made to return true
;; unconditionally -- the state of the tree before this diagnostic -- hermesc
;; compiles this module and exits 0, and this test goes red on empty FileCheck
;; input.
;; CHECK: Error: exported global "g" has type v128, and SIMD is not supported
