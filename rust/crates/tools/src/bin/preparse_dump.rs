/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! preparse-dump: Rust mirror of the C++ preparse-dump oracle tool.
//!
//! Runs a PreParse pass over a JS file and prints the side-table in a
//! canonical format. The preparse_differential test compares this byte-for-byte
//! against the C++ binary.
//!
//! OUTPUT CONTRACT
//!   On success: "PREPARSE <n>\n" then one line per entry sorted by start
//!     offset: "<start> <end> <strict> <arrow> <arrowArgs> <dirCount>
//!     [dir...]\n"
//!   On error: "ERROR <count>\n"
//!
//! Args: [--parse-flow] [--parse-ts] [file|-]
//!   omitted or "-" reads stdin.

use std::io::{self, Read, Write};

use hermes_ast::context::Context;
use hermes_parser::js::JSParserImpl;
use hermes_parser::lexer::{GrammarContext, JSLexer};
use hermes_support::manager::SourceErrorManager;

fn main() {
    let prog = "preparse-dump";
    let mut parse_flow = false;
    let mut parse_ts = false;
    let mut file_path: Option<String> = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--parse-flow" => parse_flow = true,
            "--parse-ts" => parse_ts = true,
            _ => {
                if file_path.is_some() {
                    eprintln!("{prog}: too many arguments");
                    std::process::exit(1);
                }
                file_path = Some(arg);
            }
        }
    }
    let file_path = file_path.unwrap_or_else(|| "-".to_string());

    let bytes: Vec<u8> = if file_path == "-" {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf).unwrap_or_else(|e| {
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

    let mut sm = SourceErrorManager::new();
    let buf_id = sm.add_buffer_bytes("input", &bytes);
    let mut ctx = Context::new();
    // hermesc -parse-flow defaults to ParseFlowSetting::ALL → ambiguous on
    // (same plumbing as ast_dump.rs).
    ctx.set_parse_flow(parse_flow);
    ctx.set_parse_flow_ambiguous(parse_flow);
    ctx.set_parse_ts(parse_ts);
    let gc = ctx.lock();
    let atoms = &gc.ctx().atom_table;
    let lexer = JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);

    let result = JSParserImpl::pre_parse_buffer(&gc, lexer, false);

    let stdout = io::stdout();
    let mut out = stdout.lock();

    match result {
        None => {
            writeln!(out, "ERROR {}", sm.error_count()).unwrap();
        }
        Some(mut parser) => {
            let info = parser.take_pre_parsed();
            // Sort entries by start offset for a canonical ordering.
            let mut entries: Vec<(u32, &_)> =
                info.function_info.iter().map(|(k, v)| (*k, v)).collect();
            entries.sort_by_key(|(k, _)| *k);

            writeln!(out, "PREPARSE {}", entries.len()).unwrap();
            for (start, fi) in &entries {
                write!(
                    out,
                    "{} {} {} {} {} {}",
                    start,
                    fi.end.offset,
                    fi.strict_mode as u8,
                    fi.contains_arrow_functions as u8,
                    fi.may_contain_arrow_functions_using_arguments as u8,
                    fi.directives.len(),
                )
                .unwrap();
                for d in &fi.directives {
                    write!(out, " ").unwrap();
                    out.write_all(d).unwrap();
                }
                writeln!(out).unwrap();
            }
        }
    }
    out.flush().ok();
}
