/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! sema-dump: Rust mirror of `hermesc -dump-sema`. Parses a JS file, runs the
//! ported semantic resolver over it, and prints the `SemContext` + annotated
//! AST via the ported `semDump`. The `sema_differential` test compares this
//! byte-for-byte against `hermesc -dump-sema`.
//!
//! OUTPUT CONTRACT
//!   On success (parsed, no parse errors, resolution succeeded): exactly what
//!     `sema::dump::sem_dump` emits (which ends in a newline); nothing added,
//!     exit 0.
//!   On a parse error or a resolution failure: nothing on stdout, exit 1.
//!     Diagnostics (errors AND warnings) go to stderr through the installed
//!     `StderrHandler`, in hermesc's `file:line:col: kind: message` + source
//!     line + caret format. Note that a `SourceErrorManager` with no handler
//!     installed silently DISCARDS every diagnostic, so installing one is
//!     what makes the differential's stderr comparison meaningful.
//!
//! Args: [--parse-flow] [--parse-component-syntax] [--parse-flow-records]
//!       [--parse-flow-match] [--parse-ts] [--parse-jsx] [file|-]
//!       (omitted or "-" reads stdin)
//!
//! This mirrors CompilerDriver's `-dump-sema` path: load the runtime library
//! (`libhermes`) as a global-definitions file first
//! (CompilerDriver.cpp:2001-2008 → `loadGlobalDefinition`, :762-774), parse
//! the input, then `sema::resolveAST(..., declFileList)` (:940-947) and
//! `sema::semDump` (:969-974). `-Xstd-globals` defaults to true, so the
//! libhermes load is unconditional here.
//!
//! Command-line parsing uses the `command_line` crate (the LLVM-`cl`-style
//! option parser copied from juno), like `parser`'s `ast-dump`.

use std::io::{self, Read, Write};

use ast::context::{Context, NodeRc};
use ast::node::Node;
use command_line::{CommandLine, Opt, OptDesc};
use parser::js::JSParserImpl;
use parser::lexer::{GrammarContext, JSLexer};
use sema::dump::sem_dump;
use sema::keywords::Keywords;
use sema::libhermes::LIBHERMES;
use sema::resolve::resolve_ast;
use sema::sem_context::SemContext;
use support::manager::SourceErrorManager;
use support::render::StderrHandler;

/// The parsed command-line options. Built into a [`CommandLine`] then read
/// back after parsing (the juno `command_line` idiom). This is `ast-dump`'s
/// dialect-flag set; the location/pretty flags have no `-dump-sema`
/// counterpart and are omitted.
struct Options {
    /// Enable Flow type parsing (the hermesc `-parse-flow` flag). hermesc's
    /// `-parse-flow` defaults to `ParseFlowSetting::ALL`, so this also
    /// enables the ambiguous-expression grammar.
    parse_flow: Opt<bool>,
    /// Enable Flow `component`/`hook` syntax (hermesc
    /// `-Xparse-component-syntax`). Implies `parse_flow`.
    parse_component_syntax: Opt<bool>,
    /// Enable Flow `record` declarations/expressions (hermesc
    /// `-Xparse-flow-records`). Implies `parse_flow`.
    parse_flow_records: Opt<bool>,
    /// Enable Flow `match` expressions/statements (hermesc
    /// `-Xparse-flow-match`). Implies `parse_flow`.
    parse_flow_match: Opt<bool>,
    /// Enable TypeScript type parsing (the hermesc `-parse-ts` flag). TS and
    /// Flow are mutually-exclusive dialects, so this does NOT imply
    /// `parse_flow`.
    parse_ts: Opt<bool>,
    /// Enable JSX parsing (the hermesc `-parse-jsx` flag). JSX is an
    /// independent flag: it does NOT imply (and is not implied by)
    /// `parse_flow`/`parse_ts`.
    parse_jsx: Opt<bool>,
    /// Input path; empty or "-" reads stdin.
    input: Opt<String>,
}

impl Options {
    fn new(cl: &mut CommandLine) -> Options {
        Options {
            parse_flow: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("parse-flow"),
                    desc: Some("Enable Flow type parsing."),
                    ..Default::default()
                },
            ),
            parse_component_syntax: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("parse-component-syntax"),
                    desc: Some(
                        "Enable Flow component/hook syntax (implies \
                         --parse-flow).",
                    ),
                    ..Default::default()
                },
            ),
            parse_flow_records: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("parse-flow-records"),
                    desc: Some(
                        "Enable Flow record syntax (implies --parse-flow).",
                    ),
                    ..Default::default()
                },
            ),
            parse_flow_match: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("parse-flow-match"),
                    desc: Some(
                        "Enable Flow match syntax (implies --parse-flow).",
                    ),
                    ..Default::default()
                },
            ),
            parse_ts: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("parse-ts"),
                    desc: Some("Enable TypeScript type parsing."),
                    ..Default::default()
                },
            ),
            parse_jsx: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("parse-jsx"),
                    desc: Some("Enable JSX parsing."),
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
    let mut cl = CommandLine::new(
        "Parse a JS file, resolve it, and dump the semantic information.",
    );
    let opt = Options::new(&mut cl);
    cl.parse_env_args();

    let parse_component_syntax = *opt.parse_component_syntax;
    let parse_flow_records = *opt.parse_flow_records;
    let parse_flow_match = *opt.parse_flow_match;
    // hermesc's hidden `-Xparse-*` flags imply `-parse-flow`; mirror that,
    // and `-parse-flow` itself defaults to `ParseFlowSetting::ALL`
    // (ambiguous on).
    let parse_flow = *opt.parse_flow
        || parse_component_syntax
        || parse_flow_records
        || parse_flow_match;

    let input = &*opt.input;
    let bytes = if input.is_empty() || input == "-" {
        let mut b = Vec::new();
        io::stdin().read_to_end(&mut b).unwrap_or_else(|e| {
            eprintln!("sema-dump: error reading stdin: {e}");
            std::process::exit(1);
        });
        b
    } else {
        std::fs::read(input).unwrap_or_else(|e| {
            eprintln!("sema-dump: error reading '{input}': {e}");
            std::process::exit(1);
        })
    };

    let mut sm = SourceErrorManager::new();
    // A `SourceErrorManager` starts with no handler and drops everything it
    // is given, so this must happen before the first parse. hermesc installs
    // its own printing handler the same way (the `SourceErrorManager`
    // constructor defaults to `printDiagnosticHelper`).
    let output_options = sm.output_options();
    sm.set_handler(Box::new(StderrHandler::new(output_options)));

    let mut ctx = Context::new();
    ctx.set_parse_flow(parse_flow);
    // hermesc `-parse-flow` defaults to `ParseFlowSetting::ALL`, which IS
    // the ambiguous-expression grammar; enabling Flow at all enables
    // ambiguous.
    ctx.set_parse_flow_ambiguous(parse_flow);
    ctx.set_parse_flow_component_syntax(parse_component_syntax);
    ctx.set_parse_flow_records(parse_flow_records);
    ctx.set_parse_flow_match(parse_flow_match);
    // TS and Flow are mutually-exclusive dialects; do NOT OR `parse_ts` into
    // `parse_flow`.
    ctx.set_parse_ts(*opt.parse_ts);
    // JSX is an independent flag; do NOT OR it into `parse_flow`/`parse_ts`.
    ctx.set_parse_jsx(*opt.parse_jsx);
    // hermesc's `-strict` defaults to false and `-dump-sema` never sets it;
    // `visit(ProgramNode *)` seeds the global function's strictness from it.
    let gc = ctx.lock();

    // Load the runtime library, exactly like `loadGlobalDefinition`
    // (CompilerDriver.cpp:762-774): it is parsed BEFORE the input file, into
    // its own buffer, and its Program becomes the single entry of the
    // ambient `DeclarationFileListTy`.
    let libhermes_buf_id =
        sm.add_buffer_bytes("<libhermes>", LIBHERMES.as_bytes());
    let ambient_decls: Vec<NodeRc> = {
        let atoms = &gc.ctx().atom_table;
        let lexer = JSLexer::new(
            libhermes_buf_id,
            &mut sm,
            atoms,
            GrammarContext::AllowRegExp,
        );
        let mut parser = JSParserImpl::new(&gc, lexer);
        let program = parser.parse();
        // `loadGlobalDefinition` returns false here and the driver exits
        // with `LoadGlobalsFailed`; libhermes is our own constant, so a
        // failure is a bug in this crate, not in the user's input.
        let program = program
            .expect("libhermes must parse: it is a compiled-in constant");
        vec![NodeRc::from_node(&gc, program)]
    };
    assert_eq!(
        sm.error_count(),
        0,
        "libhermes must parse without errors: it is a compiled-in constant"
    );

    let buf_id = sm.add_buffer_bytes(
        if input.is_empty() { "-" } else { input },
        &bytes,
    );
    // Parse in a scope so the parser (and its &mut sm borrow) drops before
    // we use sm again. The returned &Node lives in the gc arena, so it
    // outlives the parser.
    let parsed: Option<&Node> = {
        let atoms = &gc.ctx().atom_table;
        let lexer =
            JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut parser = JSParserImpl::new(&gc, lexer);
        parser.parse()
    };
    let root = match parsed {
        Some(root) if sm.error_count() == 0 => root,
        // The diagnostics were printed to stderr as they were produced;
        // hermesc exits nonzero with no stdout output.
        _ => std::process::exit(1),
    };

    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    if !resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &ambient_decls) {
        std::process::exit(1);
    }

    let mut out_bytes: Vec<u8> = Vec::new();
    sem_dump(&mut out_bytes, &gc, &sem_ctx, root);

    let stdout = io::stdout();
    let mut out = stdout.lock();
    out.write_all(&out_bytes).unwrap();
    out.flush().ok();
}
