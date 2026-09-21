;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A Wasm `try` must survive catching something that is not a Wasm exception.
;;
;; A Wasm exception travels as a JS Array whose element 0 is the tag object,
;; and wasmMatchException decides whether a caught value is one. It cast the
;; caught object with vmcast_or_null<JSArray>, which ASSERTS the type rather
;; than testing it -- so the `if (!obj)` line under it was dead for every
;; non-null object, and its own comment, "Check if caught is a JSArray", was
;; not what the code did.
;;
;; Anything a JS import throws lands there. `throw {}` was an assertion failure
;; in a Debug build and, in a Release build, a JSArray pointer to something
;; that is not a JSArray, whose length and elements were then read. Reaching it
;; needs no invalid module, no forged value and no unusual flag: an import that
;; throws an ordinary object is enough.
;;
;; dyn_vmcast_or_null tests instead, which is what the line below it always
;; expected, and a non-matching value is rethrown to JS unchanged.

;; REQUIRES: wasm
;; RUN: %wat2wasm --enable-exceptions %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-catch-non-array-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (import "e" "boom" (func $boom))
  (tag $t (export "t") (param i32))
  (global $caught (mut i32) (i32.const -1))
  (func (export "run") (result i32)
    (try
      (do (call $boom))
      (catch $t (global.set $caught)))
    (global.get $caught)))

;; Each of these is thrown by the import from inside the try, and each must
;; pass through the handler untouched and reach the caller as itself. The
;; first four are simply not Wasm exceptions.
;; CHECK: plain object: escaped as the same object
;; CHECK-NEXT: array with a non-tag at 0: escaped as the same object
;; CHECK-NEXT: empty array: escaped as the same object
;; CHECK-NEXT: a string: escaped as the same value

;; The fifth is different and is kept for that reason. A
;; WebAssembly.Exception built on THIS module's own tag is a Wasm exception by
;; every meaning of the words -- it carries that tag and a payload -- and it
;; escapes anyway, because wasmMatchException knows only the [tag, ...payload]
;; array representation and this one is a JSWebAssemblyException cell. That is
;; an interoperability gap, filed as 01a09db0-a02b, not the correct treatment
;; of a foreign value. What this file pins is that it escapes rather than
;; crashing.
;; CHECK-NEXT: a WebAssembly.Exception: escaped as the same object

;; And the handler still catches what it should, so the refusals above are not
;; the handler having stopped working.
;; CHECK-NEXT: the module's own tag: caught, payload 99
;; CHECK-NEXT: done
