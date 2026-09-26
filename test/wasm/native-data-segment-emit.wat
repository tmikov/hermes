;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A data segment's bytes must reach native code through the SHUnit, not
;; through the caller's RuntimeModule.
;;
;; wasmDataSegmentInit walks the stack to the caller's CodeBlock to find the
;; blob. An SH frame is a NativeJSFunction and has no CodeBlock, so that
;; builtin raises "Cannot be called from native code" and every module with a
;; nonempty data segment fails. The backend therefore emits the blob into the
;; unit and calls a unit-aware helper instead.
;;
;; This checks the emitted C. native-data-segment.wat checks the behaviour.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm %s -o %t.wasm
;; RUN: %shermes -exported-unit=dseg -emit-c -o - %t.wasm | %FileCheck %s

(module
  (memory 1)
  (data (i32.const 0) "hello")
  (func (export "first") (result i32)
    (i32.load8_u (i32.const 0))))

;; The segment copy goes through the unit-aware helper. Function bodies are
;; emitted before the unit buffers, so this comes first.
;; CHECK: _sh_wasm_data_segment_init(shr, shUnit,

;; The bytes are emitted as a unit-scope array, and the unit points at them.
;; CHECK: static const unsigned char s_binary_data[5]
;; CHECK: .binary_data = s_binary_data
;; CHECK-SAME: .binary_data_size = 5
