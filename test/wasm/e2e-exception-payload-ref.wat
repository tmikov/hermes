;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The exception-payload route into Wasm. A caught exception is a JS array;
;; the tag match looks only at element 0, and the handler then loads the
;; payload items straight onto the Wasm stack. Nothing forged is needed to
;; reach it: this module exports its tags, so JS can throw
;; [exports.t_fn, anyJSFunction] and the array the handler unpacks is one JS
;; wrote.
;;
;; The handlers below put the caught funcref in a local and the function
;; hands it back to JS. None of them puts it in a table or a global, so no
;; later funnel can refuse a bad value on the handler's behalf: whatever comes
;; out of `catch_fn` came off the Wasm stack.
;;
;; Each `catch $t...` handler bumps $ran once its payload is off the array, so
;; $ran distinguishes "the handler ran and delivered this" from "the handler
;; never ran and you are looking at the sentinel". The `catch_all` clauses
;; deliberately do not bump, which is what makes $ran 0 on a mismatch. The
;; funcref locals start at `ref.func $sentinel` for the same reason: `null` is
;; a legal payload, so a null result has to be distinguishable from an
;; untouched one, and the driver drives an unarmed `boom` once to show what an
;; untouched one looks like.
;;
;; `catch_fn_or_all` is what separates a refusal from a tag mismatch. A
;; mismatch continues down the handler chain into the `catch_all`, giving 2;
;; a refusal is a TypeError that leaves the function. The driver drives both,
;; so 2 is a value this test has seen produced.
;;
;; `catch_nested` asks the same question one level in, where the inner handler
;; chain sits inside the outer try's body: a mismatch is still 2, and anything
;; the outer `catch_all` sees is 3. A refusal answers 3, which pins a tradeoff
;; rather than a plainly right answer -- a module that wraps its work in
;; `catch_all` swallows the refusal instead of surfacing it to JS, the same
;; way it already swallows a Wasm trap (see onCatchAll's "catches everything
;; including traps" deviation). Changing that answer to "no handler can
;; intercept a refusal" is a compiler-wide change, not a local one; the
;; comment on emitBodyFuncRefCheck says what it would cost.
;;
;; The i64 rows are here for the two-index payload layout. An i64 occupies
;; array indices n and n+1, so in `(i64 funcref)` the funcref is at index 3,
;; and a handler that tested index 2 would refuse the high word of every
;; legitimate i64. $lo and $hi record the reassembled halves.
;;
;; The externref rows fail if a test ever appears on that arm: every JS value
;; is a valid externref.
;;
;; None of the `try` blocks here declares a result type. A `try` that declares
;; one and whose body FALLS THROUGH miscompiles on this branch
;; (dz 01a088d7-3c82), which is unrelated to payloads and is why the results
;; travel in locals.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s --enable-exceptions -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-exception-payload-ref-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  ;; A void import the driver uses to throw an array of its choosing from
  ;; inside a Wasm `try`.
  (import "e" "boom" (func $boom))

  (tag $t_fn (export "t_fn") (param funcref))
  (tag $t_i64fn (export "t_i64fn") (param i64 funcref))
  (tag $t_ext (export "t_ext") (param externref))
  (tag $t_i32 (export "t_i32") (param i32))

  ;; Two distinct exported functions, so the driver has two distinct
  ;; canonical wrappers to tell apart by identity, plus a third that the
  ;; handlers' funcref locals start out holding.
  (func $target (export "target") (result i32) (i32.const 1))
  (func $target2 (export "target2") (result i32) (i32.const 2))
  (func $sentinel (export "sentinel") (result i32) (i32.const 3))

  ;; How many handler bodies have run since the last reset, and the
  ;; reassembled halves of the last i64 payload unpacked.
  (global $ran (mut i32) (i32.const 0))
  (global $lo (mut i32) (i32.const 0))
  (global $hi (mut i32) (i32.const 0))
  (func (export "ran") (result i32) (global.get $ran))
  (func (export "lo") (result i32) (global.get $lo))
  (func (export "hi") (result i32) (global.get $hi))
  (func (export "reset")
    (global.set $ran (i32.const 0))
    (global.set $lo (i32.const 0))
    (global.set $hi (i32.const 0)))
  (func $bump
    (global.set $ran (i32.add (global.get $ran) (i32.const 1))))

  ;; A genuine Wasm throw, so the driver can read the array layout the engine
  ;; itself produces rather than assume one.
  (func (export "throw_i64fn")
    (throw $t_i64fn (i64.const 0x0000000700000005) (ref.func $target)))

  ;; Catch a funcref payload and hand it back to JS unchanged.
  (func (export "catch_fn") (result funcref)
    (local $f funcref)
    (local.set $f (ref.func $sentinel))
    (try
      (do
        (call $boom))
      (catch $t_fn
        (local.set $f)
        (call $bump)))
    (local.get $f))

  ;; The mixed payload. The funcref is payload item 1, at array index 3.
  (func (export "catch_i64fn") (result funcref)
    (local $f funcref)
    (local $n i64)
    (local.set $f (ref.func $sentinel))
    (try
      (do
        (call $boom))
      (catch $t_i64fn
        (local.set $f)
        (local.set $n)
        (call $bump)
        (global.set $lo (i32.wrap_i64 (local.get $n)))
        (global.set $hi
          (i32.wrap_i64 (i64.shr_u (local.get $n) (i64.const 32))))))
    (local.get $f))

  ;; An externref payload: whatever JS put there comes back.
  (func (export "catch_ext") (result externref)
    (local $x externref)
    (try
      (do
        (call $boom))
      (catch $t_ext
        (local.set $x)
        (call $bump)))
    (local.get $x))

  ;; A refusal must not read as a mismatch. 1 = the funcref handler ran,
  ;; 2 = the tag did not match and the catch_all ran, 0 = nothing threw.
  (func (export "catch_fn_or_all") (result i32)
    (local $r i32)
    (try
      (do
        (call $boom))
      (catch $t_fn
        (drop)
        (call $bump)
        (local.set $r (i32.const 1)))
      (catch_all
        (local.set $r (i32.const 2))))
    (local.get $r))

  ;; The same question with an enclosing try. 3 = the outer catch_all saw it.
  (func (export "catch_nested") (result i32)
    (local $r i32)
    (try
      (do
        (try
          (do
            (call $boom))
          (catch $t_fn
            (drop)
            (call $bump)
            (local.set $r (i32.const 1)))
          (catch_all
            (local.set $r (i32.const 2)))))
      (catch_all
        (local.set $r (i32.const 3))))
    (local.get $r)))

;; The brand probe's two verdicts first. It reaches the brand through the same
;; isWasmExportedFunction the generated check does -- see the driver comment
;; for why that sharing is deliberate -- so it cannot vouch for the refusal
;; rows below. What these two rows establish is that `plain` and `E.target`
;; sit on opposite sides of that one notion; the independent oracle for the
;; refusal rows is E.ran(), which shares nothing with the brand.
;; CHECK: oracle refuses a plain JS function: not an Exported Function
;; CHECK-NEXT: oracle accepts target: wrapper
;; CHECK-NEXT: target and target2 are different objects: true
;;
;; The array layout, read off a genuine Wasm throw: the i64 took indices 1
;; and 2, so the funcref is at index 3.
;; CHECK-NEXT: genuine is an array of length: true 4
;; CHECK-NEXT: genuine[0] is the tag: true
;; CHECK-NEXT: genuine i64 halves: 5 7
;; CHECK-NEXT: genuine[3] is the target wrapper: true
;;
;; --- funcref payloads delivered ---
;; The sentinel row first: it is what a result that no handler produced looks
;; like, so the rows under it are not comparing against a local that already
;; held the answer.
;; CHECK-NEXT: catch_fn(nothing thrown): true 0
;; CHECK-NEXT: catch_fn(target): true 1
;; CHECK-NEXT: catch_fn(target2): true 1
;; CHECK-NEXT: catch_fn(null): null true 1
;;
;; --- funcref payloads refused, with the handler body never entered ---
;; CHECK-NEXT: catch_fn(plain function): TypeError: Wasm catch: funcref payload 0 requires null or a WebAssembly exported function 0
;; CHECK-NEXT: catch_fn(5): TypeError: Wasm catch: funcref payload 0 requires null or a WebAssembly exported function 0
;; CHECK-NEXT: catch_fn(undefined): TypeError: Wasm catch: funcref payload 0 requires null or a WebAssembly exported function 0
;; CHECK-NEXT: catch_fn({}): TypeError: Wasm catch: funcref payload 0 requires null or a WebAssembly exported function 0
;; CHECK-NEXT: catch_fn(target again): true 1
;;
;; --- a refusal is a TypeError, not a tag mismatch ---
;; 1 is the funcref handler, 2 is the catch_all a mismatch reaches. The
;; refusal row is neither: it leaves the function.
;; CHECK-NEXT: catch_fn_or_all(target): 1 1
;; CHECK-NEXT: catch_fn_or_all(a different tag): 2 0
;; CHECK-NEXT: catch_fn_or_all(plain function): TypeError: Wasm catch: funcref payload 0 requires null or a WebAssembly exported function 0
;;
;; With an enclosing try the refusal is an exception the outer catch_all
;; sees -- 3, still not the mismatch answer of 2.
;; CHECK-NEXT: catch_nested(target): 1 1
;; CHECK-NEXT: catch_nested(a different tag): 2 0
;; CHECK-NEXT: catch_nested(plain function): 3 0
;;
;; --- the mixed (i64 funcref) payload ---
;; CHECK-NEXT: catch_i64fn(5, 7, target): true 5 7 1
;; CHECK-NEXT: catch_i64fn(5, 7, plain function): TypeError: Wasm catch: funcref payload 1 requires null or a WebAssembly exported function 0 0 0
;; CHECK-NEXT: catch_i64fn(genuine, funcref replaced): TypeError: Wasm catch: funcref payload 1 requires null or a WebAssembly exported function 0 0 0
;; CHECK-NEXT: catch_i64fn(genuine, funcref target2): true 5 7 1
;;
;; --- accessor-backed items: one read, and that read is what was pushed ---
;; The first row is the whole point: two reads would deliver target2 and
;; report 2.
;; CHECK-NEXT: accessor(target, target2): true false reads 1 1
;; CHECK-NEXT: accessor(target, plain function): true reads 1 1
;; CHECK-NEXT: accessor(plain function, target): TypeError: Wasm catch: funcref payload 0 requires null or a WebAssembly exported function reads 1 0
;; CHECK-NEXT: accessor i64(target, target2): true reads 1 5 7 1
;; CHECK-NEXT: accessor i64(plain function, target): TypeError: Wasm catch: funcref payload 1 requires null or a WebAssembly exported function reads 1 0 0 0
;;
;; --- externref payloads: every JS value, unchanged, read once ---
;; CHECK-NEXT: catch_ext(plain function): true 1
;; CHECK-NEXT: catch_ext(target): true 1
;; CHECK-NEXT: catch_ext(5): true 1
;; CHECK-NEXT: catch_ext({}): true 1
;; CHECK-NEXT: catch_ext(undefined): true 1
;; CHECK-NEXT: catch_ext(null): true 1
;; CHECK-NEXT: accessor ext({}, plain function): true reads 1 1
;; CHECK-NEXT: done
