;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Verify that an opcode this IR generator does not implement emits a warning
;; rather than being silently ignored. This is D.13's requirement that no
;; opcode falls through to BinaryReaderNop's no-op without saying so.
;;
;; The warning now comes from the "unknown member of a known opcode family"
;; defaults in BinaryReaderHermesIRGen -- binary, compare, convert, unary.
;; i32x4.add reaches the binary one, and when this IR generator grows a case
;; for it, this line is where the change surfaces.
;;
;; The two reference opcodes that used to warn here do not any more, and the
;; second half of this test is about that:
;;
;;   ref.null pushes JS null, which needs nothing synthesized.
;;   ref.func pushes the function's canonical Exported Function.
;;
;; ref.func is exercised in a function that nothing calls, which is the shape
;; that worked before it was implemented (the placeholder was pushed and
;; dropped) and has to keep working. "It does not warn" on its own would also
;; be satisfied by dropping the opcode on the floor, so the load of
;; exported_func_0 -- the wrapper variable of $ref_test, which is function 0
;; -- is checked as well.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm 2>&1 | %FileCheck %s
;; RUN: %hermesc --wasm --dump-ir -O0 %t.wasm 2>&1 | %FileCheck --check-prefix=NOREF %s

(module
  (table 1 funcref)
  (elem declare func $ref_test)

  (func $ref_test
    ;; Supported: a null reference and a reference to this function.
    ref.null func
    drop
    ref.func $ref_test
    drop
    ;; Not supported: a SIMD binary operator.
    v128.const i32x4 1 2 3 4
    v128.const i32x4 5 6 7 8
    i32x4.add
    drop))

;; Warnings go to stderr and the IR dump to stdout, so the two are merged in
;; an order this test must not depend on.
;; CHECK-DAG: warning: unsupported Wasm opcode: binary(unknown)
;; CHECK-DAG: LoadFrameInst (:any) {{.*}}[%VS0.exported_func_0]

;; No reference opcode warns. A NOT-only FileCheck run so that this covers
;; the whole output rather than the part before some other match.
;; NOREF-NOT: unsupported Wasm opcode: ref.
