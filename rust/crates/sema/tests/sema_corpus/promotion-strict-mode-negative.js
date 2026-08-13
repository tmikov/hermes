/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// S3 T2 battery: the strict-mode gate on BOTH promotion call sites
// (`visit(ProgramNode *)`, SemanticResolver.cpp:224-227, and
// `visitFunctionBodyAfterParamsVisited`, cpp:1935-1939 — both wrapped in
// `if (!curFunctionInfo()->strict)`). Annex B.3.3 is a sloppy-mode-only
// legacy compatibility hack; strict mode never promotes block-nested
// function declarations, so `f` keeps ONLY its block-scoped
// `ScopedFunction` decl and is never declared at function scope at all.
//
// `return typeof f;` makes that observable without also needing an
// "undefined variable" reference error: with no function-scope `f`
// declared, the reference resolves to an ambient, undeclared global
// property instead (contrast every promoted-case shape elsewhere in this
// battery, where the same kind of reference resolves to the promoted `Var`
// decl).

function strictNoPromotion() {
  "use strict";
  {
    function f() {}
  }
  return typeof f;
}
