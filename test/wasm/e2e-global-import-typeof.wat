;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; What a raw (non-WebAssembly.Global) value may satisfy is decided by the
;; declared global type, per the JS-API spec's ToWebAssemblyValue rules.
;; The check used to accept typeof "number" OR "bigint" for every global
;; type, so a BigInt satisfied an i32 import and a Number an i64 one --
;; each then failing later, deep in initialization, as a TypeError instead
;; of a LinkError naming the import. A raw value also allocates an
;; *immutable* global per spec, so it can never satisfy a mutable global
;; import; that rule was absent entirely and raw values linked against
;; (mut i32) and (mut i64) imports.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %wat2wasm %S/e2e-global-import-typeof-i64.wat_ -o %t-i64.wasm && %hermesc --wasm -emit-binary -out %t-i64.hbc %t-i64.wasm && %wat2wasm %S/e2e-global-import-typeof-mut.wat_ -o %t-mut.wasm && %hermesc --wasm -emit-binary -out %t-mut.hbc %t-mut.wasm && %wat2wasm %S/e2e-global-import-typeof-mut64.wat_ -o %t-mut64.wasm && %hermesc --wasm -emit-binary -out %t-mut64.hbc %t-mut64.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-global-import-typeof-driver.js_ -- %t.hbc %t-i64.hbc %t-mut.hbc %t-mut64.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "g" (global i32))
  (func (export "get") (result i32) global.get 0))

;; A raw value must match the declared type exactly.
;; CHECK: i32 <- 42: linked, get() = 42
;; CHECK-NEXT: i32 <- 1n: LinkError: import e.g must be a Number to satisfy an i32 global import
;; CHECK-NEXT: i64 <- 5n: linked, get() = 5
;; CHECK-NEXT: i64 <- 5: LinkError: import e.g must be a BigInt to satisfy an i64 global import

;; A raw value allocates an immutable global, so it can never satisfy a
;; mutable global import, whatever its type.
;; CHECK-NEXT: mut i32 <- 7: LinkError: import e.g must be a WebAssembly.Global to satisfy a mutable global import
;; CHECK-NEXT: mut i32 <- Global(mut, 7): linked, get() = 7
;; CHECK-NEXT: mut i64 <- 9n: LinkError: import e.g must be a WebAssembly.Global to satisfy a mutable global import
;; CHECK-NEXT: mut i64 <- Global(mut, 9n): linked, get() = 9
;; CHECK-NEXT: done
