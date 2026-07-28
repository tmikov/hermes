/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The second half of `expr-visit-generic.js` (S1 T8), added by the S2 T8 sweep.
// That file closed the four override-free expression kinds S1 had left with
// zero corpus exercise; this one closes the three the sweep found the resolver
// did not handle AT ALL — plain untyped-JS constructs on which `sema-dump`
// panicked with `unhandled node kind` while `hermesc -dump-sema` exits 0:
//
//  - `BigIntLiteral` (ESTree.def:270-272) — one `NodeLabel` child, no nodes;
//  - `TaggedTemplateExpression` (ESTree.def:483-485) — `_tag` + `_quasi`;
//  - `ImportExpression` (ESTree.def:299-302) — `_source` + optional
//    `_options`.
//
// None of the three appears anywhere in `lib/Sema/` outside the FlowChecker
// (`FlowChecker-expr.cpp:1244` for `BigIntLiteral`), so none has a
// `SemanticResolver::visit` override (the inventory in
// SemanticResolver.h:200-304) and none has a `DeclCollector` override: C++
// reaches their children through `visitESTreeChildren` and creates no scope,
// which is exactly `visit_node`'s override-free generic arm. That is where
// this sweep put them.
//
// What the dump therefore pins:
//  - identifiers INSIDE these nodes are resolved normally (the tag callee, the
//    substitution expressions, a `BigIntLiteral` object key's sibling value);
//  - folding does NOT apply to BigInt operands — `1n + 2n` stays a
//    `BinaryExpression` with two `BigIntLiteral` children in both dumps
//    (`ASTEval`'s numeric folds only accept `NumericLiteral`), and the `BinOp`
//    linearization still prints for it;
//  - a tagged template with an invalid escape (`tag`\unicode``) parses (its
//    `TemplateElement` cooked value is null) and resolves like any other.

var a = 1n, b = 0x1fn, c = 0b11n, d = 0o17n;

// Not folded: both sides keep the BinaryExpression + BinOp shape.
var e = 1n + 2n;
var e2 = 1n + 2n + 3n;

// A BigInt literal as a property key, plain and computed.
var o = { 1n: "k" };
var f = { [2n]: "c" };

function tag(s) {
  return s;
}

var g = tag`x${a}y${b}z`;
var h = tag`\unicode`;
var i = String.raw`p${a}`;
var j = tag`nested ${tag`inner ${a}`}`;

function inFunction(x) {
  return tag`${x}${1n}`;
}

var k = import("mod");
var l = import("mod", { with: { type: "json" } });
var m = import(a ? "x" : "y");
