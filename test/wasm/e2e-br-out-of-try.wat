;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A `br`, `br_if` or `br_table` that leaves the body of a `try`.
;;
;; Only onCatch, onCatchAll, onDelegate and onEnd emitted the TryEndInst that
;; closes a protected region. A branch out of the body emitted a plain
;; BranchInst and nothing else, so the region never closed on that path.
;; Exceptions.cpp walks forward from the TryStartInst adding every block it
;; reaches to the enclosing region and stops only at a TryEndInst or a throw,
;; so the region simply carried on past the branch.
;;
;; Some of those shapes were rejected -- the verifier refuses a `return` under
;; an open try -- and the ones it let through were worse:
;;
;;   (try (do (br 0)) (catch_all (return (i32.const 1))))
;;   (throw $t)
;;
;; compiled, and the `throw` AFTER the try was routed to the handler the code
;; had already left. It returned 1 instead of letting the exception escape.
;;
;; A branch now emits one TryEndInst per region it leaves, innermost first,
;; each in its own block. That order is what both consumers expect: the
;; verifier tracks the innermost enclosing try per block and requires each
;; TryEndInst's catch target to match it, and Exceptions.cpp hands the block
;; after a TryEndInst back to the enclosing level, which then sees the next
;; one.
;;
;; The chain also moves where the branch itself is emitted from, so the phi
;; operands a branch carrying a result contributes have to be recorded against
;; the LAST block of the chain rather than the block the branch was written
;; in. brValueTwo, brValueUTwo and brTableTwo are that -- their single-region
;; counterparts are not, for the reason given on them.

;; REQUIRES: wasm
;; RUN: %wat2wasm --enable-exceptions %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-br-out-of-try-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "boom" (func $boom))
  (tag $t_i32 (export "t") (param i32))

  ;; `br 0` from inside a try body targets the try's OWN continuation, so it
  ;; leaves the body and the region has to close. The throw after the try is
  ;; what says whether it did: it is outside, and must escape.
  (func (export "brSelf") (param i32) (result i32)
    (try
      (do
        (br_if 0 (local.get 0))
        (call $boom))
      (catch_all (return (i32.const 1))))
    (throw $t_i32 (i32.const 5)))

  ;; Crossing one try to reach an enclosing block.
  (func (export "brOuter") (param i32) (result i32)
    (block $out
      (try
        (do
          (br_if 1 (local.get 0))
          (call $boom))
        (catch_all (return (i32.const 2)))))
    (throw $t_i32 (i32.const 6)))

  ;; Crossing TWO nested tries, which is what makes this a chain rather than a
  ;; single instruction. A fix that closed only the innermost would leave the
  ;; outer region open here and nowhere above.
  (func (export "brTwo") (param i32) (result i32)
    (block $out
      (try
        (do
          (try
            (do
              (br_if 2 (local.get 0))
              (call $boom))
            (catch_all (return (i32.const 3)))))
        (catch_all (return (i32.const 4)))))
    (throw $t_i32 (i32.const 7)))

  ;; A branch out of an inner HANDLER, crossing the enclosing try's body. The
  ;; inner region closed when its handler started, so exactly one TryEndInst
  ;; belongs here; emitting a second would be rejected as "TryEndInst does not
  ;; match TryStartInst".
  ;;
  ;; The `return` at the end is not part of that: it sits AFTER the outer try,
  ;; and the handler's branch skips it. It is the quiet path's answer.
  (func (export "brFromHandler") (result i32)
    (block $out
      (try
        (do
          (try
            (do (call $boom))
            (catch_all (br 2))))
        (catch_all (return (i32.const 5))))
      (return (i32.const 9)))
    (throw $t_i32 (i32.const 8)))

  ;; The three below carry a RESULT across the chain. The phi operand has to
  ;; name the last block of the chain, which is the actual predecessor of the
  ;; continuation, not the block the branch was written in.
  (func (export "brValue") (param i32) (result i32)
    (block $out (result i32)
      (try
        (do
          ;; br_if leaves its branch value on the stack when not taken, so the
          ;; drop is what the not-taken path needs; it says nothing about the
          ;; chain.
          (br_if 1 (i32.const 40) (local.get 0))
          (drop)
          (call $boom))
        (catch_all))
      (i32.const 41)))

  (func (export "brValueU") (result i32)
    (block $out (result i32)
      (try
        (do
          (call $boom)
          (br 1 (i32.const 40)))
        (catch_all))
      (i32.const 41)))

  (func (export "brTableOut") (param i32) (result i32)
    (block $out (result i32)
      (try
        (do
          (call $boom)
          (i32.const 40)
          (br_table 1 1 (local.get 0)))
        (catch_all))
      (i32.const 41)))

  ;; The same three across TWO tries. One try is not enough to pin where the
  ;; phi operand is recorded. The chain is then a single block holding nothing
  ;; but the branch, and SimplifyCFG bypasses it towards its DESTINATION, which
  ;; leaves the block the branch was written in as the continuation's direct
  ;; predecessor -- the very block the wrong choice named. Recording the
  ;; operand either way ends up correct. With two regions the block that
  ;; finally reaches the continuation is neither.
  (func (export "brValueTwo") (param i32) (result i32)
    (block $out (result i32)
      (try
        (do
          (try
            (do
              (br_if 2 (i32.const 40) (local.get 0))
              (drop)
              (call $boom))
            (catch_all)))
        (catch_all))
      (i32.const 41)))

  (func (export "brValueUTwo") (result i32)
    (block $out (result i32)
      (try
        (do
          (try
            (do
              (call $boom)
              (br 2 (i32.const 40)))
            (catch_all)))
        (catch_all))
      (i32.const 41)))

  (func (export "brTableTwo") (param i32) (result i32)
    (block $out (result i32)
      (try
        (do
          (try
            (do
              (call $boom)
              (i32.const 40)
              (br_table 2 2 (local.get 0)))
            (catch_all)))
        (catch_all))
      (i32.const 41)))

  ;; A branch that leaves an INNER try and stays inside an outer one. It
  ;; crosses one region and must close exactly that one: the call after it is
  ;; still in the outer try's body and must still be caught. The earlier
  ;; branches all leave every region they are inside, so a chain that closed
  ;; all the ACTIVE regions rather than the crossed ones would pass them.
  (func (export "brInner") (param i32) (result i32)
    (try (result i32)
      (do
        (block $mid
          (try
            (do
              (br_if 1 (local.get 0))
              (call $boom))
            (catch_all)))
        (call $boom)
        (i32.const 34))
      (catch_all (i32.const 33))))

  ;; A br_table whose two targets are at DIFFERENT depths: case 0 leaves the
  ;; inner try only, landing inside the outer try's body, and the default
  ;; leaves both. The chain is per target, so one length cannot serve both,
  ;; and the earlier tables cannot say so -- their case and default depths are
  ;; equal.
  (func (export "brTableDepths") (param i32) (result i32)
    (local $r i32)
    (try
      (do
        (block $mid
          (try
            (do (br_table 1 2 (local.get 0)))
            (catch_all)))
        (local.set $r (i32.const 50))
        (call $boom))
      (catch_all (local.set $r (i32.const 51))))
    (local.get $r))

  ;; A return from inside TWO try bodies. brTwo's returns are in handlers, so
  ;; at most one region is active at each of them -- none at all in its outer
  ;; handler -- and a return that closed just the first would satisfy them.
  (func (export "retTwo") (result i32)
    (try
      (do
        (try
          (do (return (i32.const 61)))
          (catch_all)))
      (catch_all))
    (i32.const 62))

  ;; The same with MULTIPLE results, which return through the buffer rather
  ;; than directly. That is a different return sequence, and the chain is
  ;; emitted before all of it.
  (func (export "retBuf") (result i32 i32)
    (try
      (do
        (try
          (do (return (i32.const 63) (i32.const 64)))
          (catch_all)))
      (catch_all))
    (i32.const 0)
    (i32.const 0))

  ;; A value-bearing table whose two destinations are DIFFERENT and both
  ;; exercised. Each trampoline reads the same stack, so they have to peek:
  ;; the first to POP would leave the second with nothing. brTableOut and
  ;; brTableTwo cannot say that -- both their labels are the same depth, so
  ;; there is only one trampoline -- and brTableDepths, which does have two,
  ;; carries no value.
  (func (export "brTableValues") (param i32) (result i32)
    (block $outer (result i32)
      (try (result i32)
        (do
          (call $boom)
          (i32.const 70)
          (br_table 0 1 (local.get 0)))
        (catch_all (i32.const 71)))
      (i32.const 100)
      (i32.add)))

  ;; The control: a branch that crosses no try at all. It must emit no
  ;; TryEndInst and behave exactly as before.
  (func (export "plain") (param i32) (result i32)
    (block $out (result i32)
      (br_if 0 (i32.const 40) (local.get 0))
      (drop)
      (i32.const 41))))

;; The branch is taken, so the code after the try is reached with the region
;; closed and the throw that lives there escapes. Every one of these returned
;; a value instead, or did not compile, before the fix.
;; CHECK: brSelf(1): escaped with 5
;; CHECK-NEXT: brOuter(1): escaped with 6
;; CHECK-NEXT: brTwo(1): escaped with 7

;; The branch is not taken and the import throws, so the handler runs. This is
;; the direction that says the regions still CATCH what they are supposed to.
;; CHECK-NEXT: brSelf(0) throwing: 1
;; CHECK-NEXT: brOuter(0) throwing: 2
;; CHECK-NEXT: brTwo(0) throwing: 3
;; CHECK-NEXT: brFromHandler() throwing: escaped with 8

;; The branch is not taken and nothing throws, so the body falls through and
;; the same code after the try is reached that way instead.
;; CHECK-NEXT: brSelf(0) quiet: escaped with 5
;; CHECK-NEXT: brOuter(0) quiet: escaped with 6
;; CHECK-NEXT: brTwo(0) quiet: escaped with 7
;; CHECK-NEXT: brFromHandler() quiet: 9

;; Branches carrying a value. 40 arrives over the chain; 41 is the
;; fall-through, which reaches the continuation by the ordinary path.
;; CHECK-NEXT: brValue(1) quiet: 40
;; CHECK-NEXT: brValue(0) quiet: 41
;; CHECK-NEXT: brValue(0) throwing: 41
;; CHECK-NEXT: brValueU() quiet: 40
;; CHECK-NEXT: brValueU() throwing: 41
;; CHECK-NEXT: brTableOut(0) quiet: 40
;; CHECK-NEXT: brTableOut(1) quiet: 40
;; CHECK-NEXT: brTableOut(0) throwing: 41
;; CHECK-NEXT: brValueTwo(1) quiet: 40
;; CHECK-NEXT: brValueTwo(0) quiet: 41
;; CHECK-NEXT: brValueTwo(0) throwing: 41
;; CHECK-NEXT: brValueUTwo() quiet: 40
;; CHECK-NEXT: brValueUTwo() throwing: 41
;; CHECK-NEXT: brTableTwo(0) quiet: 40
;; CHECK-NEXT: brTableTwo(0) throwing: 41

;; brInner's branch leaves the inner try and stays in the outer one, so the
;; call after it is still caught. brTableDepths(0) is the same through a table
;; target; its default leaves both regions, reaching the outer continuation
;; with the local untouched.
;; CHECK-NEXT: brInner(1) quiet: 34
;; CHECK-NEXT: brInner(1) throwing: 33
;; CHECK-NEXT: brInner(0) throwing: 33
;; CHECK-NEXT: brTableDepths(0) quiet: 50
;; CHECK-NEXT: brTableDepths(0) throwing: 51
;; CHECK-NEXT: brTableDepths(1) quiet: 0
;; CHECK-NEXT: brTableDepths(1) throwing: 0

;; Returns from inside two try bodies at once, single and buffered.
;; CHECK-NEXT: retTwo(): 61
;; CHECK-NEXT: retBuf(): 63,64

;; Target 0 is the try, whose continuation adds 100; target 1 is the block
;; outside it, which takes the value as it stands.
;; CHECK-NEXT: brTableValues(0) quiet: 170
;; CHECK-NEXT: brTableValues(1) quiet: 70
;; CHECK-NEXT: brTableValues(0) throwing: 171
;; CHECK-NEXT: plain(1): 40
;; CHECK-NEXT: plain(0): 41
;; CHECK-NEXT: done
