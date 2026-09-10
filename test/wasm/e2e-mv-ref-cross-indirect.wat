;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A multi-value reference result crossing a MODULE boundary through
;; call_indirect.
;;
;; call_indirect passes the caller's numeric return buffers to whatever
;; function the table slot holds, and it used to pass nothing else. The
;; reference half travelled in a per-module JS Array instead, so the callee
;; stored into the array of the module it was DEFINED in and this module read
;; the array of the module doing the calling -- two different objects. The slot
;; this module read had never been written, so `callIt` came back undefined and
;; `callExt` came back with something other than the object the exporter holds,
;; while the i32 halves, which do travel in the passed-in buffers, were
;; correct. That split is what the checks below pin.
;;
;; The container is now allocated by the caller and passed along with the two
;; numeric views, so the callee writes the object the caller reads.

;; REQUIRES: wasm
;; RUN: %wat2wasm %S/e2e-mv-ref-cross-indirect-exporter.wat_ -o %t-exp.wasm && %hermesc --wasm -emit-binary -out %t-exp.hbc %t-exp.wasm && %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-mv-ref-cross-indirect-driver.js_ -- %t-exp.hbc %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "env" "tbl" (table 2 funcref))

  ;; Declared here, defined in the exporter. call_indirect matches by interned
  ;; type id, so both modules describe the same signature even though neither
  ;; knows the other's type numbering.
  (type $mv (func (result i32 funcref)))
  (type $mve (func (result externref i32)))

  (func (export "callIt") (result funcref)
    (local $r funcref)
    (call_indirect (type $mv) (i32.const 0))
    (local.set $r)
    (drop)
    (local.get $r))

  (func (export "callItNum") (result i32)
    (call_indirect (type $mv) (i32.const 0))
    (drop))

  (func (export "callExt") (result externref)
    (local $r externref)
    (call_indirect (type $mve) (i32.const 1))
    (drop)
    (local.set $r)
    (local.get $r))
)

;; The i32 halves travel in the numeric buffers the caller already passed, so
;; they were right before and are right now. They are here to separate "the
;; call happened and the buffer arrived" from "the reference arrived".
;; CHECK: callItNum: 42
;; CHECK-NEXT: callExt: is the host object=true
;; CHECK-NEXT: callIt: function calls -> 7 same as exporter's svGet=true
;; CHECK-NEXT: done
