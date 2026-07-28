/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// `with` is parsed but rejected by SemanticResolver::visit(WithStatementNode *)
// (SemanticResolver.cpp:757-769) whenever compile_ is set, so hermesc never
// gets as far as printing the dump. The Unresolver pass the same visit runs
// over the body is therefore invisible here (nothing is dumped) — it is pinned
// by `with_statement_unresolves_identifiers_above_its_depth` in
// `tests/resolver.rs` instead.

var o = {a: 1};
var a = 2;

with (o) {
  a;
}

// A second one, to show the error is reported per statement and points at the
// `with` keyword rather than at a range.
function f() {
  with (o)
    a;
}
