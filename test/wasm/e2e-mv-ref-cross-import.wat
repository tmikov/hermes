;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The route `B import trampoline -> A export wrapper -> A function`, with the
;; interception that route used to permit.
;;
;; The importer reaches the exporter's multi-value functions as ordinary JS
;; imports, so their results arrive as the exporter's PUBLIC result array,
;; which the importer's trampoline reads element by element. That array used to
;; be built by calling globalThis.Array after the internal call and storing the
;; results into whatever came back, so a replacement constructor decided what
;; the importer read. An externref substituted that way arrives on the Wasm
;; stack with nothing to refuse it -- any JS value is a valid externref -- and a
;; substituted funcref only has to be some other genuine Exported Function to
;; pass the importer's funcref admission.
;;
;; The driver installs exactly such a constructor. The exporter's wrapper now
;; builds its array with wasmMakeResultArray instead, so the constructor is
;; never reached and the importer reads what the exporter produced.

;; REQUIRES: wasm
;; RUN: %wat2wasm %S/e2e-mv-ref-cross-import-exporter.wat_ -o %t-exp.wasm && %hermesc --wasm -emit-binary -out %t-exp.hbc %t-exp.wasm && %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-mv-ref-cross-import-driver.js_ -- %t-exp.hbc %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "a" "mkpair" (func $mkpair (result i32 funcref)))
  (import "a" "mkext" (func $mkext (result externref i32)))

  (func (export "viaImportFunc") (result funcref)
    (local $r funcref)
    (call $mkpair)
    (local.set $r)
    (drop)
    (local.get $r))

  (func (export "viaImportNum") (result i32)
    (call $mkpair)
    (drop))

  (func (export "viaImportExt") (result externref)
    (local $r externref)
    (call $mkext)
    (drop)
    (local.set $r)
    (local.get $r)))

;; CHECK: replacement Array constructor calls: 0
;; CHECK-NEXT: viaImportNum: 42
;; CHECK-NEXT: viaImportFunc: function calls -> 7 same as exporter's svGet=true
;; CHECK-NEXT: viaImportExt: is the host object=true
;; CHECK-NEXT: substituted funcref did not come through: true
;; CHECK-NEXT: substituted externref did not come through: true
;; CHECK-NEXT: done
