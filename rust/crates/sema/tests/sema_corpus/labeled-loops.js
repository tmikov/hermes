/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Labeled loops with labeled and unlabeled break/continue. The `label_index`
// decorations are NOT printed by -dump-sema (see resolver/statements.rs's
// module doc), so what this file pins byte-for-byte is the *scope* shape and
// the absence of any diagnostic; the indices themselves are unit-tested.

outer: while (cond) {
  inner: for (let i = 0; i < 10; ++i) {
    if_never: {
      break outer;
    }
    continue inner;
    break;
    continue;
  }
}

// A label may be reused once its statement has been left.
again: for (;;) break again;
again: for (;;) continue again;

// A label directly enclosing a label enclosing a loop: `break l1` targets
// the loop, not either label.
l1: l2: while (cond) {
  break l1;
  continue l2;
}

// A label whose target is not a loop is fine for `break`.
plain: {
  break plain;
}

function inFunction() {
  // labelMap is per-function, so the outer names are invisible here.
  outer: for (;;) {
    break outer;
  }
}
