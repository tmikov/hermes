/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Class-name errors. A class forces strict mode on the enclosing function for
// the duration of the class (SemanticResolver.cpp:919), which is what makes
// validateDeclarationName reject 'arguments'/'eval' as class names even at
// loose global scope; and the inner ClassExprName decl obeys const variable
// rules (cpp:925-934), so assigning to the class name from inside the body is
// an invalid assignment target.

class Dup {}
class Dup {}

let Shadowed;
class Shadowed {}

class arguments {}
class eval {}

var anonArguments = class arguments {};

class ConstName {
  m() {
    ConstName = 1;
  }
  n() {
    ConstName += 1;
  }
  o() {
    ConstName++;
  }
}

var ConstExprName = class Inner {
  m() {
    Inner = 1;
  }
};
