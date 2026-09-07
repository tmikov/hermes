;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; `hermesc --wasm` used to run wabt::ReadBinary and stop, never
;; wabt::ValidateModule (H19), so it accepted modules the Wasm engine itself
;; rejects. A `call_indirect` through an EXTERNREF table is exactly such a
;; module: nothing in wabt's structural reader or in Hermes IRGen has an
;; opinion about a table's element type, only the semantic validator does, so
;; the module used to compile clean and reach unchecked assumptions at
;; runtime (see HermesBuiltin.cpp's wasmCallIndirect for what goes wrong
;; downstream, and dz 01a0460c-8ed8). `hermesc --wasm` now validates before
;; compiling, on the same code path `WebAssembly.Module` already used, so
;; this is refused up front with the validator's own diagnostic.
;;
;; The Wasm validator rejects this module, so `wat2wasm` itself would refuse
;; to emit it; --no-check makes it emit the binary anyway.

;; REQUIRES: wasm

;; RUN: %wat2wasm --no-check %s -o %t.wasm
;; RUN: (! %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>&1) | %FileCheck %s

(module
  (table $t 1 externref)
  (type $ty (func))
  (func $f (call_indirect $t (type $ty) (i32.const 0))))

;; The message is checked, not just a non-zero exit: a refusal for the wrong
;; reason (a generic "Failed to parse Wasm binary", say) would pass a
;; check for "Error:" alone.
;; CHECK: Error:
;; CHECK-SAME: type mismatch: call_indirect must reference table of funcref
;; CHECK-SAME: type
