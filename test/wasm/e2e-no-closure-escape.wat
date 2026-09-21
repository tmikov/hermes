;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; THE ESCAPE-ROUTE ENUMERATION (finding J4).
;;
;; A Wasm function's INTERNAL closure has statically typed parameters and an
;; internal calling convention: an i64 is a signed lo/hi pair, multi-value and
;; i64 results travel through a return buffer, and an f32/f64 parameter is
;; declared `:number`, which the float backend trusts -- FBinaryMathInst reads
;; the raw double bits. All of that is sound if and only if every caller is
;; Wasm. Hand that closure to JavaScript and it is not: f('x') reaches
;; getDouble() on a string (a Debug assert, a Release segfault) and f(5n)
;; reads a BigInt as a double.
;;
;; The canonical Exported Function -- the wrapper -- is the object that is
;; supposed to cross the boundary instead. It coerces every argument, splits
;; i64s, and unpacks the return buffer.
;;
;; This test enumerates EVERY route by which a function value can reach script
;; and asserts that each one yields a wrapper. It is the gate on the
;; `:number` parameter annotation: that annotation is honest exactly as long
;; as every line below says `wrapper`. If a new route is added -- ref.func in
;; a function body, call_ref, a funcref global export, anything that puts a
;; value on the JS side of the boundary -- it belongs here, and it must be
;; added BEFORE the feature lands, not after.
;;
;; The oracle is WebAssembly.Table.prototype.set, which accepts null or a
;; genuine Exported Function and nothing else. Its brand is an internal
;; property that script can neither name nor write, so it cannot be forged by
;; script and it cannot be approximated: this is the strongest question about
;; a function value that script is able to ask. The first line of output shows
;; the oracle refusing a plain JS function, so a broken oracle cannot make the
;; rest pass.

;; REQUIRES: wasm
;; RUN: %wat2wasm --enable-exceptions %S/e2e-no-closure-escape-consumer.wat_ -o %t-c.wasm && %hermesc --wasm -emit-binary -out %t-c.hbc %t-c.wasm && %wat2wasm --enable-exceptions %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-no-closure-escape-driver.js_ -- %t.hbc %t-c.hbc | %FileCheck --match-full-lines %s

(module
  ;; Receives a funcref as an ARGUMENT: the import-trampoline route.
  (import "env" "sink" (func $sink (param funcref)))
  ;; An imported JS function placed in a table. Its canonical Exported
  ;; Function wraps the import TRAMPOLINE, so a JS function that went into a
  ;; table comes back out the same way a native one does.
  (import "env" "jsF" (func $jsF (param f64) (result f64)))

  (table (export "tbl") 10 funcref)
  (tag $t (param i32 funcref))

  ;; The J4 shape: float parameters, exported AND in a table.
  (func $addf64 (export "addf64") (param f64 f64) (result f64)
    (f64.add (local.get 0) (local.get 1)))

  ;; Escapable but NOT exported under any name, so the only way script can
  ;; ever see it is one of the routes below. Its wrapper exists only because
  ;; computeEscapableFuncs() put it in the wrapper set.
  (func $addf32 (param f32 f32) (result f32)
    (f32.add (local.get 0) (local.get 1)))

  ;; i64: the internal convention is a pair of signed 32-bit halves, so the
  ;; raw closure called with the spec-legal 5n aborted the VM. Not a float
  ;; parameter, and included for exactly that reason -- the annotation is only
  ;; half of what the wrapper is for.
  (func $inc64 (param i64) (result i64)
    (i64.add (local.get 0) (i64.const 1)))

  (elem (i32.const 0) $addf64 $addf32 $inc64 $jsF)
  ;; Passive segment 1, for table.init.
  (elem func $addf32 $inc64)

  ;; A funcref global initialized with ref.func. Exporting one is not
  ;; supported (see the driver's note), so global.get is how it is observed.
  (global $g funcref (ref.func $addf32))

  ;; --- the routes, as Wasm sees them ---

  (func (export "getAt") (param i32) (result funcref)
    (table.get 0 (local.get 0)))

  ;; A funcref that travels through the return buffer's reference slots.
  (func (export "mvAt") (param i32) (result i32 funcref)
    (i32.const 1)
    (table.get 0 (local.get 0)))

  (func (export "fromGlobal") (result funcref)
    (global.get $g))

  (func (export "sendToJs") (param i32)
    (call $sink (table.get 0 (local.get 0))))

  (func (export "throwAt") (param i32)
    (throw $t (i32.const 5) (table.get 0 (local.get 0))))

  ;; --- the writers, each read back through Table.prototype.get ---

  (func (export "setAt") (param i32 i32)
    (table.set 0 (local.get 0) (table.get 0 (local.get 1))))

  (func (export "setFromGlobal") (param i32)
    (table.set 0 (local.get 0) (global.get $g)))

  (func (export "copy") (param i32 i32 i32)
    (table.copy 0 0 (local.get 0) (local.get 1) (local.get 2)))

  (func (export "initSeg1") (param i32 i32 i32)
    (table.init 1 (local.get 0) (local.get 1) (local.get 2)))

  (func (export "fillFrom") (param i32 i32 i32)
    (table.fill 0 (local.get 0) (table.get 0 (local.get 1)) (local.get 2)))

  (func (export "growFrom") (param i32 i32) (result i32)
    (table.grow 0 (table.get 0 (local.get 0)) (local.get 1)))

  (func (export "size") (result i32)
    (table.size 0)))

;; The oracle can say no. Without this line every `wrapper` below could be a
;; Table.prototype.set that accepts anything.
;; CHECK: oracle refuses a plain JS function: not an Exported Function (TypeError)
;; CHECK-NEXT: oracle refuses a non-function: not a function (object)

;; Every route. `wrapper` is the brand check; `same` is identity against the
;; one canonical Exported Function of that function index.
;; CHECK-NEXT: === routes to a Wasm function value ===
;; CHECK-NEXT: 01 exports.addf64: wrapper
;; CHECK-NEXT: 02 tbl.get(0) elem segment, exported func: wrapper same=true
;; CHECK-NEXT: 03 tbl.get(1) elem segment, unexported func: wrapper
;; CHECK-NEXT: 04 tbl.get(2) elem segment, i64 signature: wrapper
;; CHECK-NEXT: 05 tbl.get(3) elem segment, import trampoline: wrapper
;; CHECK-NEXT: 06 wasm table.get -> funcref result: wrapper same=true
;; CHECK-NEXT: 07 wasm table.get -> multi-value ref slot: wrapper same=true
;; CHECK-NEXT: 08 funcref global -> global.get: wrapper same=true
;; CHECK-NEXT: 09 import trampoline argument: wrapper same=true
;; CHECK-NEXT: 10 exception payload: wrapper same=true
;; CHECK-NEXT: 11 wasm table.set, read back: wrapper same=true
;; CHECK-NEXT: 12 wasm table.set of global.get, read back: wrapper same=true
;; CHECK-NEXT: 13 wasm table.copy, read back: wrapper same=true
;; CHECK-NEXT: 14 wasm table.init, read back: wrapper same=true
;; CHECK-NEXT: 15 wasm table.fill, read back: wrapper same=true
;; CHECK-NEXT: 16 wasm table.grow fill value, read back: wrapper same=true
;; CHECK-NEXT: 17 JS Table.prototype.set, read back: wrapper same=true
;; Not a route: Table.prototype.grow ignores the spec's optional fill value
;; and always writes null. Pinned so that implementing it has to come back
;; here and turn this into a real route.
;; CHECK-NEXT: 18 JS Table.prototype.grow ignores its fill value, slot is: null
;; CHECK-NEXT: 19 cross-module: importer of the table, wasm table.get: wrapper same=true
;; CHECK-NEXT: 20 cross-module: importer's own Table.prototype.get: wrapper same=true

;; Nothing about the linking ABI is a property any more, so there is no array
;; of closures to read even if one existed.
;; CHECK-NEXT: === no published ABI ===
;; CHECK-NEXT: tbl.__wasm_funcs__: undefined
;; CHECK-NEXT: tbl.__wasm_types__: undefined
;; CHECK-NEXT: tbl.__wasm_exported__: undefined
;; CHECK-NEXT: own property names of tbl: (none)
;; CHECK-NEXT: own property symbols of tbl: (none)

;; The J4 crash repro, run against the value each route hands out. A wrapper
;; coerces, so a non-number becomes NaN by ordinary JS rules; the raw closure
;; would read the argument's bits. Every route is re-tested here rather than
;; only the first: "it is a wrapper" and "it behaves like one" are different
;; claims, and the second is the one J4 is about.
;; CHECK-NEXT: === J4 repro: a float parameter given a non-number ===
;; CHECK-NEXT: addf64 via exports("x", "y"): NaN
;; CHECK-NEXT: addf64 via tbl.get("x", "y"): NaN
;; CHECK-NEXT: addf64 via table.copy slot("x", "y"): NaN
;; CHECK-NEXT: addf32 via tbl.get({}, 1): NaN
;; CHECK-NEXT: addf32 via global.get({}, 1): NaN
;; CHECK-NEXT: addf32 via exception payload({}, 1): NaN
;; CHECK-NEXT: addf32 via import argument({}, 1): NaN
;; CHECK-NEXT: addf32 via cross-module get({}, 1): NaN

;; The same values still compute correctly when given numbers, so the lines
;; above are not NaN because everything is broken.
;; CHECK-NEXT: === and the same values still work ===
;; CHECK-NEXT: addf64 via exports(2.5, 4): 6.5
;; CHECK-NEXT: addf64 via tbl.get(2.5, 4): 6.5
;; CHECK-NEXT: addf32 via tbl.get(1.5, 2.25): 3.75
;; CHECK-NEXT: addf32 via global.get(1.5, 2.25): 3.75
;; CHECK-NEXT: inc64 via tbl.get(5n): 6
;; CHECK-NEXT: jsF via tbl.get(2.5): 5
;; CHECK-NEXT: done
