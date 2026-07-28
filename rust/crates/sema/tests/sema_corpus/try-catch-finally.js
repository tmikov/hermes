/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// try/catch, try/finally and try/catch/finally — the last of which
// SemanticResolver rewrites into two nested try statements
// (SemanticResolver.cpp:771-811). The rewrite is dump-visible: the synthesized
// BlockStatement wrapping the nested TryStatement gets its own (empty) scope.

try {
  let a;
} catch (e) {
  let b;
}

try {
  let c;
} finally {
  let d;
}

try {
  let e1;
} catch (e2) {
  let f;
} finally {
  let g;
}

// Nested: the rewrite applies to the inner statement too, and a var inside
// the try body still hoists to function scope.
function outer() {
  try {
    var hoisted;
    try {
      let h;
    } catch (i) {
      let j;
    } finally {
      let k;
    }
  } catch (l) {
    let m;
  } finally {
    let n;
  }
}

// A real `throw` reaching a real handler: `ThrowStatement` has no resolver
// override of its own, so its argument resolves through the generic walk.
function thrower(x) {
  try {
    throw x;
  } catch (caught) {
    throw caught;
  } finally {
    let cleanup;
  }
}

// No handler and no finalizer on the OUTER statement means no rewrite, even
// though the inner one has both.
try {
  try {
    1 + 2;
  } catch (o) {
  } finally {
  }
} finally {
}
