;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The INTERNAL setter -- the wasmGlobalSet builtin -- with reference types.
;;
;; It is a different function from WebAssembly.Global.prototype.value's setter
;; and has a different contract: the public one COERCES with ToNumber, this
;; one VALIDATES and refuses. Nothing that goes through the public setter
;; reaches it, and a module's write to a global it DEFINES does not either --
;; that stores straight into the frame slot. What reaches it is a module
;; writing a global it IMPORTED as mutable, which needs two modules, and that
;; is what this file is.
;;
;; This module is the provider. Its two exported mutable globals are published
;; LIVE, so a write from the consumer travels wasmGlobalSet -> this module's
;; setter closure -> this module's frame slot, and get_ext/get_fn read that
;; slot. Reading the WebAssembly.Global back instead would pass against a
;; write that never arrived anywhere the provider can see.
;;
;; The consumer also writes from its START function, which runs during
;; instantiation rather than from a later call, so the builtin is exercised on
;; the path where no export has been handed out yet. get_ext reads null before
;; instantiation and the seed object after, which is what makes that
;; assertion mean something.
;;
;; The consumer's `jsfn` import is a JS-built mutable anyfunc Global, whose
;; storage is its own slot rather than a closure: that is the snapshot arm of
;; the same builtin, where the value goes through the storage funnel.
;;
;; What this file can and cannot ask of the builtin's funcref REFUSAL changed
;; when export-wrapper parameters started converting. `put_fn` and `put_jsfn`
;; are exported `(param funcref)` functions, so a value that is neither null
;; nor an Exported Function is now refused at the parameter, before the body
;; runs and therefore before wasmGlobalSet is called at all -- which is what
;; the refusal messages below say. The builtin's own refusal is asked directly
;; in unittests/VMRuntime/WasmBuiltinTest.cpp, which calls it with values that
;; no longer reach it from the modules here. What this file still asks of the
;; builtin is the accepting half: null and an Exported Function reach it and
;; land in the destination.
;;
;; It runs with -gc-sanitize-handles=1 because the funcref brand check
;; ALLOCATES -- isWasmExportedFunction reaches HiddenClass::findPropertyNoMap,
;; which initializes a missing property map. In the builtin it sits between
;; the entry and both stores, and it runs for an accepted Exported Function
;; too, not only for a refused value; the parameter conversion in the export
;; wrapper asks the same question through the wasmIsExportedFunction builtin,
;; so put_fn(h) now performs two potentially allocating brand checks on its
;; way to the slot. (Whether either one actually allocates depends on the
;; property map already being there -- HiddenClass::findPropertyNoMap builds
;; one only when it is missing -- which is why the requirement is rooting
;; across the call rather than a count.) The loop at the
;; end allocates between writes. In a build without HERMESVM_SANITIZE_HANDLES
;; the flag is ignored and this is an ordinary behavioural test.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %wat2wasm %S/e2e-global-ref-internal-setter-consumer.wat_ -o %t-c.wasm && %hermesc --wasm -emit-binary -out %t-c.hbc %t-c.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js -gc-sanitize-handles=1 %S/e2e-global-ref-internal-setter-driver.js_ -- %t.hbc %t-c.hbc | %FileCheck --match-full-lines %s

(module
  ;; A real Exported Function: the only non-null value a funcref accepts.
  (func (export "h") (param i32) (result i32)
    (i32.add (local.get 0) (i32.const 1)))

  (global $ge (export "g_ext") (mut externref) (ref.null extern))
  (global $gf (export "g_fn") (mut funcref) (ref.null func))

  ;; Read the frame slots the consumer's writes have to land in.
  (func (export "get_ext") (result externref) (global.get $ge))
  (func (export "get_fn") (result funcref) (global.get $gf))
)

;; The expected output. It lives here rather than in the driver because
;; FileCheck reads this file.
;; CHECK: fixture: function true
;; CHECK-NEXT: before instantiation: true true
;; CHECK-NEXT: the start function wrote during instantiation: true
;; CHECK-NEXT: put_ext an object: true true
;; CHECK-NEXT: put_ext a number: true
;; CHECK-NEXT: put_ext null: true
;; CHECK-NEXT: put_ext undefined: true
;; CHECK-NEXT: put_ext never coerces: true true true
;; CHECK-NEXT: put_fn an export: true true
;; CHECK-NEXT: put_fn a plain function: TypeError: Wasm call: funcref argument 0 requires null or a WebAssembly exported function
;; CHECK-NEXT: put_fn a number: TypeError: Wasm call: funcref argument 0 requires null or a WebAssembly exported function
;; CHECK-NEXT: put_fn undefined: TypeError: Wasm call: funcref argument 0 requires null or a WebAssembly exported function
;; The parameter conversion refused before the body ran, so nothing reached
;; the provider's frame slot.
;; CHECK-NEXT: put_fn intact after the refusals: true
;; CHECK-NEXT: put_fn null: true
;; CHECK-NEXT: put_jsfn null: true
;; CHECK-NEXT: put_jsfn an export: true
;; CHECK-NEXT: put_jsfn a plain function: TypeError: Wasm call: funcref argument 0 requires null or a WebAssembly exported function
;; CHECK-NEXT: put_jsfn intact after the refusal: true
;; CHECK-NEXT: writes survive collection: true
