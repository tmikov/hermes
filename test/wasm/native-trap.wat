;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A trap raised inside a native Wasm function surfaces to JS as a catchable
;; exception, not as a process crash. test/wasm/cli-run-trap.wat is not a
;; usable template for this: it has no JS driver and checks a FAILING
;; process and its stderr. This test checks the stdout of a SUCCEEDING run,
;; with the trap caught by a JS try/catch.
;;
;; What this does NOT cover: `boom` is called directly from the driver's top
;; level, through WebAssembly.Instance -> __wasm_instantiate__'s exports ->
;; an ordinary exported-function call. It does not exercise
;; __wasm_instantiate__ itself trapping, which is a different call path
;; (Callable::executeCall1, already guarded by its own SHJmpBuf at
;; Callable.cpp:60-62) from the one an exported function's trap takes.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=trapmod -c -o %t-mod.o %t.wasm
;; RUN: %shermes -o %t.exe -Wc,%t-mod.o %S/native-trap-driver.js_
;; RUN: %t.exe | %FileCheck --match-full-lines %s

(module
  (func (export "boom") (unreachable)))

;; CHECK: caught: true
;; CHECK-NEXT: done
