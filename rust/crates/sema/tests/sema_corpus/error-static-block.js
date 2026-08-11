/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The diagnostics visit(StaticBlockNode *)'s four flag save/restores
// (SemanticResolver.cpp:1081-1092) make reachable.
//
// ES14.0 15.7.1: it is a Syntax Error if ClassStaticBlockStatementList Contains
// await is true — forbidAwaitExpression_ = true, so visit(AwaitExpressionNode *)
// reports "'await' not in an async function" (cpp:1510) even though the class
// sits inside an async function.
//
// forbidArgumentsAsIdentifier_ = true makes resolveIdentifier report
// "invalid use of 'arguments' as an identifier" (cpp:2000-2005), which is a
// name-based check independent of what (if anything) `arguments` resolves to.
//
// `await`/`yield` used as plain identifiers inside a static block are rejected
// by the PARSER, not here, so they are deliberately absent.

async function f() {
  class A {
    static {
      await 1;
    }
    static {
      var x = await 2;
    }
  }
}

class B {
  static {
    arguments;
  }
  static {
    var a = arguments;
  }
  static {
    // Through an arrow: the arrow's own FunctionContext does not reset the
    // flag (there is no save/restore of it in visitFunctionLike).
    var g = () => arguments;
  }
}

// A static block's declarations live in its own scope, so this is the ordinary
// let/let redeclaration error — but reported against the block's body scope
// rather than the enclosing function's.
class C {
  static {
    let d;
    var d;
  }
}
