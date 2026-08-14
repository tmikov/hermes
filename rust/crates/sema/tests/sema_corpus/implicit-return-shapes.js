/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// CheckImplicitReturn shapes, one function per decision in
// `lib/Sema/CheckImplicitReturn.cpp`. Since upstream `04f1f53a8` the sema
// dump prints `mayReachImplicitReturn` / `noImplicitReturn` on every `Func`
// line, so each function below pins one branch of that analysis
// byte-for-byte against the oracle.
//
// Why this file is authored rather than imported: a mutation survey of
// `check_implicit_return.rs` (task-2 review round) found that ELEVEN of its
// eighteen decisions had no witness anywhere in this corpus, and the only
// files in the whole `test/` tree that witness them are large `test/hermes`
// runtime programs (`proxy.js`, `set.js`, `TypedArray.js`, …) that happen to
// contain the shape incidentally. Upstream's own
// `test/Sema/flow/implicit-return.js` does not witness them either: it is
// `--typed` Flow and every one of its functions has an explicit return on
// every path. There was nothing minimal to import.
//
// Each comment names the decision the function distinguishes; deleting that
// decision from the port flips that function's token and reds the
// differential. See `MANIFEST.md` for the survey table.

// If without else: the fall-through path reaches the implicit return.
// (Drop the no-alternate `kNextStatementLabel` insert -> wrongly terminates.)
function ifNoElse(x) {
  if (x) return 1;
}

// The else branch's `break` label must survive into the enclosing do-while.
// (Drop the alternate's target labels -> the do-while looks terminating.)
function elseBreakLabel(x) {
  do {
    if (x) return 1;
    else break;
  } while (1);
}

// A do-while body runs at least once, so this cannot fall through.
// (Treat do-while as a pre-condition loop -> wrongly reachable.)
function doWhileRunsOnce() {
  do {
    return 1;
  } while (0);
}

// A labeled statement's body always runs.
// (Treat it as a pre-condition loop -> wrongly reachable.)
function labeledRuns() {
  L: {
    return 1;
  }
}

// Breaking out of a labeled statement continues after it.
// (Stop treating a break to the statement's own label as a continuation ->
// wrongly terminates.)
function breakOutOfLabel(x) {
  L: {
    if (x) break L;
    return 1;
  }
}

// A switch whose default returns covers every input.
// (Ignore the default case -> wrongly reachable.)
function switchDefault(x) {
  switch (x) {
    default:
      return 1;
  }
}

// An explicit break escapes an otherwise exhaustive switch.
// (Ignore explicit breaks, or drop the past-the-switch label, -> wrongly
// terminates.)
function switchBreak(x) {
  switch (x) {
    case 1:
      break;
    default:
      return 1;
  }
}

// A case that falls through into a returning default does not escape.
// (Drop the per-case fall-through erase -> the first case's continuation
// survives and the switch looks escapable.)
function switchFallthrough(x) {
  switch (x) {
    case 1:
      x++;
    default:
      return 1;
  }
}

// A catch clause that runs off its end continues after the try.
// (Drop the catch clause's target labels -> wrongly terminates.)
function tryCatchFallsOut(x) {
  try {
    return 1;
  } catch (e) {}
}

// A finally that returns terminates the whole try-finally.
// (Drop the terminating-finally shortcut -> wrongly reachable.)
function tryFinallyReturns() {
  try {
  } finally {
    return 1;
  }
}

// `continue` in a do-while reaches the condition, which may then exit.
// (Treat continue as terminating -> wrongly terminates.)
function continueDoWhile(x) {
  do {
    continue;
  } while (x);
}
