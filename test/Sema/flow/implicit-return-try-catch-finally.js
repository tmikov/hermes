/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: (! %shermes -Werror -ferror-limit=0 -typed -dump-sema %s 2>&1 ) | %FileCheckOrRegen --match-full-lines %s

(function main(x) {

// Pins the answers CheckImplicitReturn gives for a try statement that has both
// a handler and a finalizer. A function is reported here exactly when its
// implicit 'return undefined' is reachable, which is what
// mayReachImplicitReturn computes.
//
// The compiler splits "try B catch H finally F" into nested try statements
// before this check runs; the parser entry point does not, and computes the
// same answers by composing the two cases in that same order. The parser-mode
// side is covered by ResolverTest.TryCatchFinallyImplicitReturnTest, which
// asserts exactly the outcomes below.

// Reachable: nothing here terminates.
function fallsThrough(): number {
  try { x(); } catch { x(); } finally { x(); }
}

// Reachable: the try returns but the handler completes normally.
function catchFallsThrough(): number {
  try { return 1; } catch { x(); } finally { x(); }
}

// Not reachable: try and catch both return and the finalizer completes
// normally, so the finalizer cannot redirect control anywhere.
function bothReturn(): number {
  try { return 1; } catch { return 2; } finally { x(); }
}

// Not reachable: a 'return' in the finalizer overrides however the protected
// part completed, including the handler falling through.
function finallyReturns(): number {
  try { x(); } catch { x(); } finally { return 1; }
}

// Reachable: 'break lbl' in the finalizer continues after the labeled block,
// so the end of the function is reachable even though try and catch both
// return.
function finallyBreaks(): number {
  lbl: {
    try { return 1; } catch { return 2; } finally { break lbl; }
  }
}

})();

// Auto-generated content below. Please do not modify manually.

// CHECK:{{.*}}implicit-return-try-catch-finally.js:24:26: error: ft: implicitly-returned 'undefined' incompatible with return type: number
// CHECK-NEXT:function fallsThrough(): number {
// CHECK-NEXT:                         ^~~~~~
// CHECK-NEXT:{{.*}}implicit-return-try-catch-finally.js:29:31: error: ft: implicitly-returned 'undefined' incompatible with return type: number
// CHECK-NEXT:function catchFallsThrough(): number {
// CHECK-NEXT:                              ^~~~~~
// CHECK-NEXT:{{.*}}implicit-return-try-catch-finally.js:48:27: error: ft: implicitly-returned 'undefined' incompatible with return type: number
// CHECK-NEXT:function finallyBreaks(): number {
// CHECK-NEXT:                          ^~~~~~
// CHECK-NEXT:Emitted 3 errors. exiting.
