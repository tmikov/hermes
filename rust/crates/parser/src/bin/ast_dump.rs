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
//!
//! Command-line parsing uses the `command_line` crate (the LLVM-`cl`-style
//! option parser copied from juno).

use std::io::{self, Read, Write};

use ast::context::Context;
use ast::dump::{dump_estree_json_with_sm, ESTreeDumpMode, ESTreeRawProp, LocationDumpMode};
use ast::node::Node;
use command_line::{CommandLine, Opt, OptDesc};
use parser::js::JSParserImpl;
use parser::lexer::{GrammarContext, JSLexer};
use support::manager::SourceErrorManager;

/// The parsed command-line options. Built into a [`CommandLine`] then read back
/// after parsing (the juno `command_line` idiom).
struct Options {
    pretty: Opt<bool>,
    dump_loc: Opt<bool>,
    include_empty: Opt<bool>,
    include_raw: Opt<bool>,
    /// Input path; empty or "-" reads stdin.
    input: Opt<String>,
}

impl Options {
    fn new(cl: &mut CommandLine) -> Options {
        Options {
            pretty: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("pretty"),
                    desc: Some("Pretty-print the JSON output."),
                    ..Default::default()
                },
            ),
            dump_loc: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("dump-source-location"),
                    desc: Some("Emit 'loc' and 'range' source locations."),
                    ..Default::default()
                },
            ),
            include_empty: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("include-empty-ast-nodes"),
                    desc: Some("Include empty AST node fields."),
                    ..Default::default()
                },
            ),
            include_raw: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("include-raw-ast-prop"),
                    desc: Some("Include the 'raw' property on literals."),
                    ..Default::default()
                },
            ),
            input: Opt::<String>::new(
                cl,
                OptDesc {
                    desc: Some("Input file ('-' or omitted reads stdin)."),
                    value_desc: Some("file"),
                    ..Default::default()
                },
            ),
        }
    }
}

fn main() {
    let mut cl = CommandLine::new("Parse a JS file and dump the ESTree as JSON.");
    let opt = Options::new(&mut cl);
    cl.parse_env_args();

    let pretty = *opt.pretty;
    let dump_loc = *opt.dump_loc;
    let include_empty = *opt.include_empty;
    let include_raw = *opt.include_raw;

    let input = &*opt.input;
    let bytes = if input.is_empty() || input == "-" {
        let mut b = Vec::new();
        io::stdin().read_to_end(&mut b).unwrap_or_else(|e| {
            eprintln!("ast-dump: error reading stdin: {e}");
            std::process::exit(1);
        });
        b
    } else {
        std::fs::read(input).unwrap_or_else(|e| {
            eprintln!("ast-dump: error reading '{input}': {e}");
            std::process::exit(1);
        })
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
