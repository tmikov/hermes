/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// REWRITE #3: `$SHBuiltin.<prop>(...)` becomes a call whose callee's
// `_object` is an SHBuiltinNode (SemanticResolver.cpp:1153-1165). The dump
// shows it directly:
// the `Id '$SHBuiltin'` line the parser produced is replaced by a bare
// `SHBuiltin` line, with no `[D:E:...]` annotation — that is the whole
// observable effect of the rewrite.
//
// Preconditions, all three required (cpp:1155-1159): the callee is a
// MemberExpression, its `_object` is the Identifier `$SHBuiltin`, and the
// member is NOT computed. Then resolveIdentifier(ident, false) must return a
// decl of kind UndeclaredGlobalProperty (cpp:1161) — libhermes declares
// `var $SHBuiltin;`, so at global scope it always does (Decl %d.23 in the
// dump below).
//
// `moduleFactory`/`export`/`import` property names are the CommonJS-module
// protocol and are NOT exercised here: they are S4 (see xmod-errors.js's
// MANIFEST row and the loud panics in resolver/calls.rs).
//
// The non-rewriting shapes live in error-shbuiltin.js — every one of them is an
// error, because an `$SHBuiltin` identifier that survives to
// visit(IdentifierNode *) is reported as `invalid use of $SHBuiltin`
// (cpp:310-314).

$SHBuiltin.foo(1);
$SHBuiltin.bar();
$SHBuiltin.baz(1, 2, 3);

// The rewritten call is an ordinary expression afterwards.
var x = $SHBuiltin.foo(1);
var y = $SHBuiltin.foo($SHBuiltin.bar());
$SHBuiltin.foo(1).prop;
($SHBuiltin.foo(1))(2);

// Inside nested scopes: `$SHBuiltin` still resolves to the global ambient decl.
function f() {
  return $SHBuiltin.foo(1);
}
var arrow = () => $SHBuiltin.foo(2);
class C {
  m() { return $SHBuiltin.foo(3); }
  p = $SHBuiltin.foo(4);
  static { $SHBuiltin.foo(5); }
}

// A rewritten call whose argument ALSO folds, so the rebuilt CallExpression is
// rebuilt a second time by its own children walk.
$SHBuiltin.foo(1 + 2);
