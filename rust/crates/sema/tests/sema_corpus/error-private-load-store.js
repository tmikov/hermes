/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Every restriction visit(MemberExpressionNode *, Node *) (cpp:1207-1253) and
// visit(OptionalMemberExpressionNode *, Node *) (cpp:1255-1295) enforce on a
// PrivateName property, plus the "not declared in any enclosing class" error
// from visit(PrivateNameNode *) (cpp:952-963).
//
// This is test/Sema/private-load-store-error.js's subject written without a
// CallExpression (`sink(...)`), which is why that file itself is still
// deferred to S2 T6.
//
// NOTE the deliberate range difference between the two overloads' `delete`
// diagnostic: MemberExpression reports at `node` (cpp:1219, so `o.#x`), while
// OptionalMemberExpression reports at `parent` (cpp:1262-1263, so the whole
// `delete o?.#x`). Both are pinned below.

class A {
  set #x(v) {}
  get #y() {}
  #m() {}

  static loads(o) {
    // Cannot load from a setter-only private name — reported at `node`.
    o.#x;
    o?.#x;
    // Legal: a getter-only name loads fine.
    o.#y;
  }

  static stores(o) {
    // Cannot store to a getter-only private name — reported at `parent`, so
    // the whole assignment is underlined.
    o.#y = 12;
    o?.#y = 12;
    // Cannot store to a method.
    o.#m = 12;
    // Legal: storing to a setter-only name.
    o.#x = 12;
  }

  static assignmentShapes(o) {
    // The C++ test is `assign->_left == node` (cpp:1227-1228), which this
    // port spells as "the parent is an AssignmentExpression AND we are its
    // `left` field". These are the shapes where the two could diverge:
    //
    // A COMPOUND assignment: the C++ `dyn_cast<AssignmentExpressionNode>`
    // matches any operator, not just `=`, but this port's
    // visit(AssignmentExpressionNode *) only LINEARIZES `=` chains — so a
    // `+=` takes the non-linearized path.
    o.#y += 1;
    // A parenthesized left-hand side: the parser drops the parens, so the
    // member expression is still directly the assignment's `left`.
    (o.#y) = 1;
    // A LINEARIZED `=` chain: `o.#x` is the inner link's `left` (legal — it
    // has a setter) while `o.#y` is the outer link's (an error). The port
    // hands each link its OWN node as the path parent, which is what makes
    // the field test equivalent to C++'s pointer comparison.
    o.#y = o.#x = 1;
    // An UpdateExpression parent is not an assignment at all, so this takes
    // the LOAD path — where only a setter-only name is an error, and a
    // method is not. No diagnostic.
    o.#m++;
  }

  static deletes(o) {
    delete o.#x;
    delete o?.#y;
  }

  superLookup() {
    // "Cannot lookup private names on super." — the check that exists only on
    // the non-optional overload (cpp:1213-1216); the parser rejects `super?.`
    // outright, so the optional overload can never see a Super object.
    super.#y;
  }
}

// An undeclared private name: resolvePrivateName fails, so the
// MemberExpression branch has nothing to validate and only
// visit(PrivateNameNode *) reports.
class B {
  m(o) {
    o.#nope;
  }
}

// The same outside any class at all.
var obj = {};
obj.#alsoNope;
