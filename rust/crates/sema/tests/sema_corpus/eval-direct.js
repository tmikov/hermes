/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The direct-eval detection in visit(CallExpressionNode *)
// (SemanticResolver.cpp:1128-1161). `eval` is declared by libhermes as
// `function eval() {}`, so it is bound in the GLOBAL scope with kind
// UndeclaredGlobalProperty: `isEval` is true and every direct call below gets
// the Warning::DirectEval "Direct call to eval(), but lexical scope is not
// supported." over the CALLEE's range (`^~~~`, not the whole call).
//
// getEnableEval() is the Context default (true), so
// registerLocalEval(curScope_)
// also runs for each of them — that marks LexicalScope::localEval on the scope
// and all its ancestors, which -dump-sema never prints, hence the unit test
// `register_local_eval_marks_the_whole_ancestor_chain` in resolver/calls.rs.

eval("1");
eval("1", 2, 3);
eval();

function f() {
  return eval("1 + 1");
}

var arrow = () => eval("2");
var arrow2 = () => { eval("3"); };

function nested() {
  {
    if (1) {
      eval("4");
    }
  }
}

class C {
  m() { return eval("5"); }
  p = eval("6");
  static { eval("7"); }
}

// NOT direct calls, hence NO warning and no registerLocalEval:
//  - the callee is a member expression, not a bare identifier (cpp:1129)
var o = { eval: f };
o.eval("8");
//  - OptionalCallExpression has no visit override at all (see calls.rs)
eval?.("9");
//  - NewExpression likewise
new eval("10");
//  - and `eval` merely referenced, not called
var g = eval;
