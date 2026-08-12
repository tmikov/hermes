/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Parse a file and print its ESTree JSON, using the crate's `parse()` façade.
//!
//! ```text
//! cargo run -p parser --example parse_to_estree_json -- file.js
//! ```
//!
//! With no argument it parses a built-in snippet. This is the façade version
//! of what `src/bin/ast_dump.rs` does with the low-level API; that bin remains
//! the reference for flag-by-flag control.

use parser::{parse_named, ParseFlags};

fn main() {
    let path = std::env::args().nth(1);
    let (name, source) = match &path {
        Some(p) => (
            p.as_str(),
            std::fs::read_to_string(p).unwrap_or_else(|e| {
                eprintln!("cannot read '{p}': {e}");
                std::process::exit(1);
            }),
        ),
        None => (
            "<builtin>",
            "function greet(name) { return 'Hello, ' + name; }".to_string(),
        ),
    };

    // `.js`/`.jsx` say nothing about the type dialect, so this stays plain
    // ECMAScript + JSX; pass `parse_flow: true` or `parse_ts: true` for those.
    let flags = ParseFlags {
        parse_jsx: true,
        ..Default::default()
    };

    match parse_named(&source, name, flags) {
        Ok(mut parsed) => print!("{}", parsed.to_estree_json(true)),
        Err(e) => {
            eprint!("{e}");
            std::process::exit(1);
        }
    }
}
