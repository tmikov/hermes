/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// visit(ClassPropertyNode *) (SemanticResolver.cpp:1008-1051): the two
// synthetic elements-initializer FunctionInfos (cpp:3116-3163) and the
// FunctionContext a field initializer is resolved inside.
//
// Each getter is exercised on its own and together: a class with only
// instance fields gets ONE extra `Func strict`, one with only static fields
// gets one, one with both gets two (instance first, in visit order). A field
// with NO initializer still creates the initializer function in untyped mode
// (cpp:1041-1050) but does NOT run declareArguments, so its scope has no
// 'arguments' decl — while any field WITH an initializer adds one (cpp:1039).

var key = 'k';
var outerVar = 1;

// Instance fields only.
class InstanceOnly {
  a;
  b = 1;
  c = outerVar;
}

// Static fields only.
class StaticOnly {
  static a;
  static b = 2;
}

// Both, in an interleaved order: the instance initializer is created first
// because the first field is an instance field.
class Both {
  i1 = outerVar;
  static s1 = outerVar;
  i2;
  static s2;
}

// The reverse order creates the static initializer first.
class StaticFirst {
  static s = outerVar;
  i = outerVar;
}

// No fields at all: neither initializer function exists, only the implicit
// constructor.
class NoFields {
  m() {}
}

// Computed keys are resolved in the ENCLOSING context (no FunctionContext is
// pushed for them, cpp:1014-1020), so `key` resolves to the global property
// and `arguments` inside one is an ordinary reference.
class Computed {
  [key] = outerVar;
  static [key + '2'] = outerVar;
  ['plain' + key];
}

// A field initializer is resolved inside the synthetic function, so `this`
// and an arrow are both fine, and a fold inside it rebuilds the ClassBody
// (and hence the class node, which must keep both decorations).
class Initializers {
  folded = 1 + 2;
  thisRef = this;
  arrowRef = () => outerVar;
}

// Fields in a class inside a function: the initializer functions are children
// of that function, not of the global one.
function holder(p) {
  class Held {
    x = p;
    static y = p;
  }
  return Held;
}

// `arguments` in a COMPUTED KEY is an ordinary reference to the enclosing
// function's arguments object (no FunctionContext is pushed for the key, and
// forbidSpecialArgumentsReference_ stays false) — the same identifier in a
// field INITIALIZER is an error, which error-class-field.js covers.
function keyArguments() {
  class UsesArguments {
    [arguments] = 1;
  }
  return UsesArguments;
}
