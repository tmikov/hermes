;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Reference-typed global EXPORTS: the direction the JS-API constructor tests
;; do not cover, and the one a module produces on its own.
;;
;; The globals this module exports are wrapped by the wasmMakeGlobal builtin,
;; which bounded its type code with `rawCode > ValType::F64` -- an ordering
;; comparison, so no -Wswitch could name it. Both reference codes are above
;; that bound, and instantiating this module threw "wasmMakeGlobal: unknown
;; value type". The bound is now the highest enumerator. An immutable funcref
;; export also needs the builtin to read its mode from isMutable: such a
;; global's snapshot VALUE is an Exported Function, and while the mode was
;; "is argument 2 callable" it was read as a live global's getter closure and
;; refused.
;;
;; The identity assertion is the load-bearing one. g_func must be the module's
;; OWN exported function -- the canonical wrapper, the same object `h` names --
;; because a stand-in that merely looked callable would satisfy the other
;; assertions here.
;;
;; g_hidden names a function this module does not export, so its wrapper can
;; come from nowhere but computeEscapableFuncs' loop over ref.func global
;; initializers: delete that loop and `g_hidden is a function` goes red. Keep
;; this module free of an `(elem declare ...)` segment, which would put the
;; index in the set by another route and mask exactly that.
;;
;; It runs with -gc-sanitize-handles=1 because wasmMakeGlobal's funcref arm
;; brand-checks its value with isWasmExportedFunction, which ALLOCATES --
;; HiddenClass::findPropertyNoMap initializes a missing property map -- and
;; that call is a safepoint in the middle of the export loop. In a build
;; without HERMESVM_SANITIZE_HANDLES the flag is ignored and this is an
;; ordinary behavioural test.
;;
;; One case pins INTERIM behaviour: writing an exported mutable reference
;; global through `.value` is refused, because writing a reference-typed
;; global is not implemented yet. That case is to be rewritten when the setter
;; task lands, not routed around.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js -gc-sanitize-handles=1 %S/e2e-global-ref-export-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; An immutable externref import, so that an exported externref global can
  ;; carry something other than null: a global initializer is a constant
  ;; expression, and ref.null and global.get are the only two that produce an
  ;; externref.
  (import "e" "hostref" (global $imp externref))

  (func $h (export "h") (param i32) (result i32)
    (i32.add (local.get 0) (i32.const 1)))

  ;; Escapable but NOT exported under any name: the only way script reaches
  ;; this function is through the funcref global below.
  (func $hidden (result i32)
    (i32.const 5))

  (global (export "g_null") externref (ref.null extern))
  (global (export "g_host") externref (global.get $imp))
  (global (export "g_func") funcref (ref.func $h))
  (global (export "g_hidden") funcref (ref.func $hidden))
  (global (export "g_nullfunc") funcref (ref.null func))
  ;; Mutable, so this one is exported LIVE: it holds no value of its own and
  ;; reads the module's frame slot through a closure.
  (global (export "g_mut") (mut externref) (global.get $imp))
)

;; The expected output. It lives here rather than in the driver because
;; FileCheck reads this file.
;; CHECK: instantiated: true
;; CHECK-NEXT: all six are Globals with no own properties: true 0
;; CHECK-NEXT: g_null: null
;; CHECK-NEXT: g_host === hostValue: true
;; CHECK-NEXT: g_func === h: true
;; CHECK-NEXT: g_func calls: 42
;; CHECK-NEXT: g_hidden is a function: function
;; CHECK-NEXT: g_hidden calls: 5
;; CHECK-NEXT: g_hidden is not h: true
;; CHECK-NEXT: g_nullfunc: null
;; CHECK-NEXT: g_mut === hostValue: true
;; CHECK-NEXT: g_mut write: TypeError: WebAssembly.Global.prototype.value: writing a reference-typed global is not implemented yet
;; CHECK-NEXT: g_mut unchanged: true
;; CHECK-NEXT: g_func write: TypeError: WebAssembly.Global.prototype.value: cannot set an immutable global
