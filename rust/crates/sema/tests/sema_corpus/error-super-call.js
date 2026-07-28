/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// `super() call only allowed in derived class constructor`
// (SemanticResolver.cpp:1195-1202). The diagnostic covers the WHOLE call
// (node->getSourceRange(), i.e. including the argument list), not just the
// `super` keyword.
//
// The check is
// `nearestNonArrow(curFunctionInfo())->constructorKind != Derived`,
// so there are exactly two ways to fail it: the nearest non-arrow function is a
// constructor of a NON-derived class (ConstructorKind::Base), or it is not a
// constructor at all (ConstructorKind::None) — which includes an ordinary
// function nested inside a derived constructor, since the nested function is
// itself the nearest non-arrow.

class A {}

// Base (non-derived) class constructor.
class B {
  constructor() {
    super();
  }
}

// ... reached through an arrow, which nearestNonArrow skips.
class C {
  constructor() {
    let arr = () => {
      super();
    };
  }
}

// A plain function inside a DERIVED constructor: the function itself is the
// nearest non-arrow, and its constructorKind is None.
class E extends A {
  constructor() {
    function norm() {
      super();
    }
  }
}

// An object-literal method: not a constructor at all.
var o = {
  m1() {
    super();
  }
};

// A non-constructor method of a derived class.
class F extends A {
  m() { super(); }
  static s() { super(); }
}

// A field initializer of a derived class runs in the synthetic
// elements-initializer FunctionInfo, whose constructorKind is None.
class G extends A {
  p = super();
}

// With arguments, to show the range covers them.
class H {
  constructor() {
    super(1, 2 + 3);
  }
}
