;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Two ways table.grow ignored a limit.
;;
;; A table declaring no maximum was handed UINT32_MAX as its ceiling, so
;; `newLen > maxEntries` never fired and an enormous delta was not refused
;; but attempted: the fill loop ran for billions of iterations, growing
;; indexed storage each time, until the process died. The spec's answer for
;; a grow that cannot be allocated is -1, and there is now an engine limit
;; that produces it.
;;
;; And growing an *imported* table used the import declaration's maximum.
;; Link validation only requires the supplied table's maximum to be no
;; larger than the declared one, so a module declaring (table 2 10) over a
;; table whose owner declared (table 2 4) could grow it to 10 -- past what
;; the owner allows, in storage they share.

;; REQUIRES: wasm
;; RUN: %wat2wasm %S/e2e-table-grow-limits-exporter.wat_ -o %t-exp.wasm && %hermesc --wasm -emit-binary -out %t-exp.hbc %t-exp.wasm && %wat2wasm %S/e2e-table-grow-limits-unbounded.wat_ -o %t-unb.wasm && %hermesc --wasm -emit-binary -out %t-unb.hbc %t-unb.wasm && %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-table-grow-limits-driver.js_ -- %t-exp.hbc %t.hbc %t-unb.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "t" (table 2 10 funcref))
  (func (export "grow") (param i32) (result i32)
    ref.null func
    local.get 0
    table.grow 0)
  (func (export "size") (result i32) table.size 0))

;; The exporter declared a maximum of 4; the importer declaring 10 does not
;; raise it.
;; CHECK: grow past the exporter's max: -1
;; CHECK-NEXT: grow within both: 2
;; CHECK-NEXT: exporter sees size: 4

;; An unbounded table must answer -1 rather than trying, and must survive it.
;; CHECK-NEXT: huge grow on unbounded table: -1
;; CHECK-NEXT: unbounded table still usable, size: 1
;; CHECK-NEXT: done
