;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; runNativeWasmUnit() (lib/VM/JSLib/WebAssembly/WebAssembly.cpp) guards
;; _sh_unit_init against a throw from a native Wasm unit's unit_main: the
;; bare call has no jmpbuf installed, so a throw would either abort() or
;; longjmp past this file's live LocalsRAII/GCScope objects. But no
;; well-formed Wasm module's top level throws -- it only builds a module
;; info object out of a handful of allocations -- so nothing else in this
;; suite reaches that path. native-trap.wat's trap is raised inside an
;; *exported function* call, which goes through __wasm_instantiate__'s own
;; SHJmpBuf (Callable::executeCall1, Callable.cpp) instead.
;;
;; This test manufactures a unit whose top level does throw: an ordinary JS
;; unit (native-unit-throws-throwing.js_), registered under a Wasm unit name
;; by a small C shim (native-unit-throws-shim.c) since a plain JS unit does
;; not self-register the way a real Wasm unit compiled with -wasm does. The
;; module below is the unrelated, well-formed module used to prove the
;; runtime is still usable afterwards -- see native-unit-throws-driver.js_.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=goodmod -emit-c -o %t-good.c %t.wasm
;; RUN: rm -rf %t.d && mkdir -p %t.d
;; RUN: sed -n 's/.*sh_export_goodmod, \("[^"]*"\).*/#define GOODMOD_CODEGEN_CONFIG \1/p' \
;; RUN:     %t-good.c > %t.d/goodmod-config.h
;; RUN: %shermes -exported-unit=goodmod -c -o %t-good.o %t.wasm
;; RUN: %shermes -exported-unit=throwmod -c -o %t-throw.o \
;; RUN:     %S/native-unit-throws-throwing.js_
;; RUN: %shermes -o %t.exe \
;; RUN:     -Wc,-I%t.d,%t-good.o,%t-throw.o,%S/native-unit-throws-shim.c \
;; RUN:     %S/native-unit-throws-driver.js_
;; RUN: %t.exe | %FileCheck --match-full-lines %s

(module
  (func (export "nop")))

;; CHECK: caught: true
;; CHECK-NEXT: is Error: true
;; CHECK-NEXT: message: boom
;; CHECK-NEXT: still usable: true
;; CHECK-NEXT: done
