/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Every shape rewrite #3 does NOT rewrite. In each case the `$SHBuiltin`
// Identifier survives into the children walk, reaches
// visit(IdentifierNode *, Node *) and is reported there as
// `invalid use of $SHBuiltin` (SemanticResolver.cpp:310-314) — the rewrite does
// not SUPPRESS that error, it merely removes the node before the walk can reach
// it.
//
// Note there is exactly ONE error per surviving `$SHBuiltin`, including in the
// one shape below that visit(CallExpressionNode *) DID call resolveIdentifier
// on (the shadowed `let $SHBuiltin` — the only one whose callee is a
// non-computed MemberExpression, cpp:1160): the second call hits the decl cache
// in checkIdentifierResolved, and the error only ever comes from the identifier
// visit.

// Not a call at all.
$SHBuiltin;
$SHBuiltin.foo;
var a = $SHBuiltin;

// A call, but the callee is not a MemberExpression (cpp:1155-1156).
$SHBuiltin(1);

// A member call, but COMPUTED (cpp:1159).
$SHBuiltin["foo"](1);
$SHBuiltin[a](1);

// A member call where `$SHBuiltin` is the PROPERTY, not the object: the ONLY
// line in this file that is legal, because visit(IdentifierNode *) returns
// early for a non-computed member property (cpp:287-293) before it can reach
// the $SHBuiltin check.
a.$SHBuiltin(1);

// An OptionalCallExpression: no visit override, so the specials never run.
$SHBuiltin.foo?.(1);
$SHBuiltin?.foo(1);

// A NewExpression: likewise.
new $SHBuiltin.foo(1);

// `$SHBuiltin` shadowed by a local declaration, so resolveIdentifier returns a
// Let decl rather than an UndeclaredGlobalProperty one (cpp:1161) — the
// declaration itself is a second `invalid use of $SHBuiltin`, since the
// identifier visit sees every occurrence.
function shadowed() {
  let $SHBuiltin = {};
  $SHBuiltin.foo(1);
}
