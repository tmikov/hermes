;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; An export names an index into one of the module's five index spaces, and a
;; malformed module can name one that does not exist. `hermesc --wasm` DOES NOT
;; VALIDATE its input -- compileWasmModule() runs wabt::ReadBinary only, never
;; wabt::ValidateModule (H19) -- so such a module reaches WasmIRGen's export
;; loops directly. The table case was a heap-buffer-overflow READ under ASan:
;;
;;   ==ERROR: AddressSanitizer: heap-buffer-overflow READ of size 1
;;       #0 WasmIRGen::finalizeModule() WasmIRGen.cpp:2159
;;       #1 BinaryReaderHermesIRGen::EndModule() BinaryReaderHermesIRGen.cpp:1738
;;
;; the global case was a bare `assert`, which is not a diagnostic in a release
;; build, and the tag case had no check at all. All five are now refused by
;; WasmIRGen::validateExportIndices() with a message naming the export, the
;; index space, the bad index and the real count.
;;
;; HOW THE MALFORMED MODULES ARE MADE. `wat2wasm` will not emit one, so each
;; module below is assembled normally and then has its LAST BYTE overwritten
;; with 5. In every one of these five modules the export section is the final
;; section and the export's index is its final byte -- a `.wat` with no code
;; section, which is why the function case imports its function instead of
;; defining one. What keeps a layout change from quietly voiding this test is
;; NOT the unpatched compile -- that still succeeds if the patch lands in a
;; later section, as was checked by appending a code section. It is the
;; full-message CHECK below: a byte patched into the wrong section yields the
;; generic "Failed to parse Wasm binary", which no expected line matches. The
;; unpatched compile stays because it proves the module is otherwise valid, so
;; a failure is attributable to the patch.

;; REQUIRES: wasm

;; The table case -- the ASan repro above -- is this file's own module.
;; RUN: %wat2wasm %s -o %t-table.wasm
;; RUN: %hermesc --wasm -emit-binary -out %t.hbc %t-table.wasm
;; RUN: printf '\5' | dd of=%t-table.wasm bs=1 seek=$(($(wc -c < %t-table.wasm) - 1)) conv=notrunc 2>/dev/null
;; RUN: (! %hermesc --wasm -emit-binary -out %t.hbc %t-table.wasm 2>&1) | %FileCheck --check-prefix=TABLE %s

;; RUN: %wat2wasm %S/compile-export-index-oob-global.wat_ -o %t-global.wasm
;; RUN: %hermesc --wasm -emit-binary -out %t.hbc %t-global.wasm
;; RUN: printf '\5' | dd of=%t-global.wasm bs=1 seek=$(($(wc -c < %t-global.wasm) - 1)) conv=notrunc 2>/dev/null
;; RUN: (! %hermesc --wasm -emit-binary -out %t.hbc %t-global.wasm 2>&1) | %FileCheck --check-prefix=GLOBAL %s

;; RUN: %wat2wasm --enable-exceptions %S/compile-export-index-oob-tag.wat_ -o %t-tag.wasm
;; RUN: %hermesc --wasm -emit-binary -out %t.hbc %t-tag.wasm
;; RUN: printf '\5' | dd of=%t-tag.wasm bs=1 seek=$(($(wc -c < %t-tag.wasm) - 1)) conv=notrunc 2>/dev/null
;; RUN: (! %hermesc --wasm -emit-binary -out %t.hbc %t-tag.wasm 2>&1) | %FileCheck --check-prefix=TAG %s

;; RUN: %wat2wasm %S/compile-export-index-oob-func.wat_ -o %t-func.wasm
;; RUN: %hermesc --wasm -emit-binary -out %t.hbc %t-func.wasm
;; RUN: printf '\5' | dd of=%t-func.wasm bs=1 seek=$(($(wc -c < %t-func.wasm) - 1)) conv=notrunc 2>/dev/null
;; RUN: (! %hermesc --wasm -emit-binary -out %t.hbc %t-func.wasm 2>&1) | %FileCheck --check-prefix=FUNC %s

;; RUN: %wat2wasm %S/compile-export-index-oob-memory.wat_ -o %t-mem.wasm
;; RUN: %hermesc --wasm -emit-binary -out %t.hbc %t-mem.wasm
;; RUN: printf '\5' | dd of=%t-mem.wasm bs=1 seek=$(($(wc -c < %t-mem.wasm) - 1)) conv=notrunc 2>/dev/null
;; RUN: (! %hermesc --wasm -emit-binary -out %t.hbc %t-mem.wasm 2>&1) | %FileCheck --check-prefix=MEM %s

(module (table 1 funcref) (export "tt" (table 0)))

;; The message names the index space and BOTH numbers. A check for "Error:"
;; alone would pass on any refusal at all, including the generic "Failed to
;; parse Wasm binary" that a truncated file produces -- which is what this
;; used to be, before the reader's failure carried a reason.
;; TABLE: Error: export "tt" names table index 5, but the module has 1 of them
;; GLOBAL: Error: export "gg" names global index 5, but the module has 1 of them
;; TAG: Error: export "ee" names tag index 5, but the module has 1 of them
;; FUNC: Error: export "ff" names function index 5, but the module has 1 of them
;; MEM: Error: export "mm" names memory index 5, but the module has 1 of them
