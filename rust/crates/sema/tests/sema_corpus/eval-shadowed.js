/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The negative half of the direct-eval detection (cpp:1131-1141): `isEval` is
// only true when the binding for `eval` is either ABSENT or a global-scope
// UndeclaredGlobalProperty/GlobalProperty decl. Every call below therefore
// warns about nothing, even though it IS a direct call to something named
// `eval` — and
// registerLocalEval still runs for all of them (cpp:1151 is unconditional
// inside the enable-eval branch), which is the warning/marking asymmetry
// noted in calls.rs's module doc.
//
// Loose mode throughout: `let eval` / `var eval` / a parameter named `eval` are
// all strict-mode errors (see error-strict-eval-decl.js), so shadowing is only
// observable here.

function param(eval) {
  eval("1");
}

function local() {
  var eval = 0;
  eval("2");
}

function letLocal() {
  let eval = 0;
  eval("3");
}

{
  let eval = 0;
  eval("4");
}

function inner() {
  function eval() {}
  eval("5");
}

try { } catch (eval) { eval("6"); }

// The QUIRK, pinned deliberately: a global `var eval` produces a
// GlobalProperty decl whose scope IS the global scope, which is one of the two
// kinds cpp:1137-1138 accepts — so this one DOES warn, unlike every case above.
var eval;
eval("7");
