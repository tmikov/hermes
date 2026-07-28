/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// visit(SuperNode *, Node *) (SemanticResolver.cpp:1086-1092) tests
// isa<MemberExpressionLikeNode>(parent), a range that spans BOTH
// MemberExpression and OptionalMemberExpression (ESTree.def:360-373) — the
// latter reached only through `super.a?.b`.
//
// canReferenceSuper_ comes from FunctionLikeDecoration::isMethodDefinition
// (cpp:1675), which the parser sets for OBJECT-literal method shorthand too,
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
