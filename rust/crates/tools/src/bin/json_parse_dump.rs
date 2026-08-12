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

use bumpalo::Bump;
use command_line::{CommandLine, Opt, OptDesc};
use hermes_atom_table::AtomTable;
use hermes_parser::json::{JSONFactory, JSONParser};
use hermes_support::json_emitter::JSONEmitter;
use hermes_support::manager::SourceErrorManager;

/// Command-line options, built into a [`CommandLine`] then read back after
/// parsing (the juno `command_line` idiom).
struct Options {
    /// Parse N times and print a timing line; 0 = normal single parse + dump.
    bench: Opt<usize>,
    convert_surrogates: Opt<bool>,
    /// Input path; "-" reads stdin.
    input: Opt<String>,
}

impl Options {
    fn new(cl: &mut CommandLine) -> Options {
        Options {
            bench: Opt::<usize>::new(
                cl,
                OptDesc {
                    long: Some("bench"),
                    desc: Some(
                        "Parse N times and print a timing line instead of the output.",
                    ),
                    value_desc: Some("N"),
                    ..Default::default()
                },
            ),
            convert_surrogates: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("convert-surrogates"),
                    desc: Some("Pass convert_surrogates=true to the JSON parser."),
                    ..Default::default()
                },
            ),
            input: Opt::<String>::new(
                cl,
                OptDesc {
                    desc: Some("Input file ('-' reads stdin)."),
                    value_desc: Some("file"),
                    min_count: 1,
                    ..Default::default()
                },
            ),
        }
    }
}

fn main() {
    let mut cl = CommandLine::new("Parse JSON and emit canonical output to stdout.");
    let opt = Options::new(&mut cl);
    cl.parse_env_args();

    let prog = "json-parse-dump";
    let bench_count = *opt.bench;
    let convert_surrogates = *opt.convert_surrogates;
    let file_path: String = (*opt.input).clone();

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
