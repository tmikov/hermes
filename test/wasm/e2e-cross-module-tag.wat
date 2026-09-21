;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; throw and catch compared module-local tag indices. Another module numbers
;; its tags differently, so an exception thrown with one tag was caught by the
;; handler for a different one -- which then decoded the payload according to
;; the wrong signature. Tag imports were also validated and then discarded,
;; with no store, so there was nothing to compare against in any case.
;;
;; Tag identity in Wasm is NOMINAL: two tags with the same signature are
;; distinct and must not catch each other, so a structural type string cannot
;; serve as the identity either. Each tag now has an object whose identity is
;; the tag's identity, shared with importers through the export.

;; REQUIRES: wasm
;; RUN: %wat2wasm --enable-exceptions %S/e2e-cross-module-tag-exporter.wat_ -o %t-exp.wasm && %hermesc --wasm -emit-binary -out %t-exp.hbc %t-exp.wasm && %wat2wasm --enable-exceptions %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-cross-module-tag-driver.js_ -- %t-exp.hbc %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; b is imported FIRST, so a is tag index 1 here but index 0 in the exporter.
  ;; With index-based matching the exporter's tag a (0) matched this module's
  ;; tag b (0), running the wrong handler.
  (import "exporter" "b" (tag $b (param i32)))
  (import "exporter" "a" (tag $a (param i32)))
  (import "exporter" "boom_a" (func $boom_a (param i32)))
  (global $which (mut i32) (i32.const -1))
  (func (export "catches_a") (param i32) (result i32)
    (try
      (do (call $boom_a (local.get 0)))
      (catch $a (drop) (global.set $which (i32.const 1)))
      (catch $b (drop) (global.set $which (i32.const 2)))
      (catch_all (global.set $which (i32.const 0))))
    (global.get $which))

  ;; Two locally-declared tags with the same signature must stay distinct.
  (tag $x (param i32))
  (tag $y (param i32))
  (global $w2 (mut i32) (i32.const -1))
  (func (export "nominal") (result i32)
    (try
      (do (throw $x (i32.const 7)))
      (catch $y (drop) (global.set $w2 (i32.const 2)))
      (catch_all (global.set $w2 (i32.const 0))))
    (global.get $w2)))

;; The exception thrown with tag a is caught by the handler for tag a, not by
;; the one whose local index happens to collide.
;; CHECK: catches_a: 1

;; And identical signatures do not make two tags interchangeable: throwing x
;; with only a handler for y present must fall through to catch_all.
;; CHECK-NEXT: nominal: 0
;; CHECK-NEXT: done
