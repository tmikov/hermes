;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A table import is validated only by comparing the __wasm_type__ string and
;; the declared minimum against .length, so script can supply a plain object
;; whose __wasm_funcs__/__wasm_types__ are not arrays at all. Those values used
;; to reach vmcast<JSArray> in the wasm table builtins, which only asserts:
;; a Debug build aborted in Casting.h and a Release build segfaulted, both
;; reachable from ordinary JavaScript. They must now raise a catchable
;; TypeError instead.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-table-import-hostile-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (type $v_i32 (func (result i32)))
  (import "e" "t" (table 2 10 funcref))
  (func (export "ci") (param i32) (result i32)
    (call_indirect (type $v_i32) (local.get 0)))
  (func (export "sz") (result i32) (table.size 0)))

;; Every non-array shape is refused at instantiation with a LinkError, before
;; any cast. Validating here rather than on each call is what allows
;; call_indirect to cast the arrays unchecked on the hot path.
;; CHECK: funcs=string: instantiation LinkError
;; CHECK-NEXT: funcs=number: instantiation LinkError
;; CHECK-NEXT: funcs=object: instantiation LinkError

;; __wasm_types__ has no length-based check at all, so before this fix a
;; genuine funcs array let execution reach it and dereference a bogus pointer.
;; This is the case that segfaulted in a Release build.
;; CHECK-NEXT: types=number: instantiation LinkError
;; CHECK-NEXT: types=string: instantiation LinkError

;; A fully well-formed hostile table is not a memory-safety problem: it is
;; simply a table whose entries do not match the expected type.
;; CHECK-NEXT: well-formed: Error

;; A well-formed table still instantiates and works, so validation is not
;; simply rejecting everything.
;; CHECK-NEXT: instantiation: ok
;; CHECK-NEXT: done
