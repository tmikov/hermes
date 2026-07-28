/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// visit(StaticBlockNode *) (SemanticResolver.cpp:1053-1084). A static block is
// treated as a function-level scope of its own:
//   - it gets a SYNTHETIC FunctionInfo of its own
//     (ClassContext::createStaticBlockFunctionInfo, cpp:3165-3177), which the
//     SemContext dump labels `StaticBlock` rather than `Func` because
//     FunctionInfo::isStaticBlock is set; the id is stored on the StaticBlock
//     node, not on the class.
//   - it ALSO forces the class's static-elements-init FunctionInfo into
//     existence (cpp:1057) even though the block doesn't use it, because IRGen
//     needs it for any static elements.
//   - its ScopeRAII is a function BODY scope (cpp:1063), so `var`s inside the
//     block hoist to the block and NOT to the function the class lives in.
//   - `super` may always be referenced inside it (cpp:1082).

// An EMPTY static block still gets its own StaticBlock FunctionInfo and its
// own (empty) body scope, and still forces the static-elements-init function
// into existence.
class Empty {
  static {}
}

// Three static blocks, each with its own `var x`: three StaticBlock
// FunctionInfos, three body scopes, and three distinct `x` Var decls (they
// would collide if they hoisted to the enclosing function).
class A {
  static {
    var x;
  }
  static {
    var x;
  }
  static {
    // Hoisting from a nested block: the `var` still lands in the static
    // block's own body scope, while the inner block gets its own scope for
    // the `let`.
    {
      var y;
      let z;
    }
    let w = y;
  }
}

// A static block next to static and instance fields: the static-elements-init
// function is created once and shared, and the instance one is separate.
class B {
  inst = 1;
  static st = 2;
  static {
    var fromBlock = 3;
  }
  static more = 4;
}

// A static block that is the ONLY static element still creates the
// static-elements-init function (cpp:1057) — visible as an extra `Func strict`
// with an empty scope.
class C {
  inst = 1;
  static {
    let onlyStatic;
  }
}

// `this` and `super.x` inside a static block (canReferenceSuper_ = true), plus
// an arrow that inherits both.
class Base {}
class D extends Base {
  static {
    this;
    super.x;
    var f = () => super.x;
    var g = () => this;
  }
}

// A static block inside a class inside a function: the StaticBlock
// FunctionInfo's parent is the class's enclosing function, and the block's
// declarations do not reach it.
function outer() {
  var x = 1;
  class E {
    static {
      var x = 2;
      let y = x;
    }
  }
  return x;
}

// A class expression with a static block, and a class nested inside a static
// block (the inner class's own scope is created inside the block's body
// scope, which is a real function body scope — unlike a field initializer's).
var CE = class {
  static {
    class Inner {
      #p = 1;
      m() {
        return this.#p;
      }
    }
    var i = Inner;
  }
};

// Private names declared by the class are visible inside its static blocks.
class F {
  static #s = 1;
  #i;
  static {
    var v = F.#s;
  }
}

// A static block whose body folds (`1 + 2`), rebuilding the StaticBlock node —
// it must keep both of its decorations (`scope` and `function_info`).
class G {
  static {
    var folded = 1 + 2;
  }
}
