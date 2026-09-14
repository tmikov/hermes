;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A `try` that declares a RESULT TYPE and whose body reaches the end of the
;; block without throwing or branching.
;;
;; This did not compile. onTry creates the continuation block's result phis,
;; and the catch arm adds its operands, but the edge from the try body -- the
;; TryEndInst that jumps to the continuation when nothing was thrown --
;; contributed none. The phi then had one operand for two incoming edges, and
;; the value it did have was the catch handler's payload load, which does not
;; dominate the continuation:
;;
;;   Operand %8 must dominate the Instruction %2 in function "wasm_func_1"
;;   error: Lowered IR verification failed
;;
;; The body's own result was not merely unused; it never reached the IR.
;;
;; No existing test had this shape: e2e-try-catch.wat's result case ends its
;; body in `throw`, and e2e-exception-payload-ref.wat deliberately routes
;; results through locals to stay clear of it and says so.
;;
;; The parameterized cases at the bottom are a second defect the first fix
;; ran into rather than caused: onTry never subtracted the block's params
;; from the recorded stack height, which onBlock has always done. No test had
;; ever used a `try` with params, and the shapes that did compile answered
;; wrongly -- see `pca`.

;; REQUIRES: wasm
;; RUN: %wat2wasm --enable-exceptions %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-try-result-fallthrough-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "boom" (func $boom))
  (tag $t_i32 (export "t") (param i32))

  ;; The shape that would not compile: a result type, and a body that falls
  ;; through to the end.
  (func (export "c") (result i32)
    (try (result i32)
      (do (call $boom) (i32.const 9))
      (catch $t_i32)))

  ;; Two results, so a multi-value continuation phi is covered too, and the
  ;; operands have to arrive in the right ORDER rather than merely in the
  ;; right number.
  (func (export "c2") (result i32 i32)
    (try (result i32 i32)
      (do (call $boom) (i32.const 3) (i32.const 4))
      (catch $t_i32 (i32.const 0))))

  ;; A nested try with a result, inside the body of another. The two
  ;; handlers add DIFFERENT constants to the payload, so the caught direction
  ;; says which one ran: returning the payload unchanged from both would pass
  ;; equally if the exception reached the wrong one.
  (func (export "cn") (result i32)
    (try (result i32)
      (do
        (try (result i32)
          (do (call $boom) (i32.const 5))
          (catch $t_i32 (i32.const 1000) (i32.add))))
      (catch $t_i32 (i32.const 2000) (i32.add))))

  ;; catch_all as the FIRST handler takes its own path through onCatchAll,
  ;; which had the identical hole.
  (func (export "ca") (result i32)
    (try (result i32)
      (do (call $boom) (i32.const 8))
      (catch_all (i32.const 1))))

  ;; An i64 result occupies TWO stack slots and so two phis, and a mixed
  ;; i64+i32 result pins that the slots arrive in the right order rather than
  ;; merely in the right number -- addBranchPhiOperands counts slots, not
  ;; results, so this is where a fix that counted results would go wrong.
  (func (export "i64c") (result i64)
    try (result i64)
      call $boom
      i64.const 1234567890123
    catch $t_i32
      drop
      i64.const 999
    end)
  (func (export "mix") (result i64 i32)
    try (result i64 i32)
      call $boom
      i64.const 77
      i32.const 5
    catch $t_i32
      drop
      i64.const 0
      i32.const 0
    end)

  ;; A try that declares PARAMS as well as a result. onTry recorded the
  ;; stack height at the `try` without subtracting the params, so the params
  ;; were counted as still being on the enclosing stack while the body
  ;; consumed them as its own -- and the same height is what a handler is
  ;; restored to, where it is wrong for the same reason. `pca` below is the
  ;; shape that showed it; the numbers are on that function.
  (func (export "p") (result i32)
    (i32.const 40)
    (i32.const 2)
    (try (param i32) (result i32)
      (do (call $boom))
      (catch $t_i32))
    (i32.add))

  ;; An i64 param occupies TWO stack slots, so a fix that subtracted one slot
  ;; per param rather than per slot would be off by one here and nowhere
  ;; above.
  (func (export "pi64") (result i64)
    (i64.const 1000)
    (i64.const 7)
    (try (param i64) (result i64)
      (do (call $boom))
      (catch $t_i32 (drop) (i64.const 999)))
    (i64.add))

  ;; The shape that showed the defect, and a parameterized first-handler
  ;; catch_all, which p/pi64/pmix do not cover -- they all use typed catches.
  ;; It answered 101 rather than 42: the handler's 99 (the only operand the
  ;; continuation phi had, since the body's edge contributed none) plus the
  ;; param nobody had removed. Once the body's edge started popping its
  ;; result, the same mismatch left the stack one below the recorded height,
  ;; onEnd refilled it with a null, and Type::isNumberType dereferenced it.
  (func (export "pca") (result i32)
    (i32.const 40)
    (i32.const 2)
    (try (param i32) (result i32)
      (do (call $boom))
      (catch_all (i32.const 99)))
    (i32.add))

  ;; The body CONSUMES its param and then throws, so the handler runs with
  ;; the param already gone. Everything above throws before touching it.
  (func (export "pafter") (result i32)
    (i32.const 40)
    (i32.const 2)
    (try (param i32) (result i32)
      (do (i32.const 1000) (i32.add) (call $boom))
      (catch $t_i32))
    (i32.add))

  ;; Mixed params with a sentinel underneath them. The sentinel is what the
  ;; result is added to, so it fails loudly if the restored height is off in
  ;; either direction.
  (func (export "pmix") (result i32)
    (i32.const 100)
    (i32.const 3)
    (i64.const 4)
    (try (param i32 i64) (result i32)
      (do (call $boom) (drop))
      (catch $t_i32))
    (i32.add))

  ;; Every handler rethrows, so the continuation is reachable ONLY from the
  ;; try body's edge. That is the half of the fix that marks the edge as
  ;; targeting the continuation: without it the continuation is treated as
  ;; unreachable even though the body reaches it.
  ;; The handler raises 55 rather than passing the caught 77 back out. With
  ;; the same payload the import throws, an exception that never reached a
  ;; handler at all would arrive looking exactly like one the handler
  ;; rethrew.
  (func (export "cr") (result i32)
    (try (result i32)
      (do (call $boom) (i32.const 6))
      (catch $t_i32 (drop) (throw $t_i32 (i32.const 55)))))

  ;; The same shape with catch_all, which reaches the assignment through
  ;; onCatchAll rather than onCatch. `ca` above cannot stand in for it: its
  ;; handler falls through, so onEnd's own fallsThrough is already true there
  ;; and deleting onCatchAll's assignment would not change the answer.
  (func (export "car") (result i32)
    (try (result i32)
      (do (call $boom) (i32.const 4))
      (catch_all (throw $t_i32 (i32.const 0))))))

;; The body falls through, so the value is the body's.
;; CHECK: c() with no throw: 9
;; CHECK-NEXT: c2() with no throw: 3,4
;; CHECK-NEXT: cn() with no throw: 5
;; CHECK-NEXT: ca() with no throw: 8
;; CHECK-NEXT: i64c() with no throw: 1234567890123
;; CHECK-NEXT: mix() with no throw: 77,5
;; CHECK-NEXT: cr() with no throw: 6
;; CHECK-NEXT: car() with no throw: 4
;; CHECK-NEXT: p() with no throw: 42
;; CHECK-NEXT: pi64() with no throw: 1007
;; CHECK-NEXT: pca() with no throw: 42
;; CHECK-NEXT: pafter() with no throw: 1042
;; CHECK-NEXT: pmix() with no throw: 103

;; And the catch arm still works, so the fix did not simply route everything
;; down one side. The payload the driver throws is what comes back.
;; CHECK-NEXT: c() catching 77: 77
;; CHECK-NEXT: c2() catching 77: 77,0
;; CHECK-NEXT: cn() catching 77: 1077
;; CHECK-NEXT: ca() catching 77: 1
;; CHECK-NEXT: i64c() catching 77: 999
;; CHECK-NEXT: mix() catching 77: 0,0

;; cr's and car's handlers rethrow, so the exception leaves the function and
;; reaches the driver rather than producing a result. Each raises its own
;; payload -- cr raises 55, car raises 0, neither of them the 77 the import
;; throws -- and the driver checks which arrived. A bare "it threw" would be
;; satisfied by the import's own exception sailing past a handler that never
;; ran, and so would a handler that merely passed the caught payload back.
;; CHECK-NEXT: cr() catching 77: the handler rethrew, payload 55
;; CHECK-NEXT: car() catching 77: the handler rethrew, payload 0

;; The handler's stack starts below the params: an exception unwinds past
;; what the body had already consumed. Each of these adds the handler's
;; result to the value that was on the stack BEFORE the params, so a param
;; left behind shows up in the sum.
;; CHECK-NEXT: p() catching 77: 117
;; CHECK-NEXT: pi64() catching 77: 1999
;; CHECK-NEXT: pca() catching 77: 139
;; CHECK-NEXT: pafter() catching 77: 117
;; CHECK-NEXT: pmix() catching 77: 177
;; CHECK-NEXT: done
