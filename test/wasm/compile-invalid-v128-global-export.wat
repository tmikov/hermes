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
;; What the diagnostic MOVED, rather than what it prevents: no bad Global was
;; ever published. globalValTypeCode maps v128 to 0xFF and wasmMakeGlobal
;; already rejected that, so this module used to fail at INSTANTIATION with
;; "TypeError: wasmMakeGlobal: unknown value type". It now fails at COMPILE
;; time with a message that names the export and the reason -- which is what
;; this test pins.
;;
;; This module is valid Wasm: wabt enables the SIMD feature by default, so
;; wat2wasm emits it without --no-check and `validateWasmBinary` accepts it
;; (WebAssembly.validate answers true for these bytes). The refusal is Hermes
;; IRGen's own, from validateGlobalExportTypes().

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: (! %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>&1) | %FileCheck --match-full-lines %s

(module
  (global (export "g") v128 (v128.const i32x4 1 2 3 4)))

;; The WHOLE line is pinned, with --match-full-lines and a single CHECK.
;;
;; Two weaker shapes were tried and rejected. A check for "Error:" alone
;; passes on any refusal, including a parse failure. Splitting the message
;; into CHECK plus CHECK-SAME fragments is not much better: CHECK-SAME allows
;; arbitrary text between and after the fragments, so a line reading
;; `Error: parse failure at byte 27; unrelated: exported global "g" has type
;; v128 BUT ACTUALLY the real cause was something else and SIMD is not
;; supported anyway` satisfies it -- a refusal that explicitly blames
;; something else, passing the assertion meant to rule that out. Measured,
;; not reasoned: that line was run through both forms.
;;
;; Measured the other way too: with validateGlobalExportTypes() made to return
;; true unconditionally -- the state of the tree before this diagnostic --
;; hermesc compiles this module and exits 0, and this test goes red on empty
;; FileCheck input.
;;
;; wabt prints a line of its own before this one, of the form
;; "<offset>: error: EndModule callback failed" -- a finalizeModule() refusal
;; reaches the driver as a failed binary read, and wabt reports that first.
;; It is not checked: CHECK scans forward, and --match-full-lines constrains
;; the line it lands on, not the lines before it. Its offset is a position in
;; the module, so checking it would pin this test to the encoding rather than
;; to the diagnostic.
;; CHECK: Error: exported global "g" has type v128, and SIMD is not supported
