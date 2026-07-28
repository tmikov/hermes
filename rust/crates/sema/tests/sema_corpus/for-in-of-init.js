/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The for-in/for-of initializer matrix (SemanticResolver.cpp:571-594). Only
// the loose-mode `for (var x = init in o)` form is allowed; everything else
// is an error, and a non-declaration left-hand side goes through
// validateAssignmentTarget instead.

// Allowed: for-in + loose mode + `var`.
for (var allowed = 1 in obj) ;

// Not a declaration at all: validateAssignmentTarget.
for (target in obj) ;
for (obj.prop of iter) ;
for ([a, b] of iter) ;
for ({p: c} of iter) ;
