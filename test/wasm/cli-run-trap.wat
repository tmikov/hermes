;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A trap in the start function must fail `hermes --wasm` exactly as an
;; uncaught JS exception does: the trap is reported on stderr and the process
;; exits non-zero. `!` fails the test if hermes exits 0, which is what a driver
;; that never called the instantiate() factory would do -- the compiled top
;; level only builds the factory, so running the bytecode alone allocates no
;; memory, runs no start function and can never trap.
;;
;; The start function only traps after computing 6*7 and finding it equal to
;; 42, so the trap also witnesses that the module body executed rather than
;; something failing before it.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: (! %hermes --wasm %t.wasm 2>&1) | %FileCheck --match-full-lines %s

;; RUN: %hermesc --wasm -emit-binary -out %t.hbc %t.wasm
;; RUN: (! %hermes --wasm %t.hbc 2>&1) | %FileCheck --match-full-lines %s

(module
  (func $start
    (if (i32.eq (i32.mul (i32.const 6) (i32.const 7)) (i32.const 42))
      (then unreachable)))
  (start $start))

;; CHECK: Uncaught Error: unreachable executed
;; CHECK-NEXT: {{.*}}at wasmTrap {{.*}}
