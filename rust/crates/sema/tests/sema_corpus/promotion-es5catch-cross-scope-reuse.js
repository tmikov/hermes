/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// S3 T2 battery: the `ES5Catch, ScopedFunc` arm of
// `validateAndDeclareIdentifier`'s "when to create a new declaration"
// switch (SemanticResolver.cpp:2563-2578, specifically the
// `promotedFuncDecls` lookup at cpp:2569-2577). `ES5Catch` never blocks
// promotion (ScopedFunctionPromoter.cpp:212-216), so a `catch (e)` and a
// same-named promoted function can end up needing to share a decl exactly
// like the `Var, ScopedFunc` arm in `promotion-var-reuse.js` — this is the
// `ES5Catch` counterpart of that file's `crossScopeReuse`.
//
// `e` is promoted once from a block that is a sibling of `try`/`catch`
// (`decl_A`, `Var`). Inside `catch (e) { ... }`, `let e;` sits in an extra
// nested block (so its OWN declare doesn't same-scope-conflict with the
// catch param — same trick `promotion-blocked-by-let.js`'s `inCatch()`
// needs for the opposite, ES5Catch-doesn't-block case) and blocks a SECOND
// `e` candidate one level deeper. When that second candidate's
// (never-before-declared) identifier is resolved, the nearest binding is
// the catch's OWN `ES5Catch` decl — the `let` is further out and doesn't
// change that — so it matches `prevKind == ES5Catch && kind ==
// ScopedFunction`, finds `e` already in
// `functionContext()->promotedFuncDecls`, and reuses `decl_A` directly
// (`reuseDeclForNewBinding = true`), without ever creating its own
// block-scoped decl and without going through the "two declarations put"
// special case (cpp:2611-2620), because `getDeclarationDecl` was never set
// for this identifier in the first place.

function outer() {
  {
    function e() {}
  }
  {
    let e;
    try {
    } catch (e) {
      {
        function e() {}
      }
    }
  }
}
