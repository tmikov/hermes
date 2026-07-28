/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Every catch-parameter shape SemanticResolver::visit(CatchClauseNode *)
// (SemanticResolver.cpp:813-819) has to declare, and the two Decl kinds
// extractIdentsFromDecl picks between (cpp:2321-2331): a *simple* binding is
// ES5Catch (ES10 B.3.5), anything destructured is a plain Catch.

// Simple binding -> ES5Catch.
try {
} catch (simple) {
  simple;
}

// Array pattern -> Catch.
try {
} catch ([a, b]) {
  a;
  b;
}

// Object pattern with defaults, rest and nesting -> Catch.
try {
} catch ({p, q: {r} = {}, ...rest}) {
  p;
  r;
  rest;
}

// Optional catch binding: no param at all, so no declaration.
try {
} catch {
  1 + 2;
}

// ES10 B.3.5: a `var` of the same name as a *simple* catch binding is
// allowed, and merges into the enclosing function scope.
function es5CatchVar() {
  try {
  } catch (merged) {
    var merged;
  }
  merged;
}

// The catch body block is its own scope, so a `let` may shadow the param.
try {
} catch (shadowed) {
  {
    let shadowed;
    shadowed;
  }
  shadowed;
}

// A fold inside the catch BODY rebuilds the body block and therefore the
// CatchClause itself, so the `scope` decoration the clause's ScopeRAII wrote
// has to survive the rebuild — visible right here as `CatchClause Scope`.
try {
} catch (rebuilt) {
  1 + 2;
  rebuilt;
}
