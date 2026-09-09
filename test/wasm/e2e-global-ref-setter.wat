;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; WebAssembly.Global.prototype.value, the SETTER, across every declared type.
;;
;; The setter was written as "an i64 global requires a BigInt; everything else
;; goes through toNumber_RJS", so an object assigned to an externref global
;; would have become NaN. That state was never reachable: reference-typed
;; globals could not be constructed until Task 3, and Task 3 added an interim
;; refusal so that this setter raised "not implemented yet" rather than
;; coercing. The dispatch below is what replaced the refusal, and the NaN is
;; what it would have done without it -- which is why the object case is first
;; and compares identity.
;;
;; The setter now dispatches on the declared type before any coercion, and
;; this file is the table: an externref takes any JS value as it stands, an anyfunc
;; takes null or an Exported Function, an i64 takes a BigInt wrapped to 64
;; bits, and a numeric global still COERCES with ToNumber -- which is what
;; separates this setter from the internal one, whose test refuses the same
;; values (e2e-global-ref-internal-setter.wat).
;;
;; The module supplies two things a pure-JS test cannot. One is a real
;; Exported Function, built by WasmIRGen and branded by the module's own
;; generated wasmSetFuncInfo call. The other is a place to observe a write
;; from: its exported MUTABLE globals are published live, so a `.value` write
;; runs the module's own setter closure, and the get_* functions read the
;; frame slot that closure writes rather than reading the Global back. A
;; closure that stored nowhere the module can see passes the second reading
;; and fails the first.
;;
;; The imported mutable externref global is the identity round trip the design
;; requires in the JS-to-Wasm direction: the host writes an object through
;; `.value` and `get_imp` hands back that same object. Its link-time value is
;; an object rather than null, because wasmLinkGlobal still spells its two
;; failures `null` and `undefined`; that collision is the link path's own gap
;; and not this file's subject.
;;
;; It runs with -gc-sanitize-handles=1 because the setter allocates in
;; several places this file reaches: ToNumber, which runs user JS; the funcref
;; brand check, which allocates in HiddenClass::findPropertyNoMap; the live
;; setter closure, which runs generated code; and the i64 store, which
;; materializes the BigInt. The loops at the end allocate between writes so
;; that a destination held raw across any of them would be a use-after-free
;; rather than a value that happened to survive. In a build without
;; HERMESVM_SANITIZE_HANDLES the flag is ignored and this is an ordinary
;; behavioural test.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js -gc-sanitize-handles=1 %S/e2e-global-ref-setter-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "ref" (global $imp (mut externref)))

  (func (export "h") (param i32) (result i32)
    (i32.add (local.get 0) (i32.const 1)))

  ;; Reads the imported global through generated code, so a host `.value`
  ;; write is observed on the Wasm side rather than through the same object.
  (func (export "get_imp") (result externref)
    (global.get $imp))

  ;; Mutable, therefore exported LIVE: each of these holds no value of its
  ;; own and a `.value` write runs a setter closure over the frame slot.
  (global $ge (export "g_ext") (mut externref) (ref.null extern))
  (global $gf (export "g_fn") (mut funcref) (ref.null func))
  (global $g64 (export "g_i64") (mut i64) (i64.const 0))
  (global $g32 (export "g_i32") (mut i32) (i32.const 0))
  (global $gf32 (export "g_f32") (mut f32) (f32.const 0))

  (func (export "get_ext") (result externref) (global.get $ge))
  (func (export "get_fn") (result funcref) (global.get $gf))
  (func (export "get_i64") (result i64) (global.get $g64))
  (func (export "get_i32") (result i32) (global.get $g32))
  (func (export "get_f32") (result f32) (global.get $gf32))
)

;; The expected output. It lives here rather than in the driver because
;; FileCheck reads this file.
;; CHECK: fixture: function true
;; CHECK-NEXT: externref an object: true
;; CHECK-NEXT: externref null: true
;; CHECK-NEXT: externref undefined: true
;; CHECK-NEXT: externref a number: true number
;; CHECK-NEXT: externref never coerces: true true
;; CHECK-NEXT: anyfunc an export: true
;; CHECK-NEXT: anyfunc a plain function: TypeError: WebAssembly.Global.prototype.value: an 'anyfunc' global requires null or a WebAssembly exported function
;; CHECK-NEXT: anyfunc undefined: TypeError: WebAssembly.Global.prototype.value: an 'anyfunc' global requires null or a WebAssembly exported function
;; CHECK-NEXT: anyfunc a number: TypeError: WebAssembly.Global.prototype.value: an 'anyfunc' global requires null or a WebAssembly exported function
;; CHECK-NEXT: anyfunc intact after the refusals: true
;; CHECK-NEXT: anyfunc null: true
;; CHECK-NEXT: i64 wraps to 64 bits: true bigint
;; CHECK-NEXT: i64 negative: true
;; CHECK-NEXT: i64 from a Number: TypeError: WebAssembly.Global.prototype.value: an i64 global requires a BigInt value
;; CHECK-NEXT: i64 intact after the refusal: true
;; CHECK-NEXT: i32 from a string: true number
;; CHECK-NEXT: i32 from a valueOf that allocates: true
;; CHECK-NEXT: f32 narrows: true true
;; CHECK-NEXT: valueOf on an externref global: true
;; CHECK-NEXT: valueOf on an anyfunc global: true
;; CHECK-NEXT: live externref: true true
;; CHECK-NEXT: live anyfunc: true true
;; CHECK-NEXT: live anyfunc refuses a plain function: TypeError: WebAssembly.Global.prototype.value: an 'anyfunc' global requires null or a WebAssembly exported function
;; CHECK-NEXT: live anyfunc intact: true
;; CHECK-NEXT: live i64: true true
;; CHECK-NEXT: live i32: true true
;; CHECK-NEXT: live f32: true true
;; CHECK-NEXT: live externref never coerces: true true true
;; CHECK-NEXT: JS write, Wasm read, same object: true
;; CHECK-NEXT: funcref writes intact across collections: true
;; CHECK-NEXT: externref writes traced across collections: true
;; CHECK-NEXT: live externref writes survive collection: true
