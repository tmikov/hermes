/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// `for await (... of ...)`: the ForOfStatement's `_await` flag changes
// nothing in the resolver, but the loop still gets its own scope.

async function f(it) {
  for await (const x of it) {
    var seen = x;
  }
  return seen;
}
