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
//!   On a parse error or a resolution failure: nothing on stdout, exit 2.
//!     Diagnostics (errors AND warnings) go to stderr through the installed
//!     `StderrHandler`, in hermesc's `file:line:col: kind: message` + source
//!     line + caret format, followed by hermesc's driver epilogue line
//!     `Emitted N errors. exiting.\n` when N (the error count, not counting
//!     warnings) is nonzero. Note that a `SourceErrorManager` with no handler
//!     installed silently DISCARDS every diagnostic, so installing one is
//!     what makes the differential's stderr comparison meaningful.
//!
//!     Both failure modes share this exact epilogue + exit code because in
//!     `CompilerDriver::compileFileToDisk` they are the SAME check: `parseJS`
//!     (CompilerDriver.cpp:800-980) returns `nullptr` on either a parser
//!     failure (early returns while parsing) or a `resolveAST` failure
//!     (:939-947), and the single caller-side check
//!     (`if (!ast) { ... } return ParsingFailed;`, CompilerDriver.cpp:2076-
//!     2080) is what prints the epilogue and picks the exit code — there is
//!     no separate sema-failure branch. `ParsingFailed == 2`
//!     (`CompileStatus`, CompilerDriver.h:19-38: `Success` = 0, `InvalidFlags`
//!     = 1, `ParsingFailed` = 2, ...), and `main` returns `res.status`
//!     verbatim as the process exit code (hermesc.cpp:49-57). Confirmed
//!     empirically: `hermesc -dump-sema` on `var 1x;` exits 2 and prints
//!     `Emitted 2 errors. exiting.\n` after the two diagnostics.
//!
//! Args: [--parse-flow] [--parse-component-syntax] [--parse-flow-records]
//!       [--parse-flow-match] [--parse-ts] [--parse-jsx] [file|-]
//!       (omitted or "-" reads stdin)
//!
//! This mirrors CompilerDriver's `-dump-sema` path: load the runtime library
//! (`libhermes`) as a global-definitions file first
//! (CompilerDriver.cpp:2001-2008 → `loadGlobalDefinition`, :762-774), parse
//! the input, then `sema::resolveAST(..., declFileList)` (:940-947) and
//! `sema::semDump` (:969-974). `-fstd-globals`/`-fno-std-globals` defaults
//! to true, so the libhermes load is unconditional here.
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

/// Print hermesc's post-`parseJS`-failure epilogue (if there were any
/// errors) and exit with hermesc's exit code. Port of the single
/// `if (!ast) { ... } return ParsingFailed;` check in
/// `CompilerDriver::compileFileToDisk` (CompilerDriver.cpp:2076-2080) that
/// both a parse failure and a `resolveAST` failure hit (see the module doc).
/// `N` is only printed when nonzero, matching
/// `if (auto N = ...getErrorCount()) llvh::errs() << ...` exactly (an
/// unconditional print would be wrong if a caller ever reached `None`/`false`
/// without emitting an error).
fn exit_on_failure(sm: &SourceErrorManager) -> ! {
    let n = sm.error_count();
    if n != 0 {
        eprintln!("Emitted {n} errors. exiting.");
    }
    // `CompileStatus::ParsingFailed == 2` (CompilerDriver.h:19-38); `main`
    // returns `res.status` as the process exit code (hermesc.cpp:57).
    std::process::exit(2);
}

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
    /// Maximum number of errors before the rest are suppressed; 0 means
    /// unlimited. The hermesc `-ferror-limit` flag, with hermesc's own default
    /// (CompilerDriver.cpp:555-559).
    ferror_limit: Opt<u32>,
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
            // Port of hermesc's `-ferror-limit` (CompilerDriver.cpp:555-559),
            // including its `init(20)` and its "0 means unlimited" contract —
            // which needs no special-casing on either side: `errorLimit_` 0 is
            // never equal to a message count that has just been incremented
            // (SourceErrorManager.cpp:132).
            ferror_limit: Opt::<u32>::new(
                cl,
                OptDesc {
                    long: Some("ferror-limit"),
                    init: Some(20),
                    desc: Some(
                        "Maximum number of errors (0 means unlimited).",
                    ),
                    value_desc: Some("N"),
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
    // A bare `SourceErrorManager` is unlimited, but hermesc's driver applies
    // its `-ferror-limit` option (default 20) with
    // `context->getSourceErrorManager().setErrorLimit(cl::ErrorLimit)`
    // (CompilerDriver.cpp:1223), before any parsing. Past the limit hermesc
    // emits `<unknown>:0: error: too many errors emitted` once and drops every
    // later message, so an unlimited `sema-dump` diverges on any input with
    // more than 20 errors (`error-limit.js` in the corpus is the pin).
    sm.set_error_limit(*opt.ferror_limit);

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
        // hermesc exits nonzero with no stdout output, after the driver's
        // epilogue line (see `exit_on_failure`).
        _ => exit_on_failure(&sm),
    };

    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    // Dump the root resolution RETURNED: the resolver rebuilds the ancestors
    // of anything it rewrites, so this is the tree carrying the annotations
    // (`hermesc` mutates in place and can reuse its own pointer).
    let resolved =
        resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &ambient_decls);
    let root = match resolved {
        Some(root) => root,
        None => exit_on_failure(&sm),
    };

    let mut out_bytes: Vec<u8> = Vec::new();
    sem_dump(&mut out_bytes, &gc, &sem_ctx, root);

    let stdout = io::stdout();
    let mut out = stdout.lock();
    out.write_all(&out_bytes).unwrap();
    out.flush().ok();
}
