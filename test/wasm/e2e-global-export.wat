;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; End-to-end test for global exports wrapped as WebAssembly.Global objects.
;;
;; This file used to assert the PRESENCE of a __wasm_type__ string such as
;; "global:i32:const" on each exported global, because that string WAS the
;; cross-module type check. Being an ordinary own property, it was also the
;; whole of the forgery: `{__wasm_type__: 'global:i32:const', value: 1234}`
;; linked and handed the importing module 1234.
;;
;; The type and the mutability now live in the Global's internal fields and
;; are compared by the wasmLinkGlobal brand check, so there is nothing to
;; assert the presence of. What this file pins instead is that BOTH halves of
;; that comparison still discriminate -- a mutable global must not satisfy an
;; immutable import, nor an f64 global an i32 one -- because a brand check
;; that ignored either half would still let every genuine global link, and
;; every positive assertion here would go on passing.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %wat2wasm %S/e2e-global-export-const-consumer.wat_ -o %t-const.wasm && %hermesc --wasm -emit-binary -out %t-const.hbc %t-const.wasm && %wat2wasm %S/e2e-global-export-mut-consumer.wat_ -o %t-mut.wasm && %hermesc --wasm -emit-binary -out %t-mut.hbc %t-mut.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-global-export-driver.js_ -- %t.hbc %t-const.hbc %t-mut.hbc | %FileCheck --match-full-lines %s

(module
  (global (export "g_i32") i32 (i32.const 42))
  (global (export "g_f64") f64 (f64.const 3.14))
  (global (export "g_mut") (mut i32) (i32.const 100))
)

;; Each export is a real WebAssembly.Global carrying no metadata at all.
;; CHECK: g_i32 type: object
;; CHECK-NEXT: g_i32 own props: []
;; CHECK-NEXT: g_i32 JSON: {}
;; CHECK-NEXT: g_i32 __wasm_type__: undefined
;; CHECK-NEXT: g_i32 value: 42
;; CHECK-NEXT: g_f64 type: object
;; CHECK-NEXT: g_f64 own props: []
;; CHECK-NEXT: g_f64 value: 3.14
;; CHECK-NEXT: g_mut type: object
;; CHECK-NEXT: g_mut own props: []
;; CHECK-NEXT: g_mut value: 100

;; Mutability survives the export, and is now observable only through the
;; spec's own accessor.
;; CHECK-NEXT: g_i32.value = 1: TypeError
;; CHECK-NEXT: g_mut.value = 1: 1

;; Cross-module linking: an immutable i32 import takes g_i32 and nothing else
;; of the three.
;; CHECK-NEXT: const import <- g_i32: 42
;; CHECK-NEXT: const import <- g_f64: LinkError: import e.g is a WebAssembly.Global that does not match the declared immutable i32 global import
;; CHECK-NEXT: const import <- g_mut: LinkError: import e.g is a WebAssembly.Global that does not match the declared immutable i32 global import

;; And a mutable i32 import takes g_mut and nothing else -- not even a raw
;; number, which cannot be shared state.
;; CHECK-NEXT: mut import <- g_mut: 1
;; CHECK-NEXT: mut import <- g_i32: LinkError: import e.g is a WebAssembly.Global that does not match the declared mutable i32 global import
;; CHECK-NEXT: mut import <- 5: LinkError: import e.g must be a WebAssembly.Global to satisfy a mutable global import
;; CHECK-NEXT: done
