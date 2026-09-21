;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A block, loop, if or try with an i64 RESULT, entered in unreachable code.
;;
;; onEnd carried three identical copies of the same loop -- one for Try, one
;; for Block/If, one for Loop -- that ran when the construct had been entered
;; in unreachable code:
;;
;;   for (auto t : entry.resultTypes) {
;;     if (t == WasmValType::I64) {
;;       push(builder_.getLiteralUndefined());
;;       push(builder_.getLiteralUndefined());
;;       valueStackIsI64Hi_.back() = true;   // <-- nothing of ours is there
;;     } else {
;;       push(builder_.getLiteralUndefined());
;;     }
;;   }
;;
;; push() is a no-op while unreachable_, which it always is on these paths, so
;; neither push put anything on the stack and back() reached whatever the
;; resize above had left. Both outcomes are here:
;;
;;   - nothing left, and back() is an invalid access on an empty vector.
;;     std::vector<bool> can keep its storage after shrinking, so what this
;;     actually reported was a heap-buffer-overflow at WasmIRGen.cpp:4245 in
;;     onEnd() rather than a null dereference.
;;   - something left, and the mark lands on a slot belonging to an ENCLOSING
;;     block, telling later code that an unrelated i32 is an i64 high word.
;;     `stale` below is that: the marked slot reaches a `select`, which
;;     consults the mark (isTopI64) to decide how many slots to pop, takes the
;;     i64 path on a pair of i32s and runs the stack empty.
;;
;; The three copies are now one helper, pushUndefinedResults, which marks the
;; slot only when the push actually happened.

;; REQUIRES: wasm
;; RUN: %wat2wasm --enable-exceptions %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-unreachable-i64-result-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (tag $t)

  ;; In each of these the `br` makes everything after it unreachable, so the
  ;; construct that follows is entered with unreachable_ set and no IR is
  ;; generated for it. The value the function returns comes from before the
  ;; br: what is being tested is that the dead code COMPILES, and that the
  ;; live code around it is undisturbed.

  (func (export "tryI64") (result i32)
    (block $out (result i32)
      (i32.const 42)
      (br $out)
      (try (result i64) (do (unreachable)) (catch_all (unreachable)))
      (drop)
      (i32.const 0)))

  (func (export "blockI64") (result i32)
    (block $out (result i32)
      (i32.const 43)
      (br $out)
      (block (result i64) (unreachable))
      (drop)
      (i32.const 0)))

  (func (export "loopI64") (result i32)
    (block $out (result i32)
      (i32.const 44)
      (br $out)
      (loop (result i64) (unreachable))
      (drop)
      (i32.const 0)))

  ;; If shares the Block copy, but reaches it as a different ControlEntry
  ;; kind.
  (func (export "ifI64") (result i32)
    (block $out (result i32)
      (i32.const 45)
      (br $out)
      (if (result i64) (i32.const 1) (then (unreachable)) (else (unreachable)))
      (drop)
      (i32.const 0)))

  ;; A mixed result list. This does not test where the marker lands -- nothing
  ;; is pushed here at all -- so for this defect it is the Block case again.
  ;; It is kept for the signature decoding: a result list of more than one
  ;; type, reaching the same path.
  (func (export "mixedI64") (result i32)
    (block $out (result i32)
      (i32.const 46)
      (br $out)
      (block (result i32 i64) (unreachable))
      (drop)
      (drop)
      (i32.const 0)))

  ;; The non-crashing half. 7 and 8 are live on the stack beneath the dead
  ;; block, so the stray mark lands on the slot holding 8. `select` then reads
  ;; that mark to decide whether it is choosing between two i64s or two i32s,
  ;; takes the i64 path on a pair of i32s, and pops twice as many slots as
  ;; exist.
  ;;
  ;; select(7, 8) with a true condition is 7; nothing about the answer depends
  ;; on the dead block, which is the point.
  (func (export "stale") (param i32) (result i32)
    (i32.const 7)
    (i32.const 8)
    (block $out
      (br $out)
      (block (result i64) (unreachable))
      (drop))
    (local.get 0)
    (select))

  ;; The two below are REACHABLE, so the pushes land and the marker is placed
  ;; for real. Everything above runs with unreachable_ set, where the helper
  ;; pushes nothing and the guard is the whole of its behaviour -- so nothing
  ;; above says the marker goes in the right place when it does go somewhere.

  ;; A mixed result with the i64 FIRST, so its high half is not the top slot
  ;; when the results are consumed. The answer depends on the i64's high word
  ;; and on the i32, so the two cannot be confused for each other.
  (func (export "liveMixed") (result i32) (local $a i64) (local $b i32)
    (block (result i64 i32)
      (i64.const 4294967298) ;; hi 1, lo 2
      (i32.const 5))
    (local.set $b)
    (local.set $a)
    (i32.add
      (i32.wrap_i64 (i64.shr_u (local.get $a) (i64.const 32)))
      (i32.mul (local.get $b) (i32.const 10))))

  ;; An `if` with an i64 PARAM whose then arm ends in `return`. onElse then
  ;; runs with unreachable_ set and re-pushes the saved param for the else
  ;; arm, marking its high slot -- which works only because onElse resets
  ;; unreachable_ BEFORE the re-push rather than after, since push() is a
  ;; no-op the other way round. Nothing else here covers that ordering.
  (func (export "ifParam") (param i32) (result i32)
    i64.const 4294967298
    local.get 0
    if (param i64) (result i32)
      drop
      i32.const 9
      return
    else
      i64.const 32
      i64.shr_u
      i32.wrap_i64
    end))

;; CHECK: tryI64(): 42
;; CHECK-NEXT: blockI64(): 43
;; CHECK-NEXT: loopI64(): 44
;; CHECK-NEXT: ifI64(): 45
;; CHECK-NEXT: mixedI64(): 46
;; CHECK-NEXT: stale(1): 7
;; CHECK-NEXT: stale(0): 8
;; CHECK-NEXT: liveMixed(): 51
;; CHECK-NEXT: ifParam(1): 9
;; CHECK-NEXT: ifParam(0): 1
;; CHECK-NEXT: done
