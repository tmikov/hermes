/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! gen-json: Deterministic big-JSON generator for benchmark workloads.
//!
//! Writes a JSON array of `<count>` records to stdout.  Every value is
//! derived deterministically from the record index — no RNG — so the
//! same count always produces the same bytes.
//!
//! Record shape:
//! ```json
//! {"id":<i>,"name":"item-<i>","price":<f>,"active":<bool>,
//!  "tags":["a","b","c"],"nested":{"x":<i>,"y":<i*2>}}
//! ```
//!
//! `price` = i as f64 / 7.0 (two decimal places), `active` = i % 2 == 0.
//!
//! Usage: gen-json <count>

use std::io::{self, BufWriter, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let prog = args.first().map(String::as_str).unwrap_or("gen-json");

    if args.len() != 2 {
        eprintln!("Usage: {prog} <count>");
        std::process::exit(1);
    }

    let count: u64 = args[1].parse().unwrap_or_else(|_| {
        eprintln!("{prog}: <count> must be a non-negative integer");
        std::process::exit(1);
    });

    // Use a large BufWriter so we don't make a system call per record.
    let stdout = io::stdout();
    let mut out = BufWriter::with_capacity(1 << 20, stdout.lock());

    out.write_all(b"[").unwrap();

    for i in 0u64..count {
        if i > 0 {
            out.write_all(b",").unwrap();
        }

        let price = i as f64 / 7.0;
        let active = if i % 2 == 0 { "true" } else { "false" };
        let x = i;
        let y = i.wrapping_mul(2);

        // Write the record directly into the buffered writer.
        write!(
            out,
            r#"{{"id":{i},"name":"item-{i}","price":{price:.2},"active":{active},"tags":["a","b","c"],"nested":{{"x":{x},"y":{y}}}}}"#
        )
        .unwrap();
    }

    out.write_all(b"]").unwrap();
    // BufWriter flushes on drop, but be explicit.
    out.flush().unwrap();
}
