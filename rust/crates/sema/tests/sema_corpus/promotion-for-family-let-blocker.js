/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// S3 final-review follow-up: the `ForStatement`/`ForInStatement`/
// `ForOfStatement` arms of `ScopedFunctionPromoter::visit`
// (ScopedFunctionPromoter.cpp:53-61), each a thin `visitScope(node)`
// forward exactly like `BlockStatementNode`'s (cpp:50-52). A
// `FunctionDeclaration` can never be a bare loop body (the grammar only
// allows a `Statement` there, and a bare function declaration is not one),
// so these three arms are only observable when a `let`-like declaration in
// the loop HEAD (its own scope, opened by `visitScope`'s
// `BindingTableScopeTy bindingScope{bindingTable_}` and populated by
// `processDeclarations(node)`, cpp:141-145) blocks a promotion candidate
// declared in the loop's BODY block. A port that dropped all three arms
// (falling back to the default `visit(Node *)`, which still recurses into
// children via `visitESTreeChildren` but never calls `processDeclarations`
// on the loop's own scope) would still pass every other corpus file, since
// the head's `let` would simply never reach `bindingTable_` and the
// candidate would be wrongly promoted.
//
// One function per arm, same shape: `let <name>` in the loop head, a
// same-named `function <name>() {}` as the sole statement in the loop
// body block. Verified with hermesc: exit 0 for all three; each inner
// function's own `Id` resolves to a block-scoped `ScopedFunction` decl
// (not promoted to `Var`/`GlobalProperty`) — byte-identical to the Rust
// port.

function forHead() {
  for (let f = 0; ; ) {
    function f() {}
  }
}

function forOfHead() {
  for (let g of []) {
    function g() {}
  }
}

function forInHead() {
  for (let h in {}) {
    function h() {}
  }
}
