;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; `hermes --wasm` calls the instantiate() factory with an *empty* import
;; object; it does not synthesise imports. A module that declares imports
;; therefore fails linking, which is the correct diagnosis of running it with
;; nothing to link against.
;;
;; With `{}` as the import object the whole namespace is absent, so the
;; LinkError names the namespace, `env`. (Naming the field as well, `env.log`,
;; requires the namespace to exist but lack the field, which no empty object
;; can arrange; e2e-import-linkerror.wat covers that form.)

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: (! %hermes --wasm %t.wasm 2>&1) | %FileCheck --match-full-lines %s

;; RUN: %hermesc --wasm -emit-binary -out %t.hbc %t.wasm
;; RUN: (! %hermes --wasm %t.hbc 2>&1) | %FileCheck --match-full-lines %s

(module
  (import "env" "log" (func $log (param i32)))
  (func $start
    (call $log (i32.const 1)))
  (start $start))

;; CHECK: Uncaught LinkError: module has no import namespace env
