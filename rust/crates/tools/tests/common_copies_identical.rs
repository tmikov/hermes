/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Enforces the byte-identity of the two `tests/common/mod.rs` copies.
//!
//! `rust/crates/parser/tests/common/mod.rs` and
//! `rust/crates/sema/tests/common/mod.rs` are the same file: both crates'
//! differential tests need `tools_bin`, and a `tests/` helper cannot be
//! shared across packages without promoting it to a crate. Keeping them in
//! sync was previously a convention enforced only by a comment in the header.
//!
//! This test lives in `tools` rather than in either of the two crates on
//! purpose: `tools` is `publish = false`, so the cross-package `include_str!`
//! below can never pull a foreign path into a `cargo package` archive or
//! break its verify build. An `include_str!` of `../../sema/...` from inside
//! `hermes-parser` would do exactly that.

/// The two copies of the differential helper, embedded at compile time.
/// A perturbation of either copy changes one of these strings and fails the
/// assertion below.
const PARSER_COPY: &str = include_str!("../../parser/tests/common/mod.rs");
const SEMA_COPY: &str = include_str!("../../sema/tests/common/mod.rs");

/// The two `tests/common/mod.rs` copies must stay byte-for-byte identical.
#[test]
fn common_helper_copies_are_byte_identical() {
    if PARSER_COPY == SEMA_COPY {
        return;
    }

    // Report the first differing line so the failure is actionable rather
    // than just "they differ".
    let mut detail = String::new();
    for (i, (a, b)) in PARSER_COPY
        .lines()
        .zip(SEMA_COPY.lines())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .take(1)
    {
        detail = format!("\nfirst difference at line {}:\n  parser: {a}\n  sema:   {b}", i + 1);
    }
    if detail.is_empty() {
        detail = format!(
            "\nno differing line in the common prefix; line counts are {} (parser) vs {} (sema)",
            PARSER_COPY.lines().count(),
            SEMA_COPY.lines().count()
        );
    }

    panic!(
        "rust/crates/parser/tests/common/mod.rs and \
         rust/crates/sema/tests/common/mod.rs must be byte-identical; \
         copy one over the other.{detail}"
    );
}
