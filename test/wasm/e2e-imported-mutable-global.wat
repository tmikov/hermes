;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A mutable imported global is shared state: the module and the host see the
;; same global, so a global.set inside Wasm must be visible through the
;; importer's WebAssembly.Global, and a host write to `.value` must be visible
;; to the next global.get. The imported value used to be read once at link
;; time into the frame Variable backing the global, making the module's view a
;; snapshot: writes were lost in both directions. An immutable import is still
;; snapshotted, which is correct because its value cannot change.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-imported-mutable-global-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; Immutable import: a snapshot is correct.
  (import "e" "imm" (global $imm i32))
  ;; Mutable imports: must alias the host's WebAssembly.Global.
  (import "e" "counter" (global $counter (mut i32)))
  (import "e" "ratio" (global $ratio (mut f64)))
  (import "e" "big" (global $big (mut i64)))
  ;; A module-local global must keep using its own frame slot.
  (global $local (mut i32) (i32.const 7))

  (func (export "get_imm") (result i32) global.get $imm)

  (func (export "get_local") (result i32) global.get $local)
  (func (export "bump_local") (param i32)
    global.get $local
    local.get 0
    i32.add
    global.set $local)

  (func (export "get_counter") (result i32) global.get $counter)
  (func (export "bump_counter") (param i32)
    global.get $counter
    local.get 0
    i32.add
    global.set $counter)

  (func (export "get_ratio") (result f64) global.get $ratio)
  (func (export "scale_ratio") (param f64)
    global.get $ratio
    local.get 0
    f64.mul
    global.set $ratio)

  ;; Return the i64 halves separately, so a lost upper word is visible
  ;; directly rather than only through 64-bit arithmetic.
  (func (export "get_big_lo") (result i32) global.get $big i32.wrap_i64)
  (func (export "get_big_hi") (result i32)
    global.get $big
    i64.const 32
    i64.shr_u
    i32.wrap_i64)
  (func (export "add_big") (param i64)
    global.get $big
    local.get 0
    i64.add
    global.set $big)

  ;; Re-exporting an imported mutable global must hand back the global it
  ;; imported, not a fresh copy of its value at link time.
  (export "counter_export" (global $counter))
)

;; CHECK: imm = 42
;; CHECK-NEXT: local = 7
;; CHECK-NEXT: counter seen by wasm = 5
;; CHECK-NEXT: after wasm bump, host sees = 15
;; CHECK-NEXT: after host set, wasm sees = 100
;; CHECK-NEXT: ratio seen by wasm = 2.5
;; CHECK-NEXT: after wasm scale, host sees = 10
;; CHECK-NEXT: after host set, wasm sees = 0.5
;; CHECK-NEXT: big lo/hi seen by wasm = 0/1
;; CHECK-NEXT: after wasm add, host sees = 4294967297 bigint
;; CHECK-NEXT: after host set, wasm sees lo/hi = -1/-1
;; CHECK-NEXT: local after bump = 10
;; CHECK-NEXT: export aliases the imported global = true
;; CHECK-NEXT: export value = 100
;; CHECK-NEXT: done
