/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Every class shape the untyped visitClassAsExpr path handles
// (SemanticResolver.cpp:913-950): declarations and expressions, named and
// anonymous, and every MethodDefinition kind (cpp:1094-1115). A class with no
// explicit constructor gets a synthetic implicit-constructor FunctionInfo
// (cpp:3088-3114) which shows up as an extra `Func strict` with one scope;
// an explicit constructor suppresses it (cpp:1656).

// No constructor: the implicit one is created after the body is visited.
class Empty {}

// Explicit constructor: no implicit one.
class WithCtor {
  constructor() {
    var inCtor;
  }
}

// Every method kind. Only the computed keys are resolved (cpp:1102-1103);
// the others are left as bare Ids with no decl.
var keyName = 'computed';
class Methods {
  plain(a) {
    return a;
  }
  ['dyn' + keyName]() {}
  get g() {
    return 1;
  }
  set g(v) {
    var inSetter = v;
  }
  static s() {}
  static ['static' + keyName]() {}
  *gen() {
    yield 1;
  }
  async am() {
    return 1;
  }
}

// Class expressions: anonymous, named, and a named one whose body references
// its own name through the inner ClassExprName decl.
var anon = class {};
var named = class Named {
  self() {
    return Named;
  }
};

// The class name is visible inside the body of a declaration too, through the
// same inner decl — two decls on one Identifier.
class SelfRef {
  me() {
    return SelfRef;
  }
}

// Classes nest: an inner class inside a method, and a class inside a
// function (so the ClassExprName scope's parent is the function scope).
function outer(p) {
  class Inner {
    useP() {
      return p;
    }
  }
  return class {
    alsoUseP() {
      return p;
    }
  };
}

// A method body that FOLDS rebuilds the ClassBody and hence the class node,
// which must not lose the implicit-constructor decoration.
class Folds {
  f() {
    return 1 + 2;
  }
}
