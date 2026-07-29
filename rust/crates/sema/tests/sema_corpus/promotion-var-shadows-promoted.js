/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// S3 T2 battery: `visit(VariableDeclarationNode *)`'s
// `prevIsLexicalBindingOfPromotedFunc` special case (SemanticResolver.cpp:
// 365-374, and the error it feeds at cpp:391-401). This is a DIFFERENT
// check than the ordinary "already declared" one: it exists because a
// `var` nested in a block has been hoisted to the function's OWN scope by
// `DeclCollector`, so the general same-scope check
// (`validateAndDeclareIdentifier`, cpp:2513-2530) cannot see that a nested
// `var` and a nested lexical declaration are actually in conflict — this
// re-check walks the identifiers of a nested `var` explicitly.
//
// Trigger shape (top level, so a single block suffices — the same shape
// reproduces at function scope, see `nested`): `function g(){}` is a
// promotion candidate; `var g;` follows it in the SAME block. By the time
// `var g;`'s own statement is visited, the block's own decl-processing has
// ALREADY re-declared `g`'s identifier a second time (the "two
// declarations put" path, cpp:2611-2620), which shadowed the promoted decl
// at the block's OWN binding-table depth with a fresh `ScopedFunction` one
// (`bindingTable_.put`, not `try_emplace`). So when `var g`'s check runs,
// `bindingTable_.findWithDepth("g")` finds that `ScopedFunction` decl,
// whose `.scope` is the block (not the function body scope) — the first
// `continue`-guard (cpp:376-379) doesn't fire — and `ScopedFunction` is
// let-like (`Decl::isKindLetLike`, SemContext.h:144-146: `kind <=
// ES5Catch`), so the ordinary error condition (cpp:391-393) already fires
// too. `prevIsLexicalBindingOfPromotedFunc` is independently computed as
// `true` here (the name IS in `functionContext()->promotedFuncDecls`, and
// the depth found is NOT the function's `bindingTableScopeDepth`), so this
// pins that computation even though, in this particular shape, it doesn't
// change the outcome by itself.

{
  function g() {}
  var g;
}

function nested() {
  {
    function h() {}
    var h;
  }
}
