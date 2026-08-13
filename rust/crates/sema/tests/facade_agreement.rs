/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The `resolve` façade must agree with the low-level entry points, file for
//! file and byte for byte.
//!
//! `sema_differential` proves that the low-level path — `Context` +
//! `JSParserImpl` + `resolve_ast`/`resolve_ast_for_parser` + `sem_dump`, wired
//! by hand in `crates/tools/src/bin/sema_dump.rs` — matches the C++ oracles
//! over the whole corpus. That gate says nothing about the façade, which is a
//! *second* wiring of the same pieces. If the two ever disagree — a different
//! entry point, a missing ambient-declaration load, the pre-resolution root
//! dumped instead of the resolved one — every façade user would silently get
//! results the differential never checked. That is the failure mode this file
//! exists to make impossible.
//!
//! So: for every corpus file, run BOTH wirings and compare the whole outcome
//! (the `-dump-sema` bytes, the error count, and the rendered diagnostics) on
//! both entry points. The low-level side here is written out by hand rather
//! than shared with the façade, deliberately: a helper both sides called would
//! agree with itself no matter what either did. It mirrors `sema_dump.rs`,
//! including the driver's ordering — `libhermes` is parsed BEFORE the input
//! file, whereas the façade necessarily parses it after (its input is an
//! already-parsed `ParsedJS`), so the comparison also pins that this ordering
//! does not matter.
//!
//! ## Non-vacuity
//!
//! A comparison of two dumps is worthless if every input produces the same
//! bytes either way. [`the_comparison_discriminates`] pins that it does not:
//! over the same corpus, the two entry points' dumps differ on a large number
//! of files, and turning the standard globals off changes the dump too. So a
//! façade that called the wrong entry point, or that forgot the ambient
//! declarations, would fail the sweep rather than pass it vacuously. (This was
//! also confirmed by hand: making `resolve_for_compile` call
//! `resolve_ast_for_parser` fails `agrees_on_the_compile_path` on 100+ files.)

use std::path::{Path, PathBuf};

use hermes_ast::context::{Context, NodeRc};
use hermes_ast::node::Node;
use hermes_parser::js::JSParserImpl;
use hermes_parser::lexer::{GrammarContext, JSLexer};
use hermes_parser::{parse_named, ParseFlags};
use hermes_sema::dump::sem_dump;
use hermes_sema::keywords::Keywords;
use hermes_sema::libhermes::LIBHERMES;
use hermes_sema::resolve::{resolve_ast, resolve_ast_for_parser};
use hermes_sema::sem_context::SemContext;
use hermes_sema::{resolve_for_compile, resolve_for_parser, CompileOptions};
use hermes_support::diag::{CollectingHandler, OutputOptions};
use hermes_support::manager::SourceErrorManager;
use hermes_support::render::render_diagnostic;

/// What a corpus file's first-line `// FLAGS:` line selects, in the subset the
/// façade can express. Everything else in the sweep is defaulted, on both
/// sides, exactly as `sema-dump` defaults it.
#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    parse_flow: bool,
    parse_flow_match: bool,
    parse_jsx: bool,
    /// hermesc's `-fstd-globals`, on by default. Compile path only: the
    /// parser path never loads ambient declarations.
    std_globals: bool,
}

impl Flags {
    /// Read the file's first line, the way `sema_differential::per_file_flags`
    /// does. Returns `None` for a flag the façade cannot express, which is the
    /// signal to skip the file.
    fn parse(src: &str) -> Option<Flags> {
        let mut flags = Flags {
            std_globals: true,
            ..Default::default()
        };
        let first = src.split('\n').next().unwrap_or("");
        let Some(rest) = first.strip_prefix("// FLAGS: ") else {
            return Some(flags);
        };
        for arg in rest.split_whitespace() {
            match arg {
                "-parse-flow" | "--parse-flow" => flags.parse_flow = true,
                "--Xparse-flow-match" => {
                    flags.parse_flow = true;
                    flags.parse_flow_match = true;
                }
                "-parse-jsx" | "--parse-jsx" => flags.parse_jsx = true,
                "-fno-std-globals" => flags.std_globals = false,
                // `-enable-eval=false` is a `Context` flag with no
                // `ParseFlags` field; nothing else appears in the corpus.
                _ => return None,
            }
        }
        Some(flags)
    }

    /// The façade's spelling of the same dialect selection.
    fn to_parse_flags(self) -> ParseFlags {
        ParseFlags {
            parse_flow: self.parse_flow,
            parse_flow_match: self.parse_flow_match,
            parse_jsx: self.parse_jsx,
            ..Default::default()
        }
    }

    /// The low-level spelling: the `Context` setters `sema_dump.rs` calls.
    /// Written out rather than shared with `ParseFlags::apply`, so that the
    /// two sides of the comparison stay independent.
    fn apply(self, ctx: &mut Context<'_>) {
        let parse_flow = self.parse_flow || self.parse_flow_match;
        ctx.set_parse_flow(parse_flow);
        ctx.set_parse_flow_ambiguous(parse_flow);
        ctx.set_parse_flow_match(self.parse_flow_match);
        ctx.set_parse_jsx(self.parse_jsx);
    }
}

/// Everything observable about one run, from either wiring.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    /// The `-dump-sema` bytes, or `None` when the run produced no tree to
    /// dump (a parse error, or a compile-path resolution failure).
    dump: Option<Vec<u8>>,
    /// `SourceErrorManager::error_count` at the end of the run.
    error_count: u32,
    /// Every diagnostic, rendered LLVM-style without colors.
    messages: Vec<String>,
}

fn render_all(sm: &SourceErrorManager) -> Vec<String> {
    let opts = OutputOptions {
        show_colors: false,
        ..OutputOptions::default()
    };
    sm.handler_as::<CollectingHandler>()
        .expect("collecting handler was replaced")
        .messages()
        .iter()
        .map(|d| render_diagnostic(d, &opts))
        .collect()
}

/// A fresh source manager that records diagnostics instead of printing them —
/// the same handler `parse_named` installs, so both sides record the same way.
fn collecting_sm() -> SourceErrorManager {
    let mut sm = SourceErrorManager::new();
    sm.set_handler(Box::new(CollectingHandler::new()));
    sm
}

/// The low-level wiring, mirroring `crates/tools/src/bin/sema_dump.rs`:
/// hand-built `Context`, `libhermes` parsed first when the ambient
/// declarations are wanted, then the input, then the entry point, then
/// `sem_dump`.
fn low_level(
    name: &str,
    source: &str,
    flags: Flags,
    parser_entry: bool,
) -> Outcome {
    let mut sm = collecting_sm();
    let mut ctx = Context::new();
    flags.apply(&mut ctx);
    let gc = ctx.lock();

    // The ambient declaration file, loaded before the input exactly as
    // `loadGlobalDefinition` does. The parser entry point never takes any.
    let ambient_decls: Vec<NodeRc> =
        if !parser_entry && flags.std_globals {
            let buf = sm.add_buffer("<libhermes>", LIBHERMES);
            let program = {
                let lexer = JSLexer::new(
                    buf,
                    &mut sm,
                    &gc.ctx().atom_table,
                    GrammarContext::AllowRegExp,
                );
                JSParserImpl::new(&gc, lexer).parse()
            };
            vec![NodeRc::from_node(&gc, program.expect("libhermes parses"))]
        } else {
            vec![]
        };

    let buf = sm.add_buffer(name, source);
    let parsed: Option<&Node> = {
        let lexer = JSLexer::new(
            buf,
            &mut sm,
            &gc.ctx().atom_table,
            GrammarContext::AllowRegExp,
        );
        JSParserImpl::new(&gc, lexer).parse()
    };
    let root = match parsed {
        Some(root) if sm.error_count() == 0 => root,
        _ => {
            return Outcome {
                dump: None,
                error_count: sm.error_count(),
                messages: render_all(&sm),
            }
        }
    };

    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let mut out: Vec<u8> = Vec::new();
    let dump = if parser_entry {
        let resolved = resolve_ast_for_parser(&gc, &mut sem_ctx, &mut sm, root);
        sem_dump(&mut out, &gc, &sem_ctx, resolved);
        Some(out)
    } else {
        match resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &ambient_decls) {
            Some(resolved) => {
                sem_dump(&mut out, &gc, &sem_ctx, resolved);
                Some(out)
            }
            None => None,
        }
    };
    Outcome {
        dump,
        error_count: sm.error_count(),
        messages: render_all(&sm),
    }
}

/// The same run through the façade.
fn facade(
    name: &str,
    source: &str,
    flags: Flags,
    parser_entry: bool,
) -> Outcome {
    let parsed = match parse_named(source, name, flags.to_parse_flags()) {
        Ok(parsed) => parsed,
        Err(e) => {
            return Outcome {
                dump: None,
                error_count: e.error_count(),
                messages: e.messages(),
            }
        }
    };

    if parser_entry {
        let mut resolved = resolve_for_parser(parsed);
        return Outcome {
            dump: Some(resolved.to_sema_dump()),
            error_count: resolved.error_count(),
            messages: rendered(&resolved),
        };
    }

    let options = CompileOptions {
        std_globals: flags.std_globals,
        ..Default::default()
    };
    match resolve_for_compile(parsed, &options) {
        Ok(mut resolved) => Outcome {
            dump: Some(resolved.to_sema_dump()),
            error_count: resolved.error_count(),
            messages: rendered(&resolved),
        },
        Err(e) => Outcome {
            dump: None,
            error_count: e.error_count(),
            messages: e.messages(),
        },
    }
}

/// `ResolvedJS`'s diagnostics rendered the same way `ResolveError::messages`
/// renders them.
fn rendered(resolved: &hermes_sema::ResolvedJS) -> Vec<String> {
    let opts = OutputOptions {
        show_colors: false,
        ..OutputOptions::default()
    };
    resolved
        .diagnostics()
        .iter()
        .map(|d| render_diagnostic(d, &opts))
        .collect()
}

/// Every `.js` file of a corpus directory, sorted, with its contents.
fn corpus(dir: &str) -> Vec<(String, String)> {
    let root: PathBuf =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join(dir);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", root.display()))
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "js"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let src = std::fs::read_to_string(&p)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()));
            (name, src)
        })
        .collect()
}

/// Corpus files the *parser* entry point cannot process at all, with the
/// reason. Nothing to do with the façade: `sema-dump --parser-entry` aborts on
/// these too, and so does the C++.
///
/// `try-catch-finally.js`: splitting `try`/`catch`/`finally` into nested
/// `try`s is a `compile = true` rewrite, but `CheckImplicitReturn` runs
/// unconditionally and asserts the split has happened
/// (`CheckImplicitReturn.cpp:248-250`, called from
/// `SemanticResolver.cpp:1957` with no `compile_` guard — the port reproduces
/// both, `check_implicit_return.rs:338` and `resolver/functions.rs:1123`).
/// So a function containing `try`/`catch`/`finally` trips a debug assert on
/// the parser path in the C++ as well. The corpus that entry point is gated
/// on (`sema_corpus_parser`) contains no such file.
const PARSER_ENTRY_SKIP: &[&str] = &["try-catch-finally.js"];

/// Run `f` on a thread with a large stack.
///
/// Both the parser and the resolver are recursive descent over the source's
/// own nesting, and the corpus has files deep enough to exhaust the 2 MiB a
/// libtest thread gets by default. The in-tree drivers never hit it because a
/// process's main thread starts with 8 MiB; this restores that headroom.
fn on_big_stack<R: Send + 'static>(
    f: impl FnOnce() -> R + Send + 'static,
) -> R {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("the sweep panicked (see the assertion above)")
}

/// Run one corpus over one entry point and return how many files were
/// compared. Any disagreement fails here, naming the file.
fn sweep(dir: &str, parser_entry: bool) -> usize {
    let mut compared = 0;
    for (name, src) in corpus(dir) {
        let Some(flags) = Flags::parse(&src) else {
            continue;
        };
        if parser_entry && PARSER_ENTRY_SKIP.contains(&name.as_str()) {
            continue;
        }
        let want = low_level(&name, &src, flags, parser_entry);
        let got = facade(&name, &src, flags, parser_entry);
        assert_eq!(
            want.error_count, got.error_count,
            "{name}: error count differs (low-level vs façade)"
        );
        assert_eq!(
            want.messages, got.messages,
            "{name}: diagnostics differ (low-level vs façade)"
        );
        assert_eq!(
            want.dump.is_some(),
            got.dump.is_some(),
            "{name}: one side produced a dump and the other did not"
        );
        if let (Some(want), Some(got)) = (&want.dump, &got.dump) {
            assert_eq!(
                String::from_utf8_lossy(want),
                String::from_utf8_lossy(got),
                "{name}: sema dump differs (low-level vs façade)"
            );
            assert_eq!(want, got, "{name}: sema dump bytes differ");
        }
        compared += 1;
    }
    compared
}

/// `resolve_for_compile` == `resolve_ast` wired by hand, over the corpus the
/// `hermesc -dump-sema` differential uses. Both the successes and the
/// failures: on a failure the two must agree that there is no dump, and on
/// the same diagnostics.
#[test]
fn agrees_on_the_compile_path() {
    let compared = on_big_stack(|| sweep("sema_corpus", false));
    // 220 corpus files, minus the one selecting `-enable-eval=false`, which
    // `ParseFlags` cannot express. A drop here means the sweep silently
    // stopped covering files.
    assert_eq!(compared, 219, "corpus size changed");
}

/// `resolve_for_parser` == `resolve_ast_for_parser` wired by hand, over the
/// `sema-parser-dump` corpus — including its error files, which this entry
/// point still dumps.
#[test]
fn agrees_on_the_parser_path() {
    let compared = on_big_stack(|| sweep("sema_corpus_parser", true));
    assert_eq!(compared, 13, "parser corpus size changed");
}

/// The parser corpus is small; run the compile-path comparison over it too,
/// and the parser-path comparison over the big corpus, so neither entry point
/// is only checked on 13 files.
#[test]
fn agrees_on_both_paths_over_both_corpora() {
    assert_eq!(on_big_stack(|| sweep("sema_corpus_parser", false)), 13);
    assert_eq!(on_big_stack(|| sweep("sema_corpus", true)), 218);
}

/// Non-vacuity: the byte comparison the sweeps make can tell the two entry
/// points apart, and can tell the ambient declarations' presence apart. If it
/// could not, a façade calling the wrong entry point would pass the sweeps.
#[test]
fn the_comparison_discriminates() {
    on_big_stack(discriminates_body);
}

fn discriminates_body() {
    let mut entry_point_differs = 0;
    let mut std_globals_differs = 0;
    let mut total = 0;
    for (name, src) in corpus("sema_corpus") {
        let Some(flags) = Flags::parse(&src) else {
            continue;
        };
        if PARSER_ENTRY_SKIP.contains(&name.as_str()) {
            continue;
        }
        total += 1;
        let compile = facade(&name, &src, flags, false);
        let parser = facade(&name, &src, flags, true);
        if compile.dump != parser.dump {
            entry_point_differs += 1;
        }
        let no_globals = facade(
            &name,
            &src,
            Flags {
                std_globals: false,
                ..flags
            },
            false,
        );
        if compile.dump != no_globals.dump {
            std_globals_differs += 1;
        }
    }
    assert_eq!(total, 218);
    // Most files differ between the entry points (the compile path folds
    // constants, rewrites arrows, and rejects what it cannot compile).
    assert!(
        entry_point_differs > 100,
        "only {entry_point_differs} of {total} files distinguish the two \
         entry points; the sweeps would be near-vacuous"
    );
    // And nearly all of them differ on the ambient declarations, which are
    // 63 extra `Decl` lines in the dump.
    assert!(
        std_globals_differs > 100,
        "only {std_globals_differs} of {total} files distinguish \
         std_globals on from off"
    );
}
