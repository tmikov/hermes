/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// A switch always creates a scope, but only *populates* it when the cases
// declare something (SemanticResolver.cpp:532-536).

switch (disc) {
case 0:
  break;
default:
  break;
}

switch (disc) {
case 0:
  let x = 1;
  break;
case 1: {
  let x = 2;
  break;
}
default:
  const y = 3;
}

// The discriminant is visited BEFORE the switch's own scope is created, so
// `n` here resolves outside it — and `1 + 2` folds, which rebuilds the
// SwitchStatement node.
switch (1 + 2) {
case n:
  break;
}

function inFunction(v) {
  switch (v) {
  case 0:
    var hoisted = 1;
    break;
  }
  return hoisted;
}
