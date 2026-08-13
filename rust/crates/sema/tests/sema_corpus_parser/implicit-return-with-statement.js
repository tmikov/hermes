/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The `CheckImplicitReturn` `WithStatement` arm (cpp:177-179), which returns
// the termination result of the `with` BODY rather than a fixed answer.
//
// This is the only place that arm can be witnessed. On the compile path
// `with` is rejected outright (`SemanticResolver.cpp:757-759`, `compile_`-
// gated), so the analysis never runs; on the parser entry it does run, but
// only over FUNCTION bodies — the corpus's other `with` file,
// `parser-mode-with-statement.js`, is a top-level `with` and therefore never
// reaches the arm. Both shapes below are needed: one where the `with` body
// terminates and one where it falls through, so that neither a fixed
// `make_must_terminate()` nor a fixed `make_next_statement()` can pass.
//
// Authored, not imported: upstream has no lit test for a `with` inside a
// function under `-Xcompile=false`. See `MANIFEST.md`.

// The body terminates, so the function does not reach the implicit return.
// (Return a fixed `make_next_statement()` from the arm, or delete the arm,
// -> this function wrongly becomes mayReachImplicitReturn.)
function withReturns(o) {
  with (o) return 1;
}

// The body falls through, so the function does reach the implicit return.
// (Return a fixed `make_must_terminate()` from the arm -> this function
// wrongly becomes noImplicitReturn.)
function withFallsThrough(o) {
  with (o) {
    g();
  }
}

// A block body whose statement list terminates: proves the arm recurses into
// the body's statement list rather than answering from the body node kind.
function withBlockReturns(o) {
  with (o) {
    g();
    return 1;
  }
}
