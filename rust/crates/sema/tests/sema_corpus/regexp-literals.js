/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// visit(RegExpLiteralNode *) (SemanticResolver.cpp:821-835). A *valid* regex
// contributes exactly one bare `RegExpLiteral` line to the dump and nothing
// to the SemContext (the compiled form goes into a Context-side cache only
// BCGen reads), which is what makes these comparable byte-for-byte despite
// the regex engine itself not being ported. An INVALID regex would produce
// `Invalid regular expression: <engine error>` and is therefore deferred —
// see the MANIFEST.

var simple = /abc/;
var classesAndQuantifiers = /[a-z0-9_]{2,3}\d*?/;
var allFlags = /x/dgimsuy;
var groups = /(?<year>\d{4})-(?:\d\d)/u;
var backslashes = /\/\\\nA/;
var lookaround = /(?<=a)b(?!c)/;
var inFunction = (function () {
  return /nested/g;
});
var folded = /a/ ;
1 + 2;
