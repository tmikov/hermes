/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The standing check that the port's `cpp:NNN` citations still point at the
//! C++ they were written against.
//!
//! Roughly 3k comments in the Rust sources name exact C++ line numbers. A
//! cherry-pick that shifts those lines breaks every citation below it without
//! breaking the build, so this test re-hashes each cited span and fails
//! naming the ones that moved. See `tools::citations` for the full story and
//! `crates/tools/citations.toml` for how a citation is resolved to a file.
//!
//! When it fails, the message points at the mechanical repair first:
//!
//! ```text
//! cargo run -p tools --bin citations -- remap --dry-run
//! cargo run -p tools --bin citations -- remap
//! ```
//!
//! `remap` moves the digits of any citation whose C++ text merely shifted —
//! it accepts a new location only when the text there still hashes to what
//! was blessed — and declines, loudly, every citation whose text *changed*.
//! Those are the ones to read against the C++ by hand before `bless`.
//!
//! This lives in `tools` (`publish = false`) for the same reason
//! `common_copies_identical.rs` does: it reads across the whole workspace and
//! the C++ tree, which must never become part of a published crate's package.

use tools::citations;

/// Every blessed citation must still hash to the same C++ span.
#[test]
fn citations_still_point_at_the_cited_cpp() {
    let root = citations::repo_root();
    let report = match citations::check(&root) {
        Ok(report) => report,
        Err(e) => panic!("citation check could not run: {e}"),
    };
    assert!(report.is_ok(), "{}", report.failure_text());
}
