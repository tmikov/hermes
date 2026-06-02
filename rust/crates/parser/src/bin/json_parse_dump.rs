/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! json-parse-dump: Rust mirror of the C++ oracle tool.
//!
//! Drives `JSONParser` + `JSONEmitter` and prints either canonical
//! (non-pretty) JSON to stdout, or a bench timing line.  The D3 differential
//! test compares this tool's parse-mode output byte-for-byte against the C++
//! binary built in D1 (`cmake-build-asan/bin/json-parse-dump`).
//!
//! OUTPUT CONTRACT
//!
//! Parse mode (default):
//!   On success (parsed value AND sm.error_count()==0):
//!     Write canonical JSON via JSONEmitter to stdout.  NO trailing newline.
//!   On error:
//!     Print exactly "ERROR <count>\n" to stdout.
//!
//! Bench mode (--bench=N):
//!   Read the source once; loop N times, each iteration creating a fresh
//!   Bump/AtomTable/JSONFactory/SourceErrorManager and calling parse().
//!   Print one line:
//!     "parsed <N>x, <ms> ms, <MB/s> MB/s\n"
//!   where ms = total milliseconds (one decimal) and
//!   MB/s = (sourceBytes * N) / seconds / 1e6 (two decimals).
//!
//! Args: [--bench=N] [--convert-surrogates] <file|->
//!
//!   -  means read from stdin.
//!
//!   --convert-surrogates  passes convert_surrogates=true to JSONParser.

use std::io::{self, Read, Write};
use std::time::Instant;

use atom_table::AtomTable;
use bumpalo::Bump;
use parser::json::{JSONFactory, JSONParser};
use support::json_emitter::JSONEmitter;
use support::manager::SourceErrorManager;

fn usage(prog: &str) {
    eprintln!(
        "Usage: {prog} [--bench=N] [--convert-surrogates] <file|->\n  \
         Parse JSON and emit canonical output to stdout.\n  \
         --bench=N             Parse N times, print timing.\n  \
         --convert-surrogates  Pass convert_surrogates=true.\n  \
         Use - to read from stdin."
    );
}

fn main() {
    let mut bench_count: usize = 0;
    let mut convert_surrogates = false;
    let mut file_path: Option<String> = None;

    let prog = std::env::args().next().unwrap_or_else(|| "json-parse-dump".to_string());

    for arg in std::env::args().skip(1) {
        if let Some(val) = arg.strip_prefix("--bench=") {
            match val.parse::<usize>() {
                Ok(n) if n > 0 => bench_count = n,
                _ => {
                    eprintln!("{prog}: --bench value must be > 0");
                    usage(&prog);
                    std::process::exit(1);
                }
            }
        } else if arg == "--convert-surrogates" {
            convert_surrogates = true;
        } else if arg.starts_with("--") {
            eprintln!("{prog}: unknown flag '{arg}'");
            usage(&prog);
            std::process::exit(1);
        } else {
            if file_path.is_some() {
                eprintln!("{prog}: too many positional arguments");
                usage(&prog);
                std::process::exit(1);
            }
            file_path = Some(arg);
        }
    }

    let file_path = match file_path {
        Some(p) => p,
        None => {
            usage(&prog);
            std::process::exit(1);
        }
    };

    // Read input bytes (binary-safe; corpus is UTF-8 but we use the bytes variant).
    let bytes: Vec<u8> = if file_path == "-" {
        let mut buf = Vec::new();
        io::stdin()
            .read_to_end(&mut buf)
            .unwrap_or_else(|e| {
                eprintln!("{prog}: error reading stdin: {e}");
                std::process::exit(1);
            });
        buf
    } else {
        std::fs::read(&file_path).unwrap_or_else(|e| {
            eprintln!("{prog}: error reading '{file_path}': {e}");
            std::process::exit(1);
        })
    };

    if bench_count > 0 {
        // --- BENCH MODE ---
        let source_bytes = bytes.len();
        let n = bench_count;

        let t0 = Instant::now();
        for _ in 0..n {
            let arena = Bump::new();
            let atoms = AtomTable::new();
            let factory = JSONFactory::new(&arena, &atoms);
            let mut sm = SourceErrorManager::new();
            let id = sm.add_buffer_bytes("json", &bytes);
            let mut p = JSONParser::new(&factory, id, &mut sm, &atoms, convert_surrogates);
            let _ = p.parse();
        }
        let elapsed = t0.elapsed();

        let secs = elapsed.as_secs_f64();
        let ms = secs * 1000.0;
        let mbps = (source_bytes as f64 * n as f64) / secs / 1e6;
        println!("parsed {n}x, {ms:.1} ms, {mbps:.2} MB/s");
    } else {
        // --- PARSE MODE ---
        let arena = Bump::new();
        let atoms = AtomTable::new();
        let factory = JSONFactory::new(&arena, &atoms);
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer_bytes("json", &bytes);
        // Parse inside a block; capture error_count before the block ends so the
        // &mut sm borrow is fully released before we use `result` and `sm` below.
        let (result, errors) = {
            let mut p = JSONParser::new(&factory, id, &mut sm, &atoms, convert_surrogates);
            let v = p.parse();
            let errs = p.error_count();
            (v, errs)
        };
        if let Some(v) = result {
            if errors == 0 {
                let mut s = String::new();
                {
                    let mut e = JSONEmitter::new(&mut s, false);
                    v.emit_into(&mut e, &atoms);
                }
                // NO trailing newline — matches C++ contract.
                print!("{s}");
            } else {
                println!("ERROR {errors}");
            }
        } else {
            println!("ERROR {errors}");
        }
    }

    // Flush stdout so output is not lost if stdout is not a tty.
    io::stdout().flush().ok();
}
