;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; `hermes --wasm <input>` declares that the input is a WebAssembly module and
;; instantiates it: it runs the bytecode, whose top level only builds a module
;; object {instantiate, exportDescs, importDescs}, and then calls that object's
;; instantiate property with an empty import object. Calling it is what creates
;; the memory and globals and runs the start function. This must work
;; identically for a .wasm binary, which is compiled first, and for a .hbc that
;; `hermesc --wasm` produced earlier.
;;
;; The .hbc form must work with no -Xenable-* flag: a file named on the command
;; line is embedder-level trusted input, exactly like plain `hermes foo.hbc`,
;; and does not go through the JS-facing WebAssembly API whose bytecode
;; content-sniffing is gated.
;;
;; The start function below is self-checking: it stores 6*7 into linear memory,
;; reads it back through a mutable global and executes `unreachable` unless the
;; round trip yields 42. Exiting 0 therefore means the start function really
;; ran and really computed; a driver that instantiated nothing could not tell
;; the difference between this module and a broken one. Nothing may be printed
;; either -- see the CHECK-NOT below.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermes --wasm %t.wasm > %t.wasm.out 2>&1 && echo "instantiated" >> %t.wasm.out
;; RUN: %FileCheck --match-full-lines %s < %t.wasm.out

;; RUN: %hermesc --wasm -emit-binary -out %t.hbc %t.wasm
;; RUN: %hermes --wasm %t.hbc > %t.hbc.out 2>&1 && echo "instantiated" >> %t.hbc.out
;; RUN: %FileCheck --match-full-lines %s < %t.hbc.out

(module
  (memory 1)
  (global $g (mut i32) (i32.const 0))
  (func $start
    (i32.store (i32.const 8) (i32.mul (i32.const 6) (i32.const 7)))
    (global.set $g (i32.load (i32.const 8)))
    (if (i32.ne (global.get $g) (i32.const 42))
      (then unreachable)))
  (start $start))

;; The module itself must print nothing: the only line is the one the RUN line
;; appends after hermes exits 0.
;; CHECK-NOT: {{.}}
;; CHECK: instantiated
