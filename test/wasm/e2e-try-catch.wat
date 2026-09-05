;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s --enable-exceptions -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-try-catch-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "env" "print" (func $print (param i32)))

  (tag $tag_i32 (param i32))
  (tag $tag_void)

  ;; Test 1: throw and catch with matching tag
  (func $test_catch_match (export "test_catch_match")
    (try
      (do
        (throw $tag_i32 (i32.const 42))
      )
      (catch $tag_i32
        ;; Caught i32 value (42) is on the stack
        (call $print)
      )
    )
  )

  ;; Test 2: throw and catch_all
  (func $test_catch_all (export "test_catch_all")
    (try
      (do
        (throw $tag_void)
      )
      (catch_all
        (call $print (i32.const 99))
      )
    )
  )

  ;; Test 3: throw with wrong tag, caught by catch_all
  (func $test_wrong_tag_catchall (export "test_wrong_tag_catchall")
    (try
      (do
        (throw $tag_void)
      )
      (catch $tag_i32
        ;; Should not match; drop the caught value
        (drop)
        (call $print (i32.const -1))
      )
      (catch_all
        (call $print (i32.const 77))
      )
    )
  )

  ;; Test 4: try/catch with result value
  (func $test_catch_result (export "test_catch_result") (result i32)
    (try (result i32)
      (do
        (throw $tag_i32 (i32.const 55))
        (i32.const 0)
      )
      (catch $tag_i32
        ;; The caught value (55) becomes the block result
      )
    )
  )

  ;; Test 5: rethrow
  (func $test_rethrow (export "test_rethrow")
    (try
      (do
        (try
          (do
            (throw $tag_i32 (i32.const 88))
          )
          (catch_all
            (rethrow 0)
          )
        )
      )
      (catch $tag_i32
        ;; Caught the re-thrown value
        (call $print)
      )
    )
  )

  ;; Entry point
  (func $main (export "_main")
    (call $test_catch_match)
    (call $test_catch_all)
    (call $test_wrong_tag_catchall)
    (call $print (call $test_catch_result))
    (call $test_rethrow)
  )
)

;; CHECK: 42
;; CHECK-NEXT: 99
;; CHECK-NEXT: 77
;; CHECK-NEXT: 55
;; CHECK-NEXT: 88
