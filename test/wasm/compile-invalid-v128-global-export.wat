;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A module that exports a v128 global is refused at compile time.
;;
;; The JS API says such a module should instantiate: the exports object gets a
;; WebAssembly.Global whose `.value` throws a TypeError, so the export is
;; present but unreadable. Hermes cannot build that object today --
;; JSWebAssemblyGlobal::ValType has no v128 member, and
;; BinaryReaderHermesIRGen has no v128.const initializer handler, so the
;; module's slot for this global would hold the number 0 -- so the module is
;; refused with a diagnostic naming SIMD instead. dz 01a07d4b-01bc tracks
;; building the real shell, and removing this diagnostic is part of that work.
;;
;; This module is valid Wasm: wabt enables the SIMD feature by default, so
;; wat2wasm emits it without --no-check and `validateWasmBinary` accepts it
;; (WebAssembly.validate answers true for these bytes). The refusal is Hermes
;; IRGen's own, from validateGlobalExportTypes().

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: (! %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>&1) | %FileCheck %s

(module
  (global (export "g") v128 (v128.const i32x4 1 2 3 4)))

;; The message is checked, not just a non-zero exit. Measured: with
;; validateGlobalExportTypes() made to return true unconditionally -- the
;; state of the tree before this diagnostic -- hermesc compiles this module
;; and exits 0, and this test goes red on empty FileCheck input. And a
;; refusal for some other reason -- a parse failure, an out-of-range export
;; index -- would pass a check for "Error:" alone; the clause naming SIMD is
;; what says the engine refused the TYPE rather than tripping over the
;; module.
;; CHECK: Error:
;; CHECK-SAME: exported global "g" has type v128
;; CHECK-SAME: SIMD is not supported
