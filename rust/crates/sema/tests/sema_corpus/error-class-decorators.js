/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The three `decorators are not supported` sites the class visits carry, all
// guarded by compile_: on the class itself (SemanticResolver.cpp:914-916, via
// ESTree::getDecorators), on a ClassProperty (cpp:1014-1016) and on a
// MethodDefinition (cpp:1107-1109). Each error points at the whole decorated
// node's range, and the decorator expressions themselves are never resolved.

@dec
class OnClass {}

var onClassExpr = @dec class {};

class OnMembers {
  @dec
  field = 1;

  @dec
  static staticField = 2;

  @dec
  method() {}

  @dec
  static staticMethod() {}

  @dec
  get accessor() {
    return 1;
  }
}

@dec1
@dec2
class MultipleOnClass {}
