/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermesc -parse-flow -Xparse-flow-match -dump-transformed-ast -pretty-json %s | %FileCheck %s

// CheckImplicitReturn used to have no case for MatchStatement, so semantic
// resolution of any function containing one aborted an assertion-enabled
// build. Make sure it resolves cleanly, including a 'break' out of a case
// body, which targets the enclosing labeled statement.

function f(x) {
  lbl: {
    match (x) {
      1 => { break lbl; }
    }
    return 1;
  }
}

// CHECK-LABEL:   "type": "LabeledStatement",
// CHECK:           "type": "MatchStatement",
// CHECK:             "type": "MatchStatementCase",
// CHECK:                 "type": "BreakStatement",
// CHECK:                   "name": "lbl"
// CHECK:           "type": "ReturnStatement",
