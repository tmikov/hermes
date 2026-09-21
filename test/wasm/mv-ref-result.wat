;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Multi-value results (and single i64 results) travel through a per-module
;; return buffer: an ArrayBuffer with a Uint32Array view and a Float64Array
;; view. A funcref in this implementation is a JS closure and an externref is
;; an arbitrary JS value, and neither can live in an ArrayBuffer. Storing one
;; through the Uint32Array view coerced it to NaN and then to 0 -- the value
;; was destroyed AT THE STORE, and the load faithfully read back that 0. The
;; export wrapper could only choose between handing the caller a bogus 0 and
;; warning that the result type was unsupported.
;;
;; The buffer now has a parallel reference array, indexed identically to the
;; Uint32Array view (a reference reserves the same 4 bytes an i32 does, and
;; uses the same byteOff/4 slot index), so references survive the round trip
;; and the export wrapper returns the real value. The diagnostic this test
;; used to pin is therefore gone for funcref and externref; V128 keeps it.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>&1 | %FileCheck --check-prefix=WARN --allow-empty --match-full-lines %s
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>/dev/null && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/mv-ref-result-driver.js_ -- %t.hbc | %FileCheck --check-prefix=EXEC --match-full-lines %s

(module
  (table 1 funcref)
  (func $f (result i32) (i32.const 7))
  (elem (i32.const 0) $f)

  ;; Multi-value result containing a funcref: goes through the return buffer.
  (func (export "mv") (result i32 funcref)
    (i32.const 42)
    (table.get (i32.const 0)))

  ;; A single funcref result does NOT go through the return buffer -- the
  ;; wrapper returns the callee's value directly -- so this path was never
  ;; broken. It is the control: whatever the multi-value path returns must
  ;; match it.
  (func (export "single") (result funcref)
    (table.get (i32.const 0)))

  ;; An externref is an arbitrary JS value, not a closure. Same buffer, same
  ;; failure before the fix.
  (func (export "mvExtern") (param externref) (result i32 externref)
    (i32.const 99)
    (local.get 0))

  ;; Two references and an i32 in one result list, to pin the slot indexing:
  ;; the funcref takes slot 0 and the externref slot 1 of the reference array,
  ;; while the i32 takes slot 2 of the integer view. Neither view's writes may
  ;; disturb the other's.
  (func (export "mvTwo") (param externref) (result funcref externref i32)
    (table.get (i32.const 0))
    (local.get 0)
    (i32.const 5)))

;; No diagnostic at all: funcref and externref results are supported now, so
;; the compile is silent. (This is the check the previous behavior fails --
;; it emitted "warning: unsupported Wasm result type: funcref".)
;; WARN-NOT: warning

;; The reference halves are the real values, not 0 and not undefined.
;; EXEC: mv: [42, function calls -> 7]
;; EXEC-NEXT: single: function calls -> 7
;; EXEC-NEXT: mvExtern: [99, same=true]
;; EXEC-NEXT: mvTwo: [function calls -> 7, same=true, 5]
