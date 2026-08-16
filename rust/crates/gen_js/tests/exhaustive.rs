/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Guards the exhaustiveness of `dispatch::GenJS::gen_node`'s kind match.
//!
//! Task 13 deleted the temporary catch-all Task 1 introduced, so the
//! compiler now proves all 271 `NodeKind`s are handled: an unhandled kind is
//! a non-exhaustive-match build failure in `dispatch.rs`, not something a
//! test has to go looking for. What a test *can* still catch is the catch-all
//! being put back — which would silently re-disable that compile-time
//! guarantee without breaking any other test — so this file keeps guarding
//! the source text.

/// The temporary catch-all from Task 1 must stay deleted. Re-adding
/// `_ => self.unsupported_kind(node)` to `dispatch.rs` compiles fine and
/// breaks nothing else, but it turns every future unhandled kind from a
/// build error back into a silent runtime `UnsupportedKind`.
#[test]
fn temporary_catch_all_is_gone() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/dispatch.rs"))
        .expect("dispatch.rs is readable");
    assert!(
        !src.contains("_ => self.unsupported_kind(node)"),
        "the temporary catch-all from Task 1 is still present; Task 13 must \
         delete it so the compiler proves all 271 kinds are handled"
    );
}
