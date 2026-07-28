/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// visit(CallExpressionNode *) (SemanticResolver.cpp:1117-1205) for every call
// shape that hits NONE of its three specials, i.e. the plain
// `visitESTreeChildren(*this, node)` tail at cpp:1204 — plus the two
// neighbouring kinds that have no override at all and go through
// visit_node's generic arm:
// OptionalCallExpression and NewExpression (see calls.rs's module doc for the
// ESTree.def evidence that OptionalCallExpression is a SIBLING of
// CallExpression, not a subclass, so visit(CallExpressionNode *) is never
// selected for it).
//
// Nothing here produces a diagnostic; the point is the resolution of callees
// and arguments and the tree shape the dump prints.

var f, o, a;

// Callee shapes.
f();
f(1, 2, 3);
f(f(f()));
o.m();
o.m.n();
o["m"]();
o[f()]();
(function () { return 1; })();
(() => 1)();
(f, o)();
(f || o)();
new f();
new f(1, 2);
new o.m();
new (f())();

// Optional calls: no override, hence no specials.
f?.();
f?.(1);
o.m?.();
o?.m();
o?.m?.(1);
f?.()();
f()?.();

// Spread arguments: CallExpression/OptionalCallExpression/NewExpression are
// three of the five parents visit(SpreadElementNode *) whitelists (cpp:1460).
f(...a);
f(1, ...a, 2);
f?.(...a);
new f(...a);

// Calls in every expression/statement position, so the argument walk runs
// inside each of the visits that own those positions.
var v = f(1);
v = f(2);
v += f(3);
if (f()) f(); else f();
while (f()) f();
do f(); while (f());
for (var i = f(); f(i); f(i)) f(i);
for (var k in f()) f(k);
switch (f()) { case f(): f(); break; default: f(); }
try { f(); } catch (e) { f(e); } finally { f(); }
label: { f(); break label; }
[f(), f()];
({ p: f(), [f()]: f() });
`${f()}${f(1)}`;
f() ? f() : f();
typeof f();
-f();
!f();
f() + f() + f();

// Calls inside every function-like body, so the callee resolves against a
// nested scope chain.
function g(p1, p2) {
  f(p1, p2, arguments);
  return function () { return f(); };
}
var arrow = (x) => f(x);
var arrow2 = (x) => { return f(x); };
function* gen() { f(yield f()); }
async function af() { f(await f()); }

// Calls inside class bodies: a computed key, a field initializer, a method body
// and a static block. `super()` is deliberately absent here — that is
// super-calls.js.
class C {
  [f()] = f(1);
  static s = f(2);
  m() { return f(this); }
  static { f(3); }
}
