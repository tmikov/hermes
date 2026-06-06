/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! ast-dump: Rust mirror of `hermesc -dump-ast`. Parses a JS file and prints
//! the ESTree as JSON via the ported ESTreeJSONDumper. The parser_differential
//! test compares this byte-for-byte against `hermesc -dump-ast`.
//!
//! OUTPUT CONTRACT
//!   On success (parsed AND error_count()==0): exactly what the dumper emits
//!     (which ends in a single newline via end_jsonl); nothing added.
//!   On error: exactly "ERROR <count>\n".
//!
//! Args: [--pretty] [--dump-source-location] [--include-empty-ast-nodes]
//!       [--include-raw-ast-prop] [file|-]   (omitted or "-" reads stdin)

use std::io::{self, Read, Write};

use ast::context::Context;
use ast::dump::{dump_estree_json_with_sm, ESTreeDumpMode, ESTreeRawProp, LocationDumpMode};
use ast::node::Node;
use parser::js::JSParserImpl;
use parser::lexer::{GrammarContext, JSLexer};
use support::manager::SourceErrorManager;

fn main() {
    let mut pretty = false;
    let mut dump_loc = false;
    let mut include_empty = false;
    let mut include_raw = false;
    let mut file_path: Option<String> = None;

    let prog = std::env::args().next().unwrap_or_else(|| "ast-dump".to_string());
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--pretty" => pretty = true,
            "--dump-source-location" => dump_loc = true,
            "--include-empty-ast-nodes" => include_empty = true,
            "--include-raw-ast-prop" => include_raw = true,
            a if a.starts_with("--") => {
                eprintln!("{prog}: unknown flag '{a}'");
                std::process::exit(1);
            }
            a => {
                if file_path.is_some() {
                    eprintln!("{prog}: multiple input files");
                    std::process::exit(1);
                }
                file_path = Some(a.to_string());
            }
        }
    }

    let bytes = match file_path.as_deref() {
        Some("-") | None => {
            let mut b = Vec::new();
            io::stdin().read_to_end(&mut b).unwrap_or_else(|e| {
                eprintln!("{prog}: error reading stdin: {e}");
                std::process::exit(1);
            });
            b
        }
        Some(p) => std::fs::read(p).unwrap_or_else(|e| {
            eprintln!("{prog}: error reading '{p}': {e}");
            std::process::exit(1);
        }),
    };

    let mut sm = SourceErrorManager::new();
    let buf_id = sm.add_buffer_bytes("input", &bytes);
    let mut ctx = Context::new();
    let gc = ctx.lock();

    // Parse in a scope so the parser (and its &mut sm borrow) drops before we
    // read sm for the dump. The returned &Node lives in the gc arena, so it
    // outlives the parser.
    let result: Option<&Node> = {
        let atoms = &gc.ctx().atom_table;
        let lexer = JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut parser = JSParserImpl::new(&gc, lexer);
        parser.parse()
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();
    match result {
        Some(program) if sm.error_count() == 0 => {
            let mode = if include_empty {
                ESTreeDumpMode::DumpAll
            } else {
                ESTreeDumpMode::HideEmpty
            };
            let loc_mode = if dump_loc {
                LocationDumpMode::LocAndRange
            } else {
                LocationDumpMode::None
            };
            let raw = if include_raw {
                ESTreeRawProp::Include
            } else {
                ESTreeRawProp::Exclude
            };
            let mut s = String::new();
            dump_estree_json_with_sm(
                &mut s,
                program,
                pretty,
                mode,
                &sm,
                loc_mode,
                raw,
                &gc.ctx().atom_table,
            );
            out.write_all(s.as_bytes()).unwrap();
        }
        _ => {
            writeln!(out, "ERROR {}", sm.error_count()).unwrap();
        }
    }
    out.flush().ok();
}
