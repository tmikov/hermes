/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// S3 T2 battery (review follow-up): `prevIsLexicalBindingOfPromotedFunc`
// (SemanticResolver.cpp:365-374) as the SOLE cause of the "already
// declared" error at cpp:391-393 — `promotion-var-shadows-promoted.js`'s
// shape pins the flag's computation, but its `prevKind` is `ScopedFunction`,
// which is independently let-like (`Decl::isKindLetLike`, `kind <=
// ES5Catch`), so the ordinary half of that `||` (cpp:392) already fires
// there too. This file isolates the flag as the ONLY thing that can fire
// the error: `prevKind == ES5Catch` is explicitly EXCLUDED from the
// ordinary check (`prevKind != Decl::Kind::ES5Catch` at cpp:392 — ES10.0
// B.3.5 normally allows `catch (e) { var e; }` to coexist, per
// `catch-shapes.js`), so with an `ES5Catch` previous decl, only
// `prevIsLexicalBindingOfPromotedFunc` being `true` can produce the error.
//
// `e` is promoted from a sibling block first
// (`functionContext()->promotedFuncDecls["e"]` populated). Inside
// `catch (e) { ... }`, a nested `var e;` triggers
// `visit(VariableDeclarationNode *)`'s special check (not at function body
// scope): `findWithDepth("e")` finds the catch's own `ES5Catch` decl,
// `prevDepth` is not `bindingTableScopeDepth`, and the name IS in
// `promotedFuncDecls` — so the flag is `true`, and since `prevKind ==
// ES5Catch` fails the ordinary check's `!= ES5Catch` guard, the flag is
// the ONLY reason the error fires. Delete the first, promoted `e` (or
// rename it) and this file compiles clean instead (the ordinary B.3.5
// `catch (e) { var e; }` exemption, same as `catch-shapes.js`) — confirming
// the flag alone flips the outcome.

function t() {
  {
    function e() {}
  }
  try {
  } catch (e) {
    {
      var e;
    }
  }
}
