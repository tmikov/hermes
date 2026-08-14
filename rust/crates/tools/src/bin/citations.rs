/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! citations: verify (or re-record) the `cpp:NNN` citations in the Rust port.
//!
//! ```text
//!   cargo run -p tools --bin citations -- check
//!   cargo run -p tools --bin citations -- bless
//! ```
//!
//! `check` exits non-zero and lists every citation whose cited C++ span
//! changed since it was blessed; it is what the standing test in
//! `crates/tools/tests/citations.rs` runs. `bless` re-records the current
//! tree — only after a human has looked at what moved.
//!
//! See `tools::citations` for what a citation is and why the port has them.

use hermes_command_line::{CommandLine, Opt, OptDesc};
use tools::citations;

fn main() {
    let mut cl = CommandLine::new("Verify the port's `cpp:NNN` citations against the C++ tree.");
    let mode = Opt::<String>::new(
        &mut cl,
        OptDesc {
            desc: Some("What to do: 'check' (verify) or 'bless' (re-record)."),
            value_desc: Some("check|bless"),
            min_count: 1,
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
                    println!("{}", report.summary);
                } else {
                    eprintln!("{}", report.failure_text());
                    std::process::exit(1);
                }
            }
        },
        "bless" => match citations::bless(&root) {
            Err(e) => fail(&e),
            Ok(report) => println!("{report}"),
        },
        other => fail(&format!(
            "unknown mode {other:?}; expected 'check' or 'bless'"
        )),
    }
}

/// Print an error to stderr and exit non-zero.
fn fail(message: &str) -> ! {
    eprintln!("citations: {message}");
    std::process::exit(1);
}
