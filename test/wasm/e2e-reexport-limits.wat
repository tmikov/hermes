;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Re-exporting an imported memory or table published the *import
;; declaration's* limits rather than the limits of the object actually
;; supplied. The declaration is only a lower bound, so this module -- which
;; declares no minimum and no maximum at all -- advertised min 0 and "no
;; maximum" for a memory that really has 2 pages and a maximum of 3, and for
;; a table that really has 2 entries and a maximum of 5. A module importing
;; the re-export then failed to link with a spurious LinkError even though
;; the underlying memory and table satisfied its declaration exactly.
;;
;; The limits now come from the values recorded from the imported object at
;; validation time, the same ones memory.grow and the buffer allocation
;; already use.

;; REQUIRES: wasm
;; RUN: %wat2wasm %S/e2e-reexport-limits-exporter.wat_ -o %t-exp.wasm && %hermesc --wasm -emit-binary -out %t-exp.hbc %t-exp.wasm && %wat2wasm %S/e2e-reexport-limits-consumer.wat_ -o %t-cons.wasm && %hermesc --wasm -emit-binary -out %t-cons.hbc %t-cons.wasm && %wat2wasm %S/e2e-reexport-limits-own.wat_ -o %t-own.wasm && %hermesc --wasm -emit-binary -out %t-own.hbc %t-own.wasm && %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-reexport-limits-driver.js_ -- %t-exp.hbc %t.hbc %t-cons.hbc %t-own.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "mem" (memory 0))
  (import "e" "tbl" (table 0 funcref))
  (export "mem2" (memory 0))
  (export "tbl2" (table 0)))

;; The re-export must describe the memory and table that were supplied, not
;; the (0, unbounded) declarations above.
;; CHECK: re-exported memory: min = 2, max = 3
;; CHECK-NEXT: re-exported table: min = 2, max = 5

;; And a module whose declaration those limits satisfy must link and run.
;; CHECK-NEXT: consumer memory.size = 2
;; CHECK-NEXT: consumer call0 = 42

;; A locally defined memory and table still report their own declarations --
;; the runtime values must not have displaced the static ones.
;; CHECK-NEXT: own memory: min = 1, max = -1
;; CHECK-NEXT: own table: min = 3, max = 7

;; Re-exporting those: an absent maximum has to stay absent. -1 is the
;; sentinel for it, and both constructors reject it as a `maximum`, so it
;; must be omitted rather than passed through.
;; CHECK-NEXT: re-export of own memory: min = 1, max = -1
;; CHECK-NEXT: re-export of own table: min = 3, max = 7
;; CHECK-NEXT: done
