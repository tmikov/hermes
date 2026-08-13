/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// S3 T1 seed: `ScopedFunctionPromoter` (lib/Sema/ScopedFunctionPromoter.cpp)
// applied at BOTH of the call sites this task wires up.
//
// - The top-level `{ function f() {} }` goes through `visit(ProgramNode *)`
//   (SemanticResolver.cpp:224-227), so the promoted decl kind is
//   `GlobalProperty` (`processPromotedFuncDecls`, cpp:2162-2163 —
//   `functionContext()->isGlobalScope()`).
// - The one inside `outer` goes through
//   `visitFunctionBodyAfterParamsVisited` (cpp:1933-1939), so the promoted
//   decl kind is `Var`.
//
// Both promote: nothing let-like with the same name is visible in the
// declaring scope, so Annex B 3.3's "replace with var" is safe. The dump
// shows each name in the FUNCTION scope (%s.1 / the `outer` body scope)
// rather than in the block's own scope, which is the whole point of the
// pass; the second, non-promoted `ScopedFunction` decl for the same
// identifier stays in the block scope, and the identifier itself carries
// BOTH (`[D:E:... D:D:...]`) via `SemContext::promotedFunctionDecls_`.
//
// A third shape pins the parameter rule (`processParameters`, cpp:147-158 /
// ES2022 B.3.2.1 29.a.ii): a formal parameter with the same name blocks
// promotion even though a parameter is not a let-like declaration.
//
// `scopes` covers four more of the seven scope-bearing kinds `visitScope`
// handles (cpp:47-67) — `Switch`, `For`, `ForIn`, `ForOf`; `Block` is
// everywhere above and `CatchClause` is in `promotion-blocked-by-let.js`
// (the seventh, `With`, cannot appear: `visit(WithStatementNode *)` reports
// "with statement is not supported" and hermesc exits before dumping).
// `twice` pins that two candidates with the SAME name both promote and that
// the `promotedFuncDecls` map keeps the FIRST one (`try_emplace`, cpp:2168).

{
  function f() {}
}
f();

function outer() {
  {
    function g() {}
  }
  return g;
}

function withParam(h) {
  {
    function h() {}
  }
  return h;
}

function scopes() {
  switch (0) {
    case 1:
      function a() {}
  }
  for (;;) {
    function b() {}
  }
  for (var i in {}) {
    function c() {}
  }
  for (var j of []) {
    function d() {}
  }
  return [a, b, c, d];
}

function twice() {
  {
    function e() {}
  }
  {
    function e() {}
  }
  return e;
}
