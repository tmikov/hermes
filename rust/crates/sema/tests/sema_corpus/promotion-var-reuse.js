/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// S3 T2 battery: the `Var, ScopedFunc` arm of
// `validateAndDeclareIdentifier`'s "when to create a new declaration"
// switch (SemanticResolver.cpp:2546-2562). `var` never blocks promotion
// (cpp:206's `!isKindLetLike(Var)`), so wherever a `var` and a promoted
// function share a name, they end up sharing ONE `Decl` rather than
// conflicting.
//
// `funcThenVar` / `varThenFunc`: the ordinary, common-case shapes — a
// block-nested function and a function-scope `var` with the same name, in
// both source orders. Both `Id` occurrences (the `var`'s and the
// `return`'s) resolve to the SAME decl the promoted function got.
//
// `crossScopeReuse`: the genuinely cross-scope branch
// (cpp:2554-2561, `reuseDeclForNewBinding`). `h`'s first block is promoted
// normally. In `h`'s second (sibling) block, `function h(){}` is declared
// BEFORE `let h;` in source order, so at the moment the function's OWN
// (never-before-declared) identifier is resolved, the binding table has
// nothing local yet — it finds the FIRST block's promoted `Var` decl
// directly (not shadowed by the not-yet-processed `let`), matches
// `Decl::isKindVarLike(prevKind) && kind == ScopedFunction`, sees `!sameScope`
// (the `Var` decl's home is the function scope, not this block), finds `h`
// already in `functionContext()->promotedFuncDecls`, and reuses that SAME
// decl (`reuseDeclForNewBinding = true`) WITHOUT ever creating a second,
// block-scoped `ScopedFunction` decl for it (contrast the ordinary
// "two declarations" shapes in `promotion-basic.js`, which always mint one).
// `getDeclarationDecl(ident)` is therefore never pre-set for this
// identifier, so `visit(VariableDeclarationNode)`'s "two declarations put"
// special case (cpp:2611-2620) is correctly skipped. The immediately
// following `let h;` then dumps as `[D:<its own Let decl> E:<the reused Var
// decl>]`, because its own `try_emplace` at the block's binding-table depth
// finds `h` already bound there (to the reused `Var` decl) and is a no-op
// per `PersistentScopedMap::tryEmplaceIntoScope`.

function funcThenVar() {
  {
    function f() {}
  }
  var f;
  return f;
}

function varThenFunc() {
  var g;
  {
    function g() {}
  }
  return g;
}

function crossScopeReuse() {
  {
    function h() {}
  }
  {
    function h() {}
    let h;
  }
  return h;
}
