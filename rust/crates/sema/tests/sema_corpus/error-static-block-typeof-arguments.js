/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// PIN for a bug-for-bug quirk, NOT a bug in this port.
//
// `visit(IdentifierNode *, Node *)` (SemanticResolver.cpp:277-323) has no
// early return after its `typeof` arm: for a `typeof x` operand it calls
// `resolveIdentifier(identifier, /* inTypeof */ true)` at cpp:306 and then
// FALLS THROUGH to the unconditional `resolveIdentifier(identifier, false)` at
// cpp:322. The second call is normally a no-op — the first one already cached a
// decl on the node, so `checkIdentifierResolved` returns early — but the two
// forbid-flag checks in `resolveIdentifier` (cpp:1986-1997) run BEFORE that
// early return and are keyed on the identifier's NAME, not on the decl. So a
// forbidden name under `typeof` reports its diagnostic TWICE.
//
// `forbidArgumentsAsIdentifier_` is only ever set by visit(StaticBlockNode *)
// (cpp:1079-1080), which is what makes this shape the sole way to reach the
// double fire: hermesc prints "invalid use of 'arguments' as an identifier"
// two times at the same location, and so must we.
//
// The `await` half of the same quirk (`forbidAwaitAsIdentifier_`) is NOT
// reachable this way: `typeof await` inside a static block is a parse error.

class C {
  static {
    typeof arguments;
  }
}
