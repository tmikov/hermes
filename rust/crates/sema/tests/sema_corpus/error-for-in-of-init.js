/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The error rows of the for-in/for-of initializer matrix
// (SemanticResolver.cpp:571-594).

// Destructuring + initializer: rejected even in the loose for-in `var` case.
for (var [d] = 1 in obj) ;
for (var {p: e} = 1 in obj) ;

// Not `var`.
for (let l = 1 in obj) ;
for (const c = 1 in obj) ;

// for-of never allows an initializer, even loose `var`.
for (var v = 1 of iter) ;

// Strict mode disables the loose `var` exception.
function strictFn() {
  "use strict";
  for (var s = 1 in obj) ;
}
