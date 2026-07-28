/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The Catch/ES5Catch rows of the redeclaration decision table, which only
// become reachable once visit(CatchClauseNode *) declares catch parameters:
// validateAndDeclareIdentifier's `prevInPrevScope` row
// (SemanticResolver.cpp:2525-2530) and visit(VariableDeclarationNode *)'s
// var-over-let-like check, whose ES10 B.3.4 exception is spelled out as
// `prevKind != Decl::Kind::ES5Catch` (cpp:388-392).

// A `let` in the catch clause's OWN scope collides with the (simple) binding.
try {} catch (a) { let a; }

// Same for `const`.
try {} catch (b) { const b = 1; }

// A destructured (plain Catch) binding also collides with a `var` in the
// body: the ES10 B.3.5 exception is only for simple bindings.
try {} catch ([c]) { var c; }

// ...and with an object-pattern binding.
try {} catch ({d}) { var d; }

// Two names bound by the same catch parameter collide with each other.
try {} catch ([e, e]) {}

// Nested: the body block is a separate scope, so this is fine, but the
// *inner* redeclaration is not.
try {} catch (f) { { let f; let f; } }
