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
// `e` is promoted once from a block that is a sibling of a SECOND block
// (`decl_A`, `Var`). That second block holds both `let e;` and the
// `try`/`catch` as sibling statements — `let e;` is declared BEFORE
// `try`, in the block enclosing the catch, not inside `catch (e)` itself;
// nesting it directly inside the catch would conflict, same-scope, with
// the catch's own `ES5Catch` decl for `e`. This outer `let e;` is still in
// scope for the PROMOTER's single upfront blocker scan of the whole
// `catch` subtree, so it correctly blocks the THIRD `e` candidate, one
// level deeper still (in an extra block nested inside `catch (e) { ... }`
// — the same trick `promotion-blocked-by-let.js`'s `inCatch()` needs for
// the opposite, ES5Catch-doesn't-block case). But the REAL resolver walks
// incrementally: by the time it reaches that same (still undeclared)
// candidate's identifier, the nearest binding it sees is the catch's OWN
// `ES5Catch` decl for `e` — the outer `let` has already been shadowed
// past by the time the resolver enters the catch body. So it matches
// `prevKind == ES5Catch && kind == ScopedFunction`, finds `e` already in
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
