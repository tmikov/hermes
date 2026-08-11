/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The legal half of the super() check in visit(CallExpressionNode *)
// (SemanticResolver.cpp:1209-1216): a `Super` callee is allowed exactly when
// semCtx_.nearestNonArrow(functionContext()->semInfo)->constructorKind is
// Derived, which visit(MethodDefinitionNode *) sets (cpp:1666-1676) for the
// `constructor` of a class WITH an `extends` clause.
//
// `nearestNonArrow` (SemContext.cpp:82-93) is what makes super() legal through
// any depth of arrow function: arrows inherit the enclosing constructor's kind
// by being skipped, not by copying it.
//
// Note the Super node itself is NOT reported by visit(SuperNode *, Node *)
// (cpp:1096-1102): that only fires for a MemberExpressionLike parent, and a
// CallExpression parent is not one — which is why super() needs its own check
// here at all.

class Base {}

class Derived extends Base {
  constructor() {
    super();
  }
}

class DerivedArgs extends Base {
  constructor(a, b) {
    super(a, b, 1 + 2);
  }
}

class DerivedSpread extends Base {
  constructor() {
    var args = [1];
    super(...args);
  }
}

// Nested statement positions inside the constructor: the check reads the
// FunctionContext, not the statement nesting.
class DerivedNested extends Base {
  constructor() {
    {
      if (1) {
        super();
      }
    }
    for (var i = 0; i < 0; ++i) super();
    try { super(); } catch (e) { super(); }
    switch (1) { case 1: super(); }
  }
}

// Through arrows, at several depths, and inside an arrow's parameter default.
class DerivedArrow extends Base {
  constructor() {
    var f = () => { super(); };
    var g = () => () => super();
    var h = (p = super()) => p;
    f(); g(); h();
  }
}

// A derived class expression, and a derived class nested in a function.
var E = class extends Base {
  constructor() { super(); }
};
function outer() {
  class F extends Base {
    constructor() { super(); }
  }
  return F;
}

// `extends` of an arbitrary expression still yields ConstructorKind::Derived.
class G extends (Base) {
  constructor() { super(); }
}
