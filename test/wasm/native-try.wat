;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Native counterpart of test/wasm/e2e-try-delegate.wat: try/catch/delegate
;; exception handling compiled to native code. The module and expected
;; answers are copied from that file; see there for why each case matters.
;; As in the original, the module drops the two return_call functions the
;; spec suite's version has -- return_call is the tail-call proposal, which
;; this frontend does not support and the whole module fails to load with it
;; present.

;; REQUIRES: shermes, wasm
;; RUN: %wat2wasm --enable-exceptions %s -o %t.wasm
;; RUN: %shermes -exported-unit=trymod -c -o %t-mod.o %t.wasm
;; RUN: %shermes -o %t.exe -Wc,%t-mod.o %S/native-try-driver.js_
;; RUN: %t.exe | %FileCheck --match-full-lines %s
;; RUN: %hermesc --wasm -emit-binary -out %t.hbc %t.wasm
;; RUN: %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/native-try-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; Exported so the driver can tell a Wasm exception from a trap or a
  ;; runtime error.
  (tag $e0 (export "e0"))
  (tag $e1 (export "e1"))

  (func (export "delegate-no-throw") (result i32)
    (try $t (result i32)
      (do (try (result i32) (do (i32.const 1)) (delegate $t)))
      (catch $e0 (i32.const 2))
    )
  )

  (func $throw-if (param i32)
    (local.get 0)
    (if (then (throw $e0)) (else))
  )

  (func (export "delegate-throw") (param i32) (result i32)
    (try $t (result i32)
      (do
        (try (result i32)
          (do (local.get 0) (call $throw-if) (i32.const 1))
          (delegate $t)
        )
      )
      (catch $e0 (i32.const 2))
    )
  )

  ;; The one the defect is named for: the delegation must skip the handler
  ;; returning 2 and reach the one returning 3.
  (func (export "delegate-skip") (result i32)
    (try $t (result i32)
      (do
        (try (result i32)
          (do
            (try (result i32)
              (do (throw $e0) (i32.const 1))
              (delegate $t)
            )
          )
          (catch $e0 (i32.const 2))
        )
      )
      (catch $e0 (i32.const 3))
    )
  )

  ;; The target is a block rather than a try, so there is no region of its own
  ;; to stop at and the throw lands in whatever encloses the block.
  (func (export "delegate-to-block") (result i32)
    (try (result i32)
      (do (block (try (do (throw $e0)) (delegate 0)))
          (i32.const 0))
      (catch_all (i32.const 1)))
  )

  (func (export "delegate-to-catch") (result i32)
    (try (result i32)
      (do (try
            (do (throw $e0))
            (catch $e0
              (try (do (rethrow 1)) (delegate 0))))
          (i32.const 0))
      (catch_all (i32.const 1)))
  )

  ;; Delegating to the function label: the exception leaves the function
  ;; rather than reaching any handler in it.
  (func (export "delegate-to-caller-trivial")
    (try
      (do (throw $e0))
      (delegate 0))
  )

  (func (export "delegate-to-caller-skipping")
    (try (do (try (do (throw $e0)) (delegate 1))) (catch_all))
  )

  (func $select-tag (param i32)
    (block (block (block (local.get 0) (br_table 0 1 2)) (return)) (throw $e0))
    (throw $e1)
  )

  (func (export "delegate-merge") (param i32 i32) (result i32)
    (try $t (result i32)
      (do
        (local.get 0)
        (call $select-tag)
        (try
          (result i32)
          (do (local.get 1) (call $select-tag) (i32.const 1))
          (delegate $t)
        )
      )
      (catch $e0 (i32.const 2))
    )
  )

  (func (export "delegate-throw-no-catch") (result i32)
    (try (result i32)
      (do (try (result i32) (do (throw $e0) (i32.const 1)) (delegate 0)))
      (catch $e1 (i32.const 2))
    )
  )

  ;; Deeply nested tries with three delegations: the innermost anonymous one
  ;; to $l1, $l1 itself to $l3, and a third inside $l3's handler back to $l3.
  ;; $l0 and $l2 are the ones those delegations must skip, and their handlers
  ;; are `catch_all unreachable`, so landing on either is supposed to trap
  ;; rather than answer.
  (func (export "delegate-correct-targets") (result i32)
    (try (result i32)
      (do (try $l3
            (do (try $l2
                  (do (try $l1
                        (do (try $l0
                              (do (try
                                    (do (throw $e0))
                                    (delegate $l1)))
                              (catch_all unreachable)))
                        (delegate $l3)))
                  (catch_all unreachable)))
            (catch_all (try
                         (do (throw $e0))
                         (delegate $l3))))
          unreachable)
      (catch_all (i32.const 1)))
  )

  ;; The same shape with the handlers made observable, discriminating an
  ;; actually-correct landing from one that merely traps and gets a fresh
  ;; exception rethrown. See e2e-try-delegate.wat for the full story.
  (func (export "delegate-targets-obs") (result i32)
    (try (result i32)
      (do (try $l3
            (do (try $l2
                  (do (try $l1
                        (do (try $l0
                              (do (try
                                    (do (throw $e0))
                                    (delegate $l1)))
                              (catch_all (return (i32.const 90)))))
                        (delegate $l3)))
                  (catch_all (return (i32.const 92)))))
            (catch_all (try
                         (do (throw $e0))
                         (delegate $l3))))
          unreachable)
      (catch_all (i32.const 1)))
  )

  ;; A delegation past TWO handlers at once. Three distinct answers: 6 when
  ;; the chain runs to the end, 5 when it stops after one region, 4 when the
  ;; depth is ignored altogether.
  (func (export "delegate-skip-two") (result i32)
    (try $t (result i32)
      (do
        (try (result i32)
          (do
            (try (result i32)
              (do
                (try (result i32)
                  (do (throw $e0) (i32.const 0))
                  (delegate $t)))
              (catch $e0 (i32.const 4))))
          (catch $e0 (i32.const 5))))
      (catch $e0 (i32.const 6))))

  ;; A block between the delegating try and its target; a block is not a
  ;; region, so the chain must pass it without spending a step.
  (func (export "delegate-past-block") (result i32)
    (try $t (result i32)
      (do
        (block $b (result i32)
          (try (result i32)
            (do
              (try (result i32)
                (do (throw $e0) (i32.const 0))
                (delegate $t)))
            (catch $e0 (i32.const 4)))))
      (catch $e0 (i32.const 7))))

  ;; A loop as the target label, which like a block has no region of its own,
  ;; so the throw lands in whatever encloses the loop.
  (func (export "delegate-to-loop") (result i32)
    (try (result i32)
      (do
        (loop $lp (result i32)
          (try (result i32)
            (do
              (try (result i32)
                (do (throw $e0) (i32.const 0))
                (delegate $lp)))
            (catch $e0 (i32.const 4)))))
      (catch_all (i32.const 8))))

  ;; A block immediately enclosing the delegating try, with the region to skip
  ;; beyond it.
  (func (export "delegate-block-first") (result i32)
    (try $t (result i32)
      (do
        (try (result i32)
          (do
            (block $b (result i32)
              (try (result i32)
                (do (throw $e0) (i32.const 0))
                (delegate $t))))
          (catch $e0 (i32.const 4))))
      (catch $e0 (i32.const 9))))

  (func $throw-void (throw $e0))

  ;; A `br` out of a delegating try's body. The branch leaves the region the
  ;; ordinary way; the delegation applies to what the body THROWS, and the
  ;; body here throws nothing.
  (func (export "break-try-delegate")
    (try (do (br 0)) (delegate 0))
  )

  (func (export "break-and-call-throw") (result i32)
    (try $outer (result i32)
      (do
        (try (result i32)
          (do
            (block $a
              (try (do (br $a)) (delegate $outer))
            )
            (call $throw-void)
            (i32.const 0)
          )
          (catch $e0 (i32.const 1))
        )
      )
      (catch $e0 (i32.const 2))
    )
  )

  (func (export "break-and-throw") (result i32)
    (try $outer (result i32)
      (do
        (try (result i32)
          (do
            (block $a
              (try (do (br $a)) (delegate $outer))
            )
            (throw $e0)
            (i32.const 0)
          )
          (catch $e0 (i32.const 1))
        )
      )
      (catch $e0 (i32.const 2))
    )
  )
)

;; CHECK: delegate-no-throw(): 1
;; CHECK-NEXT: delegate-throw(0): 1
;; CHECK-NEXT: delegate-throw(1): 2
;; CHECK-NEXT: delegate-throw-no-catch(): threw
;; CHECK-NEXT: delegate-merge(1,0): 2
;; CHECK-NEXT: delegate-merge(2,0): threw
;; CHECK-NEXT: delegate-merge(0,1): 2
;; CHECK-NEXT: delegate-merge(0,2): threw
;; CHECK-NEXT: delegate-merge(0,0): 1
;; CHECK-NEXT: delegate-skip(): 3
;; CHECK-NEXT: delegate-to-block(): 1
;; CHECK-NEXT: delegate-to-catch(): 1
;; CHECK-NEXT: delegate-to-caller-trivial(): threw
;; CHECK-NEXT: delegate-to-caller-skipping(): threw
;; CHECK-NEXT: delegate-correct-targets(): 1
;; CHECK-NEXT: delegate-targets-obs(): 1
;; CHECK-NEXT: delegate-skip-two(): 6
;; CHECK-NEXT: delegate-past-block(): 7
;; CHECK-NEXT: delegate-to-loop(): 8
;; CHECK-NEXT: delegate-block-first(): 9
;; CHECK-NEXT: break-try-delegate(): undefined
;; CHECK-NEXT: break-and-call-throw(): 1
;; CHECK-NEXT: break-and-throw(): 1
;; CHECK-NEXT: done
