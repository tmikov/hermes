/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// visit(SuperNode *, Node *) (SemanticResolver.cpp:1096-1102) tests
// isa<MemberExpressionLikeNode>(parent), a range that spans MemberExpression
// and OptionalMemberExpression (ESTree.def:360-373). Only the
// MemberExpression half is reachable, in C++ as much as here: the parser
// requires '(', '[' or '.' immediately after `super` (so `super?.a` is a parse
// error), and in `super.a?.b` the OptionalMemberExpression wraps a plain
// MemberExpression whose object is the Super — so a Super's parent is never an
// OptionalMemberExpression. `super.a?.b` is kept below because it is the
// closest thing the grammar allows, not because it reaches the other half.
//
// canReferenceSuper_ comes from FunctionLikeDecoration::isMethodDefinition
// (cpp:1689), which the parser sets for OBJECT-literal method shorthand too,
// not just class methods — so none of these is an error. The error shapes all
// live in reject-super-references.js.

var o = {
  m() {
    return super.x;
  },
  get g() {
    return super.y;
  },
  set s(v) {
    super.y = v;
  },
  ['computed']() {
    return super.z;
  },
  *gen() {
    return super.w;
  },
  async am() {
    return super.v;
  },
  arrowInMethod() {
    return () => super.u;
  },
};

class C {
  m() {
    return super.a?.b;
  }
}

var q = {
  m() {
    return super.a?.b;
  },
};
