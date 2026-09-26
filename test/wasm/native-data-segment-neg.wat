;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; An imported-memory segment bypasses the frontend's local-memory bounds
;; checks, so its original signed offset reaches _sh_wasm_data_segment_init
;; unchecked (WasmIRGen.cpp:2120, 2252). That is why Task 3 converts the
;; offset with _sh_to_uint32_double rather than a plain C cast: a cast would
;; silently wrap a negative offset into a huge unsigned one, and the helper
;; must instead reject it with a JS error.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=dnegmod -c -o %t-mod.o %t.wasm
;; RUN: %shermes -o %t.exe -Wc,%t-mod.o %S/native-data-segment-neg-driver.js_
;; RUN: %t.exe | %FileCheck --match-full-lines %s

(module
  (import "env" "mem" (memory 1))
  (data (i32.const -1) "x"))

;; A negative offset must produce a JS error, not undefined behaviour.
;; CHECK: threw: true
;; CHECK-NEXT: done
