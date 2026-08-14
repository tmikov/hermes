/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// S3 T1 seed: the NEGATIVE half of `promotion-basic.js`. Same two shapes
// (top level and inside a function), but a let-like declaration with the
// matching name is visible from the block, so `ScopedFunctionPromoter`
// refuses to promote (`processDeclarations`, cpp:223-235: the binding table
// has an entry for the name, so the candidate is dropped) and the function
// declaration keeps its block-local `ScopedFunction` decl.
//
// The `const`/`class`/`catch` variants pin the rest of
// `extractDeclaredIdents`'s kind mapping (cpp:238-306): every let-like kind
// blocks, EXCEPT `ES5Catch` — `catch (e)` with a plain identifier param is
// explicitly skipped (cpp:203-207, ES14.0 B.3.4), so `e` there does NOT
// block promotion and that function IS promoted.

let f;
{
  function f() {}
}

const c = 0;
{
  function c() {}
}

class k {}
{
  function k() {}
}

function outer() {
  let g;
  {
    function g() {}
  }
  return g;
}

function inCatch() {
  try {
  } catch (e) {
    {
      function e() {}
    }
  }
}
