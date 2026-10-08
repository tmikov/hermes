;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A multi-value function reached by another module's `call_indirect` read its
;; nested call's results out of the wrong buffer, and returned zeros.
;;
;; Multi-value results travel in a scratch buffer with two typed-array views,
;; one per module instance. A function meets them in two roles: as PARAMETERS,
;; the views its caller supplied, where its own results go; and loaded from
;; module scope, which is what it hands to the functions it calls.
;;
;; onCall and onCallIndirect passed the module-scope pair, and emitRetBufLoads
;; read the results back out of the PARAMETER pair. Inside one module those
;; are the same object, so the mistake is invisible: every entry point supplies
;; that module's own views.
;;
;; `call_indirect` is what makes them differ. A table's call array holds the
;; callee's INTERNAL CLOSURE, not its export wrapper -- setWasmTableSlot stores
;; the closure in funcsArr and the wrapper separately in exportedArr, and an
;; element segment installs the closure the same way -- so the call goes
;; straight into the other module's function with the CALLING module's views.
;; That function then hands its OWN module's views to its nested call, and
;; reads the caller's back. Nobody wrote those.
;;
;; Measured before the fix: `slot 0, nested call: 0,0,0`.
;;
;; Note what does NOT show it, since two obvious checks do not:
;;
;;   - `flat`, whose results go straight into the views it was given, is right
;;     either way. Only a NESTED call reads a buffer back.
;;   - calling the same function from JS is right either way: that goes
;;     through its own export wrapper, which supplies its own module's views.
;;
;; Both are kept below as controls, because a fix that pointed everything at
;; one pair of views would break them.

;; REQUIRES: wasm
;; RUN: %wat2wasm %S/e2e-cross-module-retbuf-exporter.wat_ -o %t-e.wasm && %hermesc --wasm -emit-binary -out %t-e.hbc %t-e.wasm && %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-cross-module-retbuf-driver.js_ -- %t-e.hbc %t.hbc | %FileCheck --match-full-lines %s

(module
  (type $t (func (result i32 f64 i64)))

  ;; The exporter's own table, with its $nested already in it. Reaching the
  ;; defect this way involves no JS whatsoever -- two modules, an exported
  ;; table and an imported one.
  (import "a" "tbl" (table $imported 1 funcref))

  ;; And a table of this module's own, for the driver to place functions into
  ;; through the JS API.
  (table $own (export "tbl") 2 funcref)

  ;; Three result kinds, so both views cross the module boundary: i32 and i64
  ;; through the integer one, f64 through the float one.
  (func (export "callSlot") (param i32) (result i32 f64 i64)
    (call_indirect $own (type $t) (local.get 0)))

  (func (export "callImported") (result i32 f64 i64)
    (call_indirect $imported (type $t) (i32.const 0))))

;; The defect, reached two ways. The first needs no JS in the path: the
;; exporter's own table, imported. The second is the same callee placed
;; through the JS API. Both returned 0,0,0.
;; CHECK: imported table, no JS in the path: 11,2.5,7
;; CHECK-NEXT: slot 0, nested call: 11,2.5,7

;; The two controls, both correct before the fix as well.
;; CHECK-NEXT: slot 1, no nested call: 33,4.5,9
;; CHECK-NEXT: nested() called from JS: 11,2.5,7
;; CHECK-NEXT: done
