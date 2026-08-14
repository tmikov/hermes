/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! citations: verify, repair, or re-record the `cpp:NNN` citations in the
//! Rust port.
//!
//! ```text
//!   cargo run -p tools --bin citations -- check
//!   cargo run -p tools --bin citations -- remap [--dry-run]
//!   cargo run -p tools --bin citations -- bless
//! ```
//!
//! `check` exits non-zero and lists every citation whose cited C++ span
//! changed since it was blessed; it is what the standing test in
//! `crates/tools/tests/citations.rs` runs. `remap` is the mechanical repair:
//! it moves the digits of a citation whose text merely shifted, and declines
//! — loudly — any citation whose text changed. `bless` re-records the current
//! tree, and is for what remap declined, after a human has looked at it.
//!
//! See `tools::citations` for what a citation is and why the port has them,
//! and `tools::citations::remap` for what remap will and will not do.

use hermes_command_line::{CommandLine, Opt, OptDesc};
use tools::citations;

fn main() {
    let mut cl = CommandLine::new("Verify the port's `cpp:NNN` citations against the C++ tree.");
    let mode = Opt::<String>::new(
        &mut cl,
        OptDesc {
            desc: Some(
                "What to do: 'check' (verify), 'remap' (mechanically repair \
                 shifted citations) or 'bless' (re-record).",
            ),
            value_desc: Some("check|remap|bless"),
            min_count: 1,
            ..Default::default()
        },
    );
    let dry_run = Opt::<bool>::new_bool(
        &mut cl,
        OptDesc {
            long: Some("dry-run"),
            desc: Some("remap: report what would be rewritten, write nothing."),
            init: Some(false),
            ..Default::default()
        },
    );
    cl.parse_env_args();

    let root = citations::repo_root();
    match mode.as_str() {
        "check" => match citations::check(&root) {
            Err(e) => fail(&e),
            Ok(report) => {
                if report.is_ok() {
                    println!("{}", report.success_text());
                } else {
                    eprintln!("{}", report.failure_text());
                    std::process::exit(1);
                }
            }
        },
        // A remap that left anything for a human exits non-zero: the tree is
        // still not in the state the standing test wants.
        "remap" => match citations::remap(&root, *dry_run) {
            Err(e) => fail(&e),
            Ok(report) => {
                print!("{}", report.text());
                if !report.is_clean() {
                    std::process::exit(1);
                }
            }
        },
        "bless" => match citations::bless(&root) {
            Err(e) => fail(&e),
            Ok(report) => println!("{report}"),
        },
        other => fail(&format!(
            "unknown mode {other:?}; expected 'check', 'remap' or 'bless'"
        )),
    }
}

/// Print an error to stderr and exit non-zero.
fn fail(message: &str) -> ! {
    eprintln!("citations: {message}");
    std::process::exit(1);
}
