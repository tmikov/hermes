/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// S3 T2 battery: the OTHER half of `extractDeclaredIdents`'s CatchClause
// mapping (ScopedFunctionPromoter.cpp:282-289): a DESTRUCTURING catch
// parameter (`catch ({ e })`) maps to `Decl::Kind::Catch`, not `ES5Catch`
// (that requires a plain `IdentifierNode` param, cpp:284-285). `Catch` IS
// let-like and is NOT the `ES5Catch` exemption (cpp:203-207: "only
// interested in let-like declarations, but not ES5Catch"), so it blocks
// promotion exactly like `let`/`const`/`class` do in
// `promotion-blocked-by-let.js` — contrast that file's `inCatch()`, where
// the plain-identifier `catch (e)` does NOT block.

function outer() {
  try {
  } catch ({ e }) {
    { function e() {} }
  }
}
