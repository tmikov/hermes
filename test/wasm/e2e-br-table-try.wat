;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A `br_table` whose target is a `try` with a result type.
;;
;; onBrTable filled the target's result phis from its own inline copy of the
;; peek loop, and that copy admitted only Block and If. A `try` target reached
;; the continuation contributing nothing, so the phi carried only whatever the
;; other edges had supplied and the branch's value was silently replaced by
;; one of theirs:
;;
;;   (try (result i32)
;;     (do (call $boom) (i32.const 9))
;;     (catch_all (i32.const 7) (br_table 0 (i32.const 0))))
;;
;; returned 9 in the caught direction -- the body's value, from the only edge
;; that had contributed -- rather than 7. It compiled and ran; only the answer
;; was wrong.
;;
;; onBrTable now calls peekBranchPhiOperands, which br_if already used and
;; which has admitted Try since the try-result fix. (`br` uses the consuming
;; addBranchPhiOperands instead; a br_table trampoline must peek, because
;; every trampoline reads the same values.) The two inline loops the
;; trampoline duplicated are gone.
;;
;; Every case here branches out of a HANDLER, which is outside the protected
;; region. A `br_table` out of a try BODY is a separate defect
;; (dz 01a09e72-e936): nothing emits the TryEndInst that closes the region, so
;; the region stays open past the branch. Some of those shapes are rejected --
;; the verifier refuses a `return` under an open try -- but it checks only
;; returns, and the ones it lets through are worse: an exception raised after
;; the try is routed to the handler the code has already left. This file will
;; be worth extending when that lands.

;; REQUIRES: wasm
;; RUN: %wat2wasm --enable-exceptions %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-br-table-try-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "boom" (func $boom))
  (tag $t_i32 (export "t") (param i32))

  ;; One case label and one default, both the try. Two labels is the fewest
  ;; that reaches the SwitchInst; a lone label is the default, which `def`
  ;; covers.
  (func (export "sw") (result i32)
    (try (result i32)
      (do (call $boom) (i32.const 9))
      (catch_all (i32.const 7) (br_table 0 0 (i32.const 0)))))

  ;; A lone label, which in wat is the default with no cases at all. onBrTable
  ;; takes a different path for that -- a plain branch rather than a
  ;; SwitchInst -- into the same trampoline.
  (func (export "def") (result i32)
    (try (result i32)
      (do (call $boom) (i32.const 9))
      (catch_all (i32.const 6) (br_table 0 (i32.const 0)))))

  ;; An i64 target occupies TWO slots and so two phis. The body's high word is
  ;; nonzero and the handler's is not, so delivering half the pair, or the
  ;; slots in the wrong order, shows up in the value.
  (func (export "i64sw") (result i64)
    (try (result i64)
      (do (call $boom) (i64.const 1234567890123))
      (catch_all (i64.const 999) (br_table 0 0 (i32.const 0)))))

  ;; Two results, over a sentinel the branch must NOT carry. The results are
  ;; combined with i32.sub, not i32.add, so their ORDER is pinned rather than
  ;; only their number: 3 and 10 arriving the other way round gives 507
  ;; instead of 493. The sentinel is what a peek window starting one slot too
  ;; low would reach, and it would arrive as the first result.
  (func (export "two") (result i32)
    (block (result i32)
      (i32.const 500)
      (try (result i32 i32)
        (do (call $boom) (i32.const 1) (i32.const 2))
        (catch_all (i32.const 3) (i32.const 10) (br_table 0 0 (i32.const 0))))
      (i32.sub)
      (i32.add)))

  ;; The body ALWAYS throws, so the handler's br_table is the only edge
  ;; reaching the continuation. That is what protects onBrTable's
  ;; branchTargeted assignment: everywhere else here the body falls through,
  ;; so onCatchAll has already set it and onEnd would call the continuation
  ;; reachable either way.
  (func (export "only") (result i32)
    (try (result i32)
      (do (throw $t_i32 (i32.const 0)))
      (catch_all (i32.const 3) (br_table 0 0 (i32.const 0)))))

  ;; Three case labels and a default, of two different kinds: labels 0 and 2
  ;; are the try, labels 1 and 3 the enclosing block. onBrTable makes one
  ;; trampoline per DEPTH, so the two try cases share one and the SwitchInst
  ;; sends two case values to it.
  ;;
  ;; The try's continuation adds 100, so the arms are distinguishable -- both
  ;; carrying the handler's 7 would otherwise look identical. The block arm is
  ;; the control: it went through the branch of the loop that was already
  ;; correct, so it answered right before this fix and must still.
  (func (export "share") (param i32) (result i32)
    (block (result i32)
      (try (result i32)
        (do (call $boom) (i32.const 9))
        (catch_all (i32.const 7) (br_table 0 1 0 1 (local.get 0))))
      (i32.const 100)
      (i32.add))))

;; Nothing is thrown, so no handler runs and no br_table is reached. These are
;; the rows that say the fix did not simply route everything down the branch
;; edge: the body's value still arrives when the body is what ran. `only` has
;; no such row -- its body always throws.
;; CHECK: sw() with no throw: 9
;; CHECK-NEXT: def() with no throw: 9
;; CHECK-NEXT: i64sw() with no throw: 1234567890123
;; CHECK-NEXT: two() with no throw: 499
;; CHECK-NEXT: share(0) with no throw: 109

;; The handler runs and branches. Its value is what the try produces.
;; CHECK-NEXT: sw() catching 77: 7
;; CHECK-NEXT: def() catching 77: 6
;; CHECK-NEXT: i64sw() catching 77: 999
;; CHECK-NEXT: two() catching 77: 493
;; CHECK-NEXT: only(): 3

;; Indices 0 and 2 take the try, whose continuation adds 100. Indices 1 and 3
;; leave the block directly with the handler's 7; index 3 arrives there as the
;; default rather than as a case.
;; CHECK-NEXT: share(0) catching 77: 107
;; CHECK-NEXT: share(1) catching 77: 7
;; CHECK-NEXT: share(2) catching 77: 107
;; CHECK-NEXT: share(3) catching 77: 7
;; CHECK-NEXT: done
