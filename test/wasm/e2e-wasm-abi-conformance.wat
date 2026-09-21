;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The WebAssembly linking ABI was PUBLISHED to script as ordinary own
;; properties. A Memory carried __wasm_type__ / __wasm_min__ / __wasm_max__, a
;; Global carried __wasm_type__, and a Table carried those three plus its three
;; backing arrays -- all writable, all enumerable, all forgeable.
;;
;; This file is the whole-ABI statement, across all three kinds at once:
;;
;;   * CONFORMANCE. The spec gives Memory, Global and Table no own properties
;;     at all. Object.keys, getOwnPropertyNames, getOwnPropertySymbols and
;;     JSON.stringify must every one of them come back empty. This is the gap
;;     no point fix addresses: each publication was individually defensible and
;;     the sum of them was a non-conforming object model.
;;   * FORGERY. Since the ABI was a set of ordinary properties, an object
;;     literal carrying them satisfied an import -- for a global it linked
;;     outright and handed the module the literal's `value`. So did an object
;;     that merely INHERITED from a genuine one, which is why the check is a
;;     brand check (dyn_vmcast) and not `instanceof`: `instanceof` says yes to
;;     Object.create(realMemory), and the rows below pin that it does.
;;   * H7. The published limits were a snapshot taken in the constructor and
;;     never updated by grow, so a memory grown from one page to two still
;;     advertised a minimum of one and failed to satisfy a (memory 2) import
;;     it plainly satisfied. Limits now come from the internal fields at use
;;     time.
;;   * H2. The metadata was written with putNamed_RJS, which walks the
;;     prototype chain -- so a setter on WebAssembly.Memory.prototype ran
;;     arbitrary user JS inside the native constructor, on a half-built
;;     object. Deleting the publication closes that window; there is nothing
;;     left for the constructor to write.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %wat2wasm %S/e2e-wasm-abi-conformance-consumer.wat_ -o %t-cons.wasm && %hermesc --wasm -emit-binary -out %t-cons.hbc %t-cons.wasm && %wat2wasm %S/e2e-wasm-abi-conformance-grown.wat_ -o %t-grown.wasm && %hermesc --wasm -emit-binary -out %t-grown.hbc %t-grown.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-wasm-abi-conformance-driver.js_ -- %t.hbc %t-cons.hbc %t-grown.hbc | %FileCheck --match-full-lines %s

(module
  (memory (export "mem") 1 4)
  (global (export "g") i32 (i32.const 42))
  (table (export "tbl") 2 funcref)
  (elem (i32.const 0) $f)
  (func $f (result i32) (i32.const 7)))

;; -- Conformance: no own properties, on any kind, from either route --
;; CHECK: exported memory getOwnPropertyNames: []
;; CHECK-NEXT: exported memory keys: []
;; CHECK-NEXT: exported memory symbols: []
;; CHECK-NEXT: exported memory JSON: {}
;; CHECK-NEXT: exported global getOwnPropertyNames: []
;; CHECK-NEXT: exported global keys: []
;; CHECK-NEXT: exported global symbols: []
;; CHECK-NEXT: exported global JSON: {}
;; CHECK-NEXT: exported table getOwnPropertyNames: []
;; CHECK-NEXT: exported table keys: []
;; CHECK-NEXT: exported table symbols: []
;; CHECK-NEXT: exported table JSON: {}
;; CHECK-NEXT: JS-API memory getOwnPropertyNames: []
;; CHECK-NEXT: JS-API memory keys: []
;; CHECK-NEXT: JS-API memory symbols: []
;; CHECK-NEXT: JS-API memory JSON: {}
;; CHECK-NEXT: JS-API global getOwnPropertyNames: []
;; CHECK-NEXT: JS-API global keys: []
;; CHECK-NEXT: JS-API global symbols: []
;; CHECK-NEXT: JS-API global JSON: {}
;; CHECK-NEXT: JS-API table getOwnPropertyNames: []
;; CHECK-NEXT: JS-API table keys: []
;; CHECK-NEXT: JS-API table symbols: []
;; CHECK-NEXT: JS-API table JSON: {}

;; Genuine objects link and the module runs, from both routes. Without these
;; two lines every LinkError below could be a check that refuses everything.
;; CHECK-NEXT: genuine module objects: linked, probe = 42
;; CHECK-NEXT: genuine JS-API objects: linked, probe = 42

;; -- Forgery: the shape that used to be the ABI --
;; CHECK-NEXT: forged memory literal: LinkError
;; CHECK-NEXT: forged global literal: LinkError
;; CHECK-NEXT: forged table literal: LinkError

;; instanceof is true for all three inheriting forgeries, which is exactly why
;; the check is a brand check.
;; CHECK-NEXT: Object.create(memory) instanceof WebAssembly.Memory: true
;; CHECK-NEXT: Object.create(memory) as import: LinkError
;; CHECK-NEXT: Object.create(global) instanceof WebAssembly.Global: true
;; CHECK-NEXT: Object.create(global) as import: LinkError
;; CHECK-NEXT: Object.create(table) as import: LinkError
;; CHECK-NEXT: Proxy(memory) as import: LinkError
;; CHECK-NEXT: Proxy(global) as import: LinkError
;; CHECK-NEXT: Proxy(table) as import: LinkError

;; -- H7: the size is read at use time --
;; CHECK-NEXT: grown memory links: memory.size = 2
;; CHECK-NEXT: ungrown memory links: LinkError: import e.m does not satisfy the declared memory limits

;; -- H2: no prototype setter runs inside a native constructor --
;; CHECK-NEXT: prototype setters that ran inside a constructor: []
;; CHECK-NEXT: done
