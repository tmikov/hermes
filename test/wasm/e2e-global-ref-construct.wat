;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The WebAssembly.Global constructor's DESCRIPTOR TABLE for reference types,
;; and the one thing a pure-JS test cannot supply: a real WebAssembly Exported
;; Function, which is the only non-null value an "anyfunc" global accepts.
;;
;; The module is a fixture, not the subject. It contributes an export wrapper
;; built by WasmIRGen and branded by the module's own generated
;; wasmSetFuncInfo call -- a hand-built closure would be the wrong shape and a
;; hand-stamped brand would not prove the constructor agrees with the one
;; production code stamps. Its two numeric global exports are here so that the
;; wasmMakeGlobal mode change is exercised through the compiler as well: an
;; immutable global export takes the SNAPSHOT arm and a mutable one the LIVE
;; arm, which are now selected by isMutable rather than by whether the value
;; happens to be callable. Its externref global IMPORT is how generated code
;; reaches a reference-typed global at all: the driver supplies a JS-built
;; Global for it, so instantiating is itself an assertion that one links.
;;
;; It runs with -gc-sanitize-handles=1 because the funcref brand check
;; ALLOCATES -- isWasmExportedFunction reaches HiddenClass::findPropertyNoMap,
;; which initializes a missing property map -- and because an externref
;; global's value is a GC pointer written and read through paths that did not
;; exist before. The loops in the driver allocate between constructions so
;; that a raw pointer held across either would be a use-after-free rather than
;; a value that happened to survive. In a build without
;; HERMESVM_SANITIZE_HANDLES the flag is ignored and this is an ordinary
;; behavioural test.
;;
;; The write direction is here as well as the read one, now that both setters
;; dispatch per type: a mutable reference snapshot round-trips through
;; `.value`, and the module's own `global.set` on the imported mutable
;; externref global reaches the internal setter and stores whatever it is
;; given -- a Number included, which is an ordinary externref value.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js -gc-sanitize-handles=1 %S/e2e-global-ref-construct-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; A mutable externref global import, satisfied by a JS-built
  ;; WebAssembly.Global -- the route by which generated code reaches the
  ;; internal setter with a reference-typed global.
  (import "e" "ref" (global $eref (mut externref)))

  (func (export "add") (param i32 i32) (result i32)
    (i32.add (local.get 0) (local.get 1)))

  ;; Both directions on the import: global.get hands the host's own value
  ;; back, and global.set writes it through the internal setter.
  (func (export "get_ref") (result externref)
    (global.get $eref))
  (func (export "put_ref") (param externref)
    (global.set $eref (local.get 0)))

  (global (export "g_const") i32 (i32.const 42))
  (global $mut (export "g_mut") (mut i32) (i32.const 100))

  ;; Reads the mutable global through this module's own frame slot, so a
  ;; write made through the exported WebAssembly.Global is observed on the
  ;; Wasm side rather than only read back through the same object.
  (func (export "get_mut") (result i32)
    (global.get $mut))
)

;; The expected output. It lives here rather than in the driver because
;; FileCheck reads this file.
;; CHECK: externref default: undefined / undefined
;; CHECK-NEXT: anyfunc default: null / object
;; CHECK-NEXT: funcref: TypeError: WebAssembly.Global(): 'funcref' is not a value type in the JS API; use 'anyfunc'
;; CHECK-NEXT: v128: TypeError: WebAssembly.Global(): 'v128' requires SIMD, which is not supported
;; CHECK-NEXT: bogus: TypeError: WebAssembly.Global(): 'value' must be 'i32', 'i64', 'f32', 'f64', 'externref' or 'anyfunc'
;; CHECK-NEXT: anyfunc null is null: true
;; CHECK-NEXT: anyfunc from a plain function: TypeError: WebAssembly.Global(): an 'anyfunc' global requires null or a WebAssembly exported function
;; CHECK-NEXT: anyfunc explicit undefined: TypeError: WebAssembly.Global(): an 'anyfunc' global requires null or a WebAssembly exported function
;; CHECK-NEXT: anyfunc from an object: TypeError: WebAssembly.Global(): an 'anyfunc' global requires null or a WebAssembly exported function
;; CHECK-NEXT: externref stores every JS value as it stands: true
;; CHECK-NEXT: mutable reference snapshots hold their value: true true
;; CHECK-NEXT: immutable externref write: TypeError: WebAssembly.Global.prototype.value: cannot set an immutable global
;; CHECK-NEXT: mutable externref write: true
;; CHECK-NEXT: mutable externref write null: true
;; CHECK-NEXT: mutable anyfunc write a plain function: TypeError: WebAssembly.Global.prototype.value: an 'anyfunc' global requires null or a WebAssembly exported function
;; CHECK-NEXT: mutable numeric snapshot round-trips: 2
;; CHECK-NEXT: externref values traced across collections: true
;; CHECK-NEXT: fixture: function 5
;; CHECK-NEXT: anyfunc from a real export: true
;; CHECK-NEXT: externref holds an export too: true
;; CHECK-NEXT: funcref constructions intact: true
;; CHECK-NEXT: anyfunc write an export: true
;; CHECK-NEXT: anyfunc write back to null: true
;; CHECK-NEXT: g_const: 42 TypeError: WebAssembly.Global.prototype.value: cannot set an immutable global
;; CHECK-NEXT: g_mut: 100 / 100
;; CHECK-NEXT: g_mut after a host write: 7 / 7
;; CHECK-NEXT: global.get sees the host value: true
;; CHECK-NEXT: put_ref an object: true true
;; CHECK-NEXT: put_ref a number: true true
;; CHECK-NEXT: put_ref undefined: true
;; CHECK-NEXT: put_ref never coerces: true true true
;; CHECK-NEXT: .value write is seen by global.get: true
