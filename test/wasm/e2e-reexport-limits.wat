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
;; Neither kind publishes limits any more -- they moved into internal state
;; along with the rest of the linking ABI, and the spec gives a Memory and a
;; Table no such accessors. Re-exporting an imported memory or table now
;; publishes the very object that was imported, which is what the spec says an
;; export of an import is, and, since the storage lives in the object's
;; internal fields, the only way it can still be shared. That identity
;; subsumes the old assertion: the same object cannot report different limits.
;; What the limits still are is pinned below by what links and by what grows.

;; REQUIRES: wasm
;; RUN: %wat2wasm %S/e2e-reexport-limits-exporter.wat_ -o %t-exp.wasm && %hermesc --wasm -emit-binary -out %t-exp.hbc %t-exp.wasm && %wat2wasm %S/e2e-reexport-limits-consumer.wat_ -o %t-cons.wasm && %hermesc --wasm -emit-binary -out %t-cons.hbc %t-cons.wasm && %wat2wasm %S/e2e-reexport-limits-own.wat_ -o %t-own.wasm && %hermesc --wasm -emit-binary -out %t-own.hbc %t-own.wasm && %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-reexport-limits-driver.js_ -- %t-exp.hbc %t.hbc %t-cons.hbc %t-own.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "mem" (memory 0))
  (import "e" "tbl" (table 0 funcref))
  (export "mem2" (memory 0))
  (export "tbl2" (table 0)))

;; The re-export must BE the memory and table that were supplied, not fresh
;; objects described by the (0, unbounded) declarations above.
;; CHECK: re-exported memory is the exporter's own object: true
;; CHECK-NEXT: re-exported memory pages: 2
;; CHECK-NEXT: re-exported table is the exporter's own object: true
;; CHECK-NEXT: re-exported table length: 2

;; And a module whose declaration those limits satisfy must link and run. The
;; consumer declares (table 2 5), so this is what checks the table's maximum:
;; it links only if the maximum read from the table itself is 5.
;; CHECK-NEXT: consumer memory.size = 2
;; CHECK-NEXT: consumer call0 = 42

;; A locally defined memory and table still get their own declarations --
;; the runtime values must not have displaced the static ones.
;; CHECK-NEXT: own memory pages: 1
;; CHECK-NEXT: own table length: 3

;; Re-exporting those: an absent maximum has to stay absent. Be precise about
;; what the two lines below actually pin, because an earlier version of this
;; comment claimed both directions and neither of them holds:
;;   * the grow goes through Memory.prototype.grow, which reads maxPages_
;;     directly and never sees the -1 the link path uses, so nothing about the
;;     sentinel can affect it;
;;   * the unbounded memory fails (memory 2 3) for the arithmetic reason --
;;     65536 exceeds 3 -- whichever way "no maximum" is spelled.
;; What they do pin is that a re-exported import is the same object and that
;; its limits travel with it. The direction "a real maximum must not be
;; reported as the sentinel" is pinned by `consumer memory.size` above, which
;; needs the exporter's maximum of 3 to be reported as 3. The reverse
;; direction -- an absent maximum must not be reported as a real one -- is
;; pinned only by e2e-memory-import-metadata-max64k.wat_, where the
;; declaration's maximum is 65536 and the comparison cannot decide.
;; CHECK-NEXT: re-export of own memory is the same object: true
;; CHECK-NEXT: re-export of own memory grew to: 3 pages
;; CHECK-NEXT: unbounded memory vs (memory 2 3): LinkError: import e.m does not satisfy the declared memory limits
;; CHECK-NEXT: re-export of own table is the same object: true

;; The declared table maximum of 7 is enforced, which is the only remaining
;; way to observe it. Done last, because it mutates the table.
;; CHECK-NEXT: own table grow(4): 3, length 7
;; CHECK-NEXT: own table grow(1): RangeError, length 7
;; CHECK-NEXT: done
