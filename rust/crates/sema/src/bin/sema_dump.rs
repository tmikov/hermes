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
//!       [--parse-flow-match] [--parse-ts] [--parse-jsx] [--enable-eval]
//!       [--fstd-globals] [--fno-std-globals] [--ferror-limit=N]
//!       [--parser-entry] [file|-] (omitted or "-" reads stdin)
//!
//! This mirrors CompilerDriver's `-dump-sema` path: load the runtime library
//! (`libhermes`) as a global-definitions file first, gated on
//! `-fstd-globals`/`-fno-std-globals`
//! (CompilerDriver.cpp:2000-2007 → `loadGlobalDefinition`, :762-774), parse
//! the input, then `sema::resolveAST(..., declFileList)` (:940-947) and
//! `sema::semDump` (:969-974). `-fstd-globals`/`-fno-std-globals` defaults
//! to true, matching `cl::StdGlobals`'s `CLFlag` default
//! (CompilerDriver.cpp:273-278).
//!
//! `--parser-entry` switches to a SEPARATE entry point that mirrors the C++
//! `tools/sema-parser-dump/sema-parser-dump.cpp` oracle instead: resolve via
//! `sema::resolve_ast_for_parser` (port of `resolveASTForParser`,
//! `SemResolve.cpp:295-306`, the `compile = false` entry point
//! `hermes-parser-wasm.cpp:104` uses) and dump the result UNCONDITIONALLY,
//! even when resolution reported errors — unlike the driver path above,
//! which never dumps on a `resolveAST` failure. Three consequences, ported
//! from the C++ tool's own OUTPUT CONTRACT: no ambient decls are ever loaded
//! (`resolveASTForParser` takes none), no `-ferror-limit` is applied (the
//! C++ tool never sets one, so the `SourceErrorManager` stays unbounded),
//! and on failure there is no "Emitted N errors. exiting." epilogue (the
//! C++ tool never prints one) — see [`exit_parser_entry`] vs
//! [`exit_on_failure`]. The other dialect/eval flags above still apply
//! normally in this mode.
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
use sema::resolve::{resolve_ast, resolve_ast_for_parser};
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

/// Exit with `sema-parser-dump`'s (the C++ tool's) exit-code contract:
/// `sm.getErrorCount() != 0 ? 2 : 0`, with no epilogue text. Used by
/// `--parser-entry` instead of [`exit_on_failure`], which prints hermesc's
/// driver epilogue — the C++ oracle for this mode
/// (`tools/sema-parser-dump/sema-parser-dump.cpp`) never prints one.
fn exit_parser_entry(sm: &SourceErrorManager) -> ! {
    std::process::exit(if sm.error_count() != 0 { 2 } else { 0 });
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
    /// Enable support for `eval()` (the hermesc `-enable-eval` flag,
    /// `CompilerRuntimeFlags.h:19-22`: a plain `cl::opt<bool>` defaulting to
    /// true, accepting both bare `-enable-eval` and `-enable-eval=false`).
    /// Wired into `ast::Context::enable_eval` (S2 T6's field), which
    /// `resolver/calls.rs`'s `visit_call_expression` reads to choose between
    /// the `DirectEval`/`EvalDisabled` warning branches.
    enable_eval: Opt<bool>,
    /// Enable registration of standard globals: the positive spelling of the
    /// hermesc `-fstd-globals`/`-fno-std-globals` pair (`CompilerDriver.cpp:
    /// 273-278`: a `CLFlag`, i.e. two `ValueDisallowed` options resolved by
    /// whichever was given last, defaulting to true when neither is given).
    /// Defaults to true here too; see `no_std_globals` for the negative
    /// spelling and `main`'s merge of the two into the effective value.
    fstd_globals: Opt<bool>,
    /// The negative spelling, `-fno-std-globals`. Kept as a separate `Opt`
    /// rather than sharing `fstd_globals`'s storage (the `command_line`
    /// crate's `OptDesc::opt_value` sharing exists, but each registered
    /// `Opt` unconditionally calls `OptValue::finish()`
    /// (`command_line/src/opt.rs:384-385`) via `CommandLine`'s parse-end
    /// sweep, and `OptValue::finish()` asserts it is never called twice
    /// (`opt.rs:72-78`) — sharing one `OptValue` between two registered
    /// options panics there). `main` merges the two fields into the
    /// effective bool; this is a deliberate simplification, not a full port
    /// of `CLFlag::getValue()`'s position-based tie-break: it is unreachable
    /// via this harness's per-file `// FLAGS:` line, which never spells out
    /// both `-fstd-globals` and `-fno-std-globals` for the same file.
    no_std_globals: Opt<bool>,
    /// Switch to the `resolveASTForParser` (`compile = false`) entry point
    /// and its "dump unconditionally" output contract — see the module doc.
    parser_entry: Opt<bool>,
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
            enable_eval: Opt::<bool>::new_bool(
                cl,
                OptDesc {
                    long: Some("enable-eval"),
                    init: Some(true),
                    desc: Some("Enable support for eval()."),
                    ..Default::default()
                },
            ),
            fstd_globals: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("fstd-globals"),
                    desc: Some("Enable registration of standard globals."),
                    init: Some(true),
                    ..Default::default()
                },
            ),
            no_std_globals: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("fno-std-globals"),
                    desc: Some("Disable registration of standard globals."),
                    init: Some(false),
                    ..Default::default()
                },
            ),
            parser_entry: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("parser-entry"),
                    desc: Some(
                        "Resolve via resolveASTForParser (compile = false) \
                         and dump unconditionally, mirroring the \
                         sema-parser-dump C++ oracle.",
                    ),
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
    // Merge the `-fstd-globals`/`-fno-std-globals` pair; see the
    // `no_std_globals` field doc for why this is two independent `Opt`s
    // instead of one shared-storage `CLFlag` pair. `-fno-std-globals` wins
    // if both are given (not a full port of `CLFlag`'s last-one-wins tie
    // break — see that doc).
    let fstd_globals = *opt.fstd_globals && !*opt.no_std_globals;
    let parser_entry = *opt.parser_entry;

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
    //
    // `--parser-entry` skips this: its C++ oracle
    // (`tools/sema-parser-dump/sema-parser-dump.cpp`) has no `-ferror-limit`
    // flag at all and never calls `setErrorLimit`, so the manager stays at
    // its default (unbounded) there too.
    if !parser_entry {
        sm.set_error_limit(*opt.ferror_limit);
    }

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
    // The hermesc driver does
    // `context->setEnableEval(cl::compilerRuntimeFlags.EnableEval)`
    // (CompilerDriver.cpp:1207) before any parsing.
    ctx.set_enable_eval(*opt.enable_eval);
    // hermesc's `-strict` defaults to false and `-dump-sema` never sets it;
    // `visit(ProgramNode *)` seeds the global function's strictness from it.
    let gc = ctx.lock();

    // Load the runtime library, exactly like `loadGlobalDefinition`
    // (CompilerDriver.cpp:762-774): it is parsed BEFORE the input file, into
    // its own buffer, and its Program becomes the single entry of the
    // ambient `DeclarationFileListTy` — but ONLY when `-fstd-globals` is
    // enabled, mirroring `if (cl::StdGlobals) { loadGlobalDefinition(...) }`
    // (CompilerDriver.cpp:2000-2007): with `-fno-std-globals`, hermesc never
    // even parses `libhermes`, so `ambient_decls` stays empty and none of
    // the 63 ambient `UndeclaredGlobalProperty` decls appear in the dump.
    //
    // `--parser-entry` never loads it, full stop: `resolveASTForParser`
    // (`SemResolve.cpp:295-306`) takes no `ambientDecls` parameter at all —
    // see `resolve_ast_for_parser`'s doc.
    let ambient_decls: Vec<NodeRc> = if parser_entry {
        vec![]
    } else if fstd_globals {
        let libhermes_buf_id =
            sm.add_buffer_bytes("<libhermes>", LIBHERMES.as_bytes());
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
    } else {
        vec![]
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
        // `--parser-entry`'s oracle (`sema-parser-dump.cpp`) only checks
        // "is there a parsed AST at all" (`if (!parsedJs)`) before calling
        // `resolveASTForParser` — it does NOT also require a clean parse,
        // unlike the driver path below. Mirror that: accept any `Some` here
        // when `parser_entry`, and let the post-resolution error count alone
        // decide the exit code (see `exit_parser_entry`).
        Some(root) if parser_entry || sm.error_count() == 0 => root,
        // The diagnostics were printed to stderr as they were produced;
        // hermesc exits nonzero with no stdout output, after the driver's
        // epilogue line (see `exit_on_failure`) — `--parser-entry` exits the
        // same way but without that epilogue (see `exit_parser_entry`).
        _ if parser_entry => exit_parser_entry(&sm),
        _ => exit_on_failure(&sm),
    };

    if parser_entry {
        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        // Dump unconditionally, even if resolution reported errors — the
        // whole point of `--parser-entry`, mirroring
        // `tools/sema-parser-dump/sema-parser-dump.cpp`'s OUTPUT CONTRACT.
        //
        // CAVEAT (known, currently-unexercised gap): if resolution reports
        // an error ANYWHERE in the tree, `resolve_ast_for_parser` returns
        // `None` and the fully-computed rewritten tree is discarded (see
        // `SemanticResolver::run`'s doc — unlike C++, which mutates in
        // place, so the partially-annotated tree survives a `false`
        // return). This port has no way to recover that tree, so on `None`
        // nothing is dumped even though the C++ oracle would still print
        // one. Every file in the live `tests/sema_corpus_parser` resolves
        // with zero errors, so this gap is not hit by the current gate.
        if let Some(resolved_root) =
            resolve_ast_for_parser(&gc, &mut sem_ctx, &mut sm, root)
        {
            let mut out_bytes: Vec<u8> = Vec::new();
            sem_dump(&mut out_bytes, &gc, &sem_ctx, resolved_root);
            let stdout = io::stdout();
            let mut out = stdout.lock();
            out.write_all(&out_bytes).unwrap();
            out.flush().ok();
        }
        exit_parser_entry(&sm);
    }

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
