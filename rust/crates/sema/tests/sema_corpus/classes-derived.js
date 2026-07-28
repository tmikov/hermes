/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Derived classes: the superclass expression is visited BEFORE private names
// are declared but AFTER the class name (SemanticResolver.cpp:936-939), so it
// sees the inner ClassExprName binding. isDerivedClass() (SemanticResolver.h:
// 659-662) is what makes an implicit or explicit constructor `Derived`
// (cpp:1657-1661, cpp:3097-3099) — invisible in the dump, unit-tested
// separately, but the shapes are covered here.
//
// `super.x` member access is allowed wherever canReferenceSuper_ is set:
// inside a method (because the FunctionExpression is a method definition,
// cpp:1675), inside an arrow nested in a method (arrows inherit it), and in a
// field initializer (cpp:1027). super() CALLS are not here — the call check
// lives in visit(CallExpressionNode *).

class Base {
  m() {
    return 1;
  }
}

// A plain identifier superclass, resolved in the enclosing scope.
class D1 extends Base {
  m() {
    return super.m;
  }
  static sm() {
    return super.m;
  }
  get g() {
    return super.m;
  }
  set g(v) {
    super.m = v;
  }
}

// An arrow inside a method inherits the super binding; a nested arrow does
// too, and so does an arrow in a field initializer.
class D2 extends Base {
  m() {
    return () => super.m;
  }
  n() {
    return () => () => super.m;
  }
  f = () => super.m;
  g = super.m;
}

// The superclass is an arbitrary expression: a member expression, a folding
// binary expression (which REBUILDS the class node — the implicit-constructor
// decoration must survive), and a class expression.
var ns = {sub: Base};
class D3 extends ns.sub {}
class D4 extends (1 + 2, Base) {}
class D5 extends class {} {}

// Anonymous derived class expressions.
var d7 = class extends Base {
  m() {
    return super.m;
  }
};

// A derived class declared inside a function, with the superclass coming from
// that function's scope.
function make(SuperArg) {
  class Local extends SuperArg {
    m() {
      return super.m;
    }
  }
  return Local;
}
