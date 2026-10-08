;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; What WebAssembly.Module does with a .hbc image, across both gates. Only
;; when the embedder has opted into content-sniffing AND into untrusted
;; bytecode from JS is the image executed; with sniffing alone it is detected
;; and explicitly refused, and with sniffing off it is just an invalid .wasm.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods %S/e2e-sniff-gate-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s
;; RUN: %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-sniff-gate-driver.js_ -- %t.hbc | %FileCheck --check-prefix=UNTRUSTED --match-full-lines %s
;; RUN: %hermes -Xhermes-internal-test-methods -Xenable-wasm-bytecode-content-sniffing %S/e2e-sniff-gate-driver.js_ -- %t.hbc | %FileCheck --check-prefix=SNIFF --match-full-lines %s
;; RUN: %hermes -Xhermes-internal-test-methods -Xenable-wasm-bytecode-content-sniffing -Xenable-untrusted-bytecode-from-js %S/e2e-sniff-gate-driver.js_ -- %t.hbc | %FileCheck --check-prefix=BOTH --match-full-lines %s

(module (func (export "f") (result i32) (i32.const 7)))

;; Neither gate: not sniffed, so the .hbc is an invalid .wasm binary.
;; CHECK: hbc bytes to WebAssembly.Module: CompileError (other)
;; CHECK-NEXT: done

;; Untrusted bytecode allowed, but nothing sniffs, so still invalid .wasm.
;; UNTRUSTED: hbc bytes to WebAssembly.Module: CompileError (other)
;; UNTRUSTED-NEXT: done

;; Sniffed and recognized as .hbc, but untrusted bytecode is off: refused.
;; SNIFF: hbc bytes to WebAssembly.Module: CompileError (refused)
;; SNIFF-NEXT: done

;; Both gates on: the bytecode is loaded and runs.
;; BOTH: hbc bytes to WebAssembly.Module: loaded, f() = 7
;; BOTH-NEXT: done
