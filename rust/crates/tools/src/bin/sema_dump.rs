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
//!     `hermes_sema::dump::sem_dump` emits (which ends in a newline); nothing
//!     added, exit 0.
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
//!     (CompilerDriver.cpp:811-998) returns `nullptr` on either a parser
//!     failure (early returns while parsing) or a `resolveAST` failure
//!     (:939-947), and the single caller-side check
//!     (`if (!ast) { ... } return ParsingFailed;`, CompilerDriver.cpp:2105-
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
//! (CompilerDriver.cpp:2029-2036 → `loadGlobalDefinition`, :773-785), parse
//! the input, then `sema::resolveAST(..., declFileList)` (:940-947) and
//! `sema::semDump` (:969-974). `-fstd-globals`/`-fno-std-globals` defaults
//! to true, matching `cl::StdGlobals`'s `CLFlag` default
//! (CompilerDriver.cpp:273-278).
//!
//! `--parser-entry` switches to a SEPARATE entry point that mirrors the C++
//! `tools/sema-parser-dump/sema-parser-dump.cpp` oracle instead: resolve via
//! `hermes_sema::resolve::resolve_ast_for_parser` (port of
//! `resolveASTForParser`, `SemResolve.cpp:299-310`, the `compile = false`
//! entry point `hermes-parser-wasm.cpp:104` uses) and dump the result
//! UNCONDITIONALLY, even when resolution reported errors — unlike the driver
//! path above, which never dumps on a `resolveAST` failure. Three
//! consequences, ported from the C++ tool's own OUTPUT CONTRACT: no ambient
//! decls are ever loaded (`resolveASTForParser` takes none), no
//! `-ferror-limit` is applied (the C++ tool never sets one, so the
//! `SourceErrorManager` stays unbounded), and on failure there is no
//! "Emitted N errors. exiting." epilogue (the C++ tool never prints one) —
//! see [`exit_parser_entry`] vs [`exit_on_failure`]. The other dialect/eval
//! flags above still apply normally in this mode.
//!
//! Command-line parsing uses the `hermes-command-line` crate (the
//! LLVM-`cl`-style option parser copied from juno), like this crate's
//! `ast-dump`.
//!
//! ## One `hermes-command-line`-vs-LLVM-`cl` spelling difference, and its exit code
//!
//! LLVM's `cl` accepts BOTH `-ferror-limit=2` and the space-separated
//! `-ferror-limit 2` for a value-taking option; hermesc therefore takes
//! either. The `hermes-command-line` crate's parser (`parser.rs`'s
//! `parse_long_arg`/`parse_single_dash_arg`) only ever reads the value out of
//! the SAME argv element, so **only the `=` form works here** — the space
//! form is rejected with "option requires a value". Closing that is a
//! `hermes-command-line`-crate port item, not a Sema one, and is deliberately left
//! alone here.
//!
//! What the whole-Sema capstone review flagged (finding F3) was not the
//! spelling but the exit code: `hermes-command-line`'s `parse_env_args` printed the
//! usage error and then called `exit(0)`, so `sema-dump -ferror-limit 2
//! file.js` produced no dump yet reported SUCCESS — invisible to a scripted
//! differential sweep. That is fixed at the source (`hermes-command-line`'s
//! `cl.rs`, now `exit(1)`, matching LLVM's `ParseCommandLineOptions` error
//! path and hermesc's own exit 1 on a bad option).
//!
//! Practical consequence for the corpus harness: a `// FLAGS:` line must
//! spell value-taking options with `=`. `sema_differential` appends the line
//! verbatim to BOTH binaries' argv, so a space-form spelling would make
//! hermesc apply the option while `sema-dump` died on it — a comparison of
//! two different runs rather than a mismatch.

use std::io::{self, Read, Write};
use std::rc::Rc;

use hermes_command_line::{CommandLine, Hidden, Opt, OptDesc, OptValue};
use hermes_ast::context::{Context, NodeRc};
use hermes_ast::node::Node;
use hermes_parser::js::JSParserImpl;
use hermes_parser::lexer::{GrammarContext, JSLexer};
use hermes_sema::dump::sem_dump;
use hermes_sema::keywords::Keywords;
use hermes_sema::libhermes::LIBHERMES;
use hermes_sema::resolve::{resolve_ast, resolve_ast_for_parser};
use hermes_sema::sem_context::SemContext;
use hermes_support::manager::SourceErrorManager;
use hermes_support::render::StderrHandler;

/// Print hermesc's post-`parseJS`-failure epilogue (if there were any
/// errors) and exit with hermesc's exit code. Port of the single
/// `if (!ast) { ... } return ParsingFailed;` check in
/// `CompilerDriver::compileFileToDisk` (CompilerDriver.cpp:2105-2109) that
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
    /// Alias for [`Self::parse_flow_match`] spelled the way hermesc spells
    /// it, so a corpus file's `// FLAGS:` line — which
    /// `sema_differential.rs` appends VERBATIM to both binaries' argv — can
    /// name the flag once for both. It must be written `--Xparse-flow-match`
    /// (double dash): `hermes-command-line`'s single-dash path treats `-X...` as a
    /// short option `X` with an attached value, while LLVM's `cl` accepts
    /// either spelling, so the double-dash form is the one both sides
    /// understand. The other two hidden `-Xparse-*` flags
    /// (`-Xparse-component-syntax`, `-Xparse-flow-records`) can gain the same
    /// alias the same way when a corpus file needs them. Hidden, exactly like
    /// hermesc's own `-X` flags.
    xparse_flow_match: Opt<bool>,
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
    /// (CompilerDriver.cpp:566-570).
    ferror_limit: Opt<u32>,
    /// Enable support for `eval()` (the hermesc `-enable-eval` flag,
    /// `CompilerRuntimeFlags.h:19-22`: a plain `cl::opt<bool>` defaulting to
    /// true, accepting both bare `-enable-eval` and `-enable-eval=false`).
    /// Wired into `hermes_ast::Context::enable_eval` (S2 T6's field), which
    /// `resolver/calls.rs`'s `visit_call_expression` reads to choose between
    /// the `DirectEval`/`EvalDisabled` warning branches.
    enable_eval: Opt<bool>,
    /// Whether standard globals are registered: hermesc's
    /// `-fstd-globals`/`-fno-std-globals` pair (`CompilerDriver.cpp:273-278`),
    /// which is a `CLFlag` — two `ValueDisallowed` options over ONE stored
    /// value, so whichever spelling comes last on the command line wins, and
    /// the value defaults to true when neither is given.
    ///
    /// Ported literally: both spellings are registered against one shared
    /// `OptValue` via `OptDesc::opt_value`, which is what makes last-one-wins
    /// fall out with no merge step on this side either. This handle is the
    /// positive spelling's; it reads the shared storage, so it already holds
    /// the resolved value and the negative spelling needs no handle of its
    /// own. Verified against hermesc: `-fno-std-globals -fstd-globals` loads
    /// the 63 ambient globals and `-fstd-globals -fno-std-globals` loads
    /// none.
    fstd_globals: Opt<bool>,
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
            xparse_flow_match: Opt::new_flag(
                cl,
                OptDesc {
                    long: Some("Xparse-flow-match"),
                    desc: Some(
                        "Alias for --parse-flow-match, spelled the hermesc \
                         way (use the double-dash form).",
                    ),
                    hidden: Hidden::Yes,
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
            // Port of hermesc's `-ferror-limit` (CompilerDriver.cpp:566-570),
            // including its `init(20)` and its "0 means unlimited" contract —
            // which needs no special-casing on either side: `errorLimit_` 0 is
            // never equal to a message count that has just been incremented
            // (SourceErrorManager.cpp:133).
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
            // The `CLFlag` pair, registered in hermesc's own order against a
            // single shared storage. `init` (the value when neither spelling
            // occurs) must be the SAME on both, because registering an option
            // writes its `init` into the storage and the second registration
            // would otherwise overwrite the first's; `def_value` is what each
            // spelling stores when it does occur, and the later occurrence
            // overwrites the earlier one. See the `fstd_globals` field doc.
            fstd_globals: {
                let shared: Rc<OptValue<bool>> = Rc::new(Default::default());
                let positive = Opt::new_flag(
                    cl,
                    OptDesc {
                        long: Some("fstd-globals"),
                        desc: Some("Enable registration of standard globals."),
                        init: Some(true),
                        def_value: Some(true),
                        opt_value: Some(shared.clone()),
                        ..Default::default()
                    },
                );
                Opt::new_flag(
                    cl,
                    OptDesc {
                        long: Some("fno-std-globals"),
                        desc: Some("Disable registration of standard globals."),
                        init: Some(true),
                        def_value: Some(false),
                        opt_value: Some(shared),
                        ..Default::default()
                    },
                );
                positive
            },
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
    let parse_flow_match = *opt.parse_flow_match || *opt.xparse_flow_match;
    // hermesc's hidden `-Xparse-*` flags imply `-parse-flow`; mirror that,
    // and `-parse-flow` itself defaults to `ParseFlowSetting::ALL`
    // (ambiguous on).
    let parse_flow = *opt.parse_flow
        || parse_component_syntax
        || parse_flow_records
        || parse_flow_match;
    // No merge step: `-fstd-globals` and `-fno-std-globals` share one stored
    // value, so this handle already reads what the `CLFlag` pair resolved to
    // (last spelling on the command line wins). See the field doc.
    let fstd_globals = *opt.fstd_globals;
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
    // (CompilerDriver.cpp:1252), before any parsing. Past the limit hermesc
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
    // (CompilerDriver.cpp:1236) before any parsing.
    ctx.set_enable_eval(*opt.enable_eval);
    // hermesc's `-strict` defaults to false and `-dump-sema` never sets it;
    // `visit(ProgramNode *)` seeds the global function's strictness from it.
    let gc = ctx.lock();

    // Load the runtime library, exactly like `loadGlobalDefinition`
    // (CompilerDriver.cpp:773-785): it is parsed BEFORE the input file, into
    // its own buffer, and its Program becomes the single entry of the
    // ambient `DeclarationFileListTy` — but ONLY when `-fstd-globals` is
    // enabled, mirroring `if (cl::StdGlobals) { loadGlobalDefinition(...) }`
    // (CompilerDriver.cpp:2029-2036): with `-fno-std-globals`, hermesc never
    // even parses `libhermes`, so `ambient_decls` stays empty and none of
    // the 63 ambient `UndeclaredGlobalProperty` decls appear in the dump.
    //
    // `--parser-entry` never loads it, full stop: `resolveASTForParser`
    // (`SemResolve.cpp:299-310`) takes no `ambientDecls` parameter at all —
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
        // BOTH pairs require a clean parse, and both for the same C++
        // reason. `JSParserImpl::parse()` (JSParserImpl.cpp:164-172) ends
        // with
        //     if (lexer_.getSourceMgr().getErrorCount() != 0)
        //       return None;
        // so on the C++ side a nonzero error count after parsing ALWAYS
        // shows up as a `None`/`nullptr` AST — which is why the parser-entry
        // oracle's lone `if (!parsedJs) return sm.getErrorCount() != 0 ? 2 :
        // 0;` (sema-parser-dump.cpp:134-138) suffices there and why hermesc's
        // `parseJS` needs no separate check either. The Rust `parser.parse()`
        // (`parser/src/js/mod.rs`, "Port of `JSParserImpl::parse`") now
        // ports that same tail gate, so `sm.error_count() == 0` here is
        // always true when `parsed` is `Some` — this guard is redundant
        // defense in depth, not a compensation for a missing check anymore
        // (see `parse-error-recoverable.js` in `tests/sema_corpus_parser`,
        // originally the pin for the gap, kept as a pin for the gate itself).
        Some(root) if sm.error_count() == 0 => root,
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
        // `resolve_ast_for_parser` always hands back a tree to dump (see its
        // doc and `SemanticResolver::run_always`'s) — there is no `Option`
        // to unwrap here, unlike the driver path below.
        let resolved_root =
            resolve_ast_for_parser(&gc, &mut sem_ctx, &mut sm, root);
        let mut out_bytes: Vec<u8> = Vec::new();
        sem_dump(&mut out_bytes, &gc, &sem_ctx, resolved_root);
        let stdout = io::stdout();
        let mut out = stdout.lock();
        out.write_all(&out_bytes).unwrap();
        out.flush().ok();
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
