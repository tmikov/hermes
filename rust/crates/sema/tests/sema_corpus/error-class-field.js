/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The three flags visit(ClassPropertyNode *) sets while resolving a field
// initializer (SemanticResolver.cpp:1027-1033):
//
//  - forbidSpecialArgumentsReference_ = true (ES14.0 15.7.1: it is a Syntax
//    Error if ContainsArguments of the initializer is true) — the
//    `invalid use of 'arguments'` error, which fires through arrows too
//    because they inherit the flag;
//  - forbidAwaitExpression_ = true — `'await' not in an async function`, even
//    inside an `async` function, because the initializer runs in its own
//    synthesized function;
//  - canReferenceSuper_ = true, which is why `super.x` is NOT an error here
//    (covered by classes-derived.js).
//
// Both fire for static and instance fields alike, and the declareArguments()
// call at cpp:1039 is what makes the `arguments` case an error rather than an
// unresolved global at top level.

class AtGlobal {
  a = arguments;
  static b = arguments;
  c = () => arguments;
  d = [arguments, arguments];
}

function inFunction(p) {
  class C {
    a = arguments;
    b = () => () => arguments;
  }
  return C;
}

async function inAsync() {
  class C {
    a = await 1;
    static b = await 2;
  }
  return C;
}

// A computed key is resolved OUTSIDE the synthesized function, so `await`
// there is an error only because the enclosing context forbids it — at global
// scope it is a plain reference instead. Inside an async function the key is
// in the async function's own context, so `await` is allowed there.
async function keyAwait() {
  class C {
    [await 1] = 2;
  }
  return C;
}
