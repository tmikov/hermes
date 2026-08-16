/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The Tier 2 wide sweep: round-trip **every** `.js` file in the Hermes lit
//! test tree through `hermes-gen-js` and compare the ESTree dumps.
//!
//! This is a development-time tool, not a standing test, and that is why it
//! lives in the unpublished `tools` crate: it reads `test/` at the repository
//! root, a directory a published crate cannot assume exists. The Tier 1 gate
//! (`crates/gen_js/tests/corpus.rs`) covers the 420 checked-in parser/sema
//! corpus files and runs on every `cargo test`; this sweeps ~1900 files that
//! were written to exercise the *compiler*, in every dialect the parser
//! supports, and reports what breaks.
//!
//! ## The oracle
//!
//! Identical to Tier 1's, deliberately: parse, generate under both
//! [`Pretty`] modes, reparse the generated text with the *same* flags, and
//! require [`ParsedJS::to_estree_json_with`] with [`ESTreeRawProp::Exclude`]
//! and [`LocationDumpMode::None`] to match byte for byte. `"raw"` is verbatim
//! source text for a numeric literal, so no correct generator preserves it;
//! locations change by construction. Nothing else is normalized.
//!
//! ## Parse flags come from the lit `RUN:` lines
//!
//! A file in `test/` carries its dialect in the `hermesc`/`hermes` command
//! line of its `RUN:` directives, so this tool reads those rather than
//! guessing from the path (`test/flow/` is not the only place Flow syntax
//! appears). A `// FLAGS:` line, the convention the corpus directories use,
//! is honored too. Unrecognized options are ignored — unlike the Tier 1
//! gate, which panics on them: a lit `RUN:` line is a whole compiler
//! invocation with dozens of codegen options, and only the handful that
//! change the *grammar* matter here.
//!
//! ## Panics are data
//!
//! The parser under sweep has at least one known panic and a recursion-depth
//! stack hazard, and the point of a 1900-file sweep is to find more. Each
//! file's work runs inside [`std::panic::catch_unwind`] so one bad file is a
//! reported outcome rather than the end of the run, and the whole sweep runs
//! on a 64 MiB stack because deeply-nested fixtures overflow the default one
//! before any of this crate's code executes.
//!
//! ## Usage
//!
//! ```text
//! cargo run -p tools --bin gen-js-sweep --release -- [TEST_ROOT] [--kinds-only|--failures-only]
//! ```
//!
//! `TEST_ROOT` defaults to `test/` at the repository root. Output is
//! tab-separated on stdout; progress, if any, goes to stderr.

use std::collections::HashMap;
use std::io::Write;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};

use hermes_ast::dump::{ESTreeDumpMode, ESTreeRawProp, LocationDumpMode};
use hermes_ast::node::{Node, NodeKind};
use hermes_ast::visitor::Visitor;
use hermes_gen_js::{generate, Opt, Pretty};
use hermes_parser::{ParseFlags, ParsedJS};

/// The generated AST source, baked in at compile time.
///
/// Used only to recover the **names of all 271 [`NodeKind`]s**, so that kinds
/// the sweep never saw can be printed as explicit zero rows rather than
/// silently omitted. `NodeKind` is `#[repr(u32)]` with interleaved
/// `_Name_First`/`_Last` range sentinels and offers no iterator, and this
/// crate forbids `unsafe_code`, so transmuting a range of discriminants is
/// not an option; the `Node::kind()` match in the generated file is the one
/// place that lists exactly the real kinds and nothing else.
const NODE_RS: &str = include_str!("../../../ast/src/node.rs");

/// Every [`NodeKind`] name, in `ESTree.def` order.
///
/// Extracted from the `Node::X(_) => NodeKind::X,` arms of `Node::kind()` in
/// [`NODE_RS`]. Panics unless it finds exactly 271, the count the generator's
/// exhaustiveness argument rests on (`crates/gen_js/tests/exhaustive.rs`), so
/// that an AST change makes this tool complain instead of quietly reporting a
/// short table.
fn all_kind_names() -> Vec<String> {
    const MARKER: &str = "=> NodeKind::";
    let mut names = Vec::new();
    for line in NODE_RS.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("Node::") {
            continue;
        }
        let Some(pos) = trimmed.find(MARKER) else {
            continue;
        };
        let rest = &trimmed[pos + MARKER.len()..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            names.push(name);
        }
    }
    names.sort();
    names.dedup();
    assert_eq!(
        names.len(),
        271,
        "expected 271 NodeKinds in the generated AST; the extraction in \
         `all_kind_names` is out of date"
    );
    names
}

/// Tallies how many times each [`NodeKind`] occurs in a tree.
///
/// Counting on the **original** parse rather than on the generator's output
/// is what makes the number mean "a tree containing this kind was handed to
/// the generator": the generator has no per-kind counter of its own, and a
/// kind that it dropped on the floor would be invisible in a tally taken
/// downstream of it — the exact bug the sweep is looking for.
#[derive(Default)]
struct KindTally {
    /// Occurrences per kind.
    counts: HashMap<NodeKind, u64>,
}

impl<'gc> Visitor<'gc> for KindTally {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        *self.counts.entry(node.kind()).or_insert(0) += 1;
        node.visit_children(self);
    }
}

/// Finds cover-grammar nodes: the parser's placeholders for syntax that is
/// only legal inside arrow parameters or a destructuring target, left in the
/// tree for sema to reject.
///
/// Copied from `crates/gen_js/tests/corpus.rs`, which explains at length why a
/// tree containing one is classified as "not a program" rather than as a
/// generator failure: these 5 kinds are 5 of the 8 that report
/// `GenJsError::UnsupportedKind` by design.
#[derive(Default)]
struct CoverFinder {
    /// The first cover kind seen, as a name for the report.
    found: Option<&'static str>,
}

impl<'gc> Visitor<'gc> for CoverFinder {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        let name = match node {
            Node::CoverEmptyArgs(_) => Some("CoverEmptyArgs"),
            Node::CoverInitializer(_) => Some("CoverInitializer"),
            Node::CoverRestElement(_) => Some("CoverRestElement"),
            Node::CoverTrailingComma(_) => Some("CoverTrailingComma"),
            Node::CoverTypedIdentifier(_) => Some("CoverTypedIdentifier"),
            _ => None,
        };
        if name.is_some() && self.found.is_none() {
            self.found = name;
        }
        node.visit_children(self);
    }
}

/// The ESTree dump used as the round-trip oracle: pretty-printed JSON, empty
/// fields hidden, no locations, and no `"raw"`.
fn ast_json(parsed: &mut ParsedJS) -> String {
    parsed.to_estree_json_with(
        true,
        ESTreeDumpMode::HideEmpty,
        LocationDumpMode::None,
        ESTreeRawProp::Exclude,
    )
}

/// Generate `parsed`'s program under `pretty`.
fn gen(parsed: &mut ParsedJS, pretty: Pretty) -> Result<String, String> {
    let mut out = Vec::new();
    let res = parsed.with_program(|gc, root| {
        generate(
            &mut out,
            gc,
            root,
            Opt {
                pretty,
                ..Opt::default()
            },
        )
    });
    match res {
        Ok(()) => String::from_utf8(out).map_err(|e| format!("non-UTF-8 output: {e}")),
        Err(e) => Err(format!("generation failed: {e:?}")),
    }
}

/// The first differing line of two ESTree dumps, with a little context, so a
/// failure names the divergent node instead of dumping two multi-thousand-line
/// JSON blobs. Copied from `crates/gen_js/tests/corpus.rs`, flattened onto one
/// line because this tool's output is tab-separated records.
fn first_json_difference(before: &str, after: &str) -> String {
    let (mut b_lines, mut a_lines) = (before.lines(), after.lines());
    let mut i = 0usize;
    loop {
        i += 1;
        match (b_lines.next(), a_lines.next()) {
            (Some(b), Some(a)) if b == a => continue,
            (Some(b), Some(a)) => {
                return format!(
                    "first difference at dump line {i}: original: {} | regenerated: {}",
                    b.trim(),
                    a.trim()
                )
            }
            (Some(b), None) => {
                return format!(
                    "regenerated dump ENDS at line {}; original continues: {}",
                    i - 1,
                    b.trim()
                )
            }
            (None, Some(a)) => {
                return format!(
                    "original dump ENDS at line {}; regenerated continues: {}",
                    i - 1,
                    a.trim()
                )
            }
            (None, None) => return "dumps are identical (should not happen here)".to_string(),
        }
    }
}

/// Plain JavaScript: the flags every file starts from.
const PLAIN: ParseFlags = ParseFlags {
    parse_flow: false,
    parse_flow_component_syntax: false,
    parse_flow_records: false,
    parse_flow_match: false,
    parse_ts: false,
    parse_jsx: false,
    strict_mode: false,
};

/// Infer a file's dialect from its lit `RUN:` directives and any `// FLAGS:`
/// line.
///
/// Every option that changes the *grammar* the parser applies is honored;
/// everything else in a `RUN:` line — optimization levels, dump selectors,
/// output paths, `FileCheck` invocations — is ignored, because a lit command
/// line is a whole compiler invocation and only these few options reach the
/// parser. A file whose `RUN:` lines disagree gets the union: `test/flow` in
/// particular runs several `hermesc` passes over one source, and the source is
/// Flow for all of them.
///
/// The two implicit ones matter more than the explicit ones. `-typed` selects
/// Hermes's typed dialect, and the driver derives *two* parser settings from
/// it: Flow parsing unless a type dialect was named explicitly
/// (`lib/CompilerDriver/CompilerDriver.cpp:1290-1296`) and unconditional
/// strict mode (`CompilerDriver.cpp:1235`,
/// `tools/shermes/shermes.cpp:662`). Note the Flow default is `hermesc`'s
/// alone: `shermes` rejects `-typed` without an explicit dialect rather than
/// picking one (`shermes.cpp:707-710`), so it is the `hermesc` behavior that
/// governs the lit files this sweep reads. Nearly 300 files in
/// `test/Sema/flow`, `test/hermes/flow` and `test/IRGen/flow` are Flow source
/// whose `RUN:` line never says `-parse-flow`; without this they would all be
/// counted as unparseable and dropped from the sweep.
fn flags_from_source(src: &str) -> ParseFlags {
    let mut flags = PLAIN;
    // `-typed` and the strict-mode pair are resolved after the scan, because
    // the driver combines them rather than letting the last one on the line
    // win: `setStrictMode((!NonStrictMode && StrictMode) || Typed)`.
    let (mut typed, mut strict, mut non_strict) = (false, false, false);
    for line in src.lines() {
        // Accept `// RUN:`, `# RUN:`, `; RUN:` and the `RUN: %hermes ...`
        // continuation form; lit only requires the marker to appear on the
        // line at all.
        let body = match (line.find("RUN:"), line.find("FLAGS:")) {
            (Some(i), _) => &line[i + "RUN:".len()..],
            (None, Some(i)) => &line[i + "FLAGS:".len()..],
            (None, None) => continue,
        };
        for tok in body.split_whitespace() {
            // Normalize `--flag` to `-flag` and drop any `=value` suffix, so
            // `-parse-flow=true` and `--parse-flow` are the same token.
            // Only options are interesting, and an option starts with a dash.
            // Requiring it keeps a bare word in a path or a `FileCheck`
            // prefix from being read as a dialect switch. `--flag` and
            // `-flag` are the same option to Hermes's option parser, and a
            // `=value` suffix is dropped.
            let Some(tok) = tok.strip_prefix('-') else {
                continue;
            };
            let tok = tok.strip_prefix('-').unwrap_or(tok);
            let tok = tok.split('=').next().unwrap_or(tok);
            match tok {
                "parse-flow" => flags.parse_flow = true,
                "Xparse-flow-match" => {
                    flags.parse_flow = true;
                    flags.parse_flow_match = true;
                }
                "Xparse-component-syntax" => {
                    flags.parse_flow = true;
                    flags.parse_flow_component_syntax = true;
                }
                "Xparse-flow-records" => {
                    flags.parse_flow = true;
                    flags.parse_flow_records = true;
                }
                "parse-ts" => flags.parse_ts = true,
                "parse-jsx" => flags.parse_jsx = true,
                "typed" => typed = true,
                "strict" => strict = true,
                "non-strict" => non_strict = true,
                _ => {}
            }
        }
    }
    // Typed mode defaults the type dialect to Flow, and forces strict mode.
    if typed && !flags.parse_flow && !flags.parse_ts {
        flags.parse_flow = true;
    }
    flags.strict_mode = typed || (strict && !non_strict);
    // Flow and TypeScript are mutually exclusive dialects; no file in the
    // tree asks for both, but if one ever does, Flow wins (it is the dialect
    // Hermes actually ships) rather than the parser being handed a
    // contradiction.
    if flags.parse_flow {
        flags.parse_ts = false;
    }
    flags
}

/// What happened to one file.
enum Outcome {
    /// The bytes on disk are not UTF-8, so there is nothing to parse.
    NotUtf8(String),
    /// The parser rejected it: no tree, nothing to generate from.
    Unparseable(String),
    /// It parsed, but its tree holds a cover-grammar node, so it is not a
    /// program. Carries the kind name.
    Cover(&'static str),
    /// It was round-tripped. Carries one message per failing round trip
    /// (empty when both modes agreed).
    RoundTripped {
        /// One entry per failing `(file, Pretty)` pair.
        failures: Vec<String>,
        /// Round trips attempted for this file (2 unless something aborted).
        attempted: usize,
    },
}

/// Parse `path`, then round-trip it under both [`Pretty`] modes, tallying the
/// kinds of the original tree.
///
/// Returns the outcome and the tally; the tally is empty unless a tree was
/// actually handed to the generator, so the per-kind table reports coverage of
/// the generator rather than of the parser.
///
/// With `show`, the inferred flags and the generated text of each mode are
/// echoed to stderr. That is the single-file investigation mode: a sweep
/// failure names a file and a dump line, and the next question is always
/// "what did it actually print?".
fn process_file(path: &Path, show: bool) -> (Outcome, HashMap<NodeKind, u64>) {
    let empty = HashMap::new();
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => return (Outcome::NotUtf8(e.to_string()), empty),
    };
    let flags = flags_from_source(&src);
    if show {
        eprintln!("flags: {flags:?}");
    }

    let mut parsed = match hermes_parser::parse(&src, flags) {
        Ok(p) => p,
        Err(e) => return (Outcome::Unparseable(format!("{e:?}")), empty),
    };

    let cover = parsed.with_program(|_gc, root| {
        let mut finder = CoverFinder::default();
        finder.visit_node(root);
        finder.found
    });
    if let Some(kind) = cover {
        return (Outcome::Cover(kind), empty);
    }

    let counts = parsed.with_program(|_gc, root| {
        let mut tally = KindTally::default();
        tally.visit_node(root);
        tally.counts
    });

    let before = ast_json(&mut parsed);
    let mut failures = Vec::new();
    let mut attempted = 0usize;
    for pretty in [Pretty::Yes, Pretty::No] {
        attempted += 1;
        let js = match gen(&mut parsed, pretty) {
            Ok(js) => js,
            Err(e) => {
                failures.push(format!("{pretty:?}\t{e}"));
                continue;
            }
        };
        if show {
            eprintln!("--- generated [{pretty:?}] ---\n{js}--- end ---");
        }
        let mut reparsed = match hermes_parser::parse(&js, flags) {
            Ok(p) => p,
            Err(e) => {
                failures.push(format!(
                    "{pretty:?}\tgenerated source does not parse: {e:?}"
                ));
                continue;
            }
        };
        let after = ast_json(&mut reparsed);
        if after != before {
            failures.push(format!(
                "{pretty:?}\treparses to a DIFFERENT AST: {}",
                first_json_difference(&before, &after)
            ));
        }
    }
    (
        Outcome::RoundTripped {
            failures,
            attempted,
        },
        counts,
    )
}

/// Every `.js` file under `root`, recursively, sorted by path.
///
/// A `root` that is itself a file is the whole list, whatever its extension:
/// that is the single-file investigation mode.
fn js_files(root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        return vec![root.to_path_buf()];
    }
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("warning: cannot read {}: {e}", dir.display());
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                // Symlinks are not followed: the tree has none today, and
                // following them risks counting a file twice or looping.
                Ok(ft) if ft.is_dir() => stack.push(path),
                Ok(ft) if ft.is_file() => {
                    if path.extension().is_some_and(|x| x == "js") {
                        out.push(path);
                    }
                }
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// Turn a [`std::panic::catch_unwind`] payload into a one-line message.
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Flatten a message onto one line, so every record is one output line.
fn one_line(s: &str) -> String {
    s.replace('\n', " ").replace('\r', "")
}

/// Which sections to print.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Report {
    /// Everything (the default).
    Full,
    /// Only the per-kind table.
    KindsOnly,
    /// Only the summary and the failure records.
    FailuresOnly,
}

/// The accumulated result of a whole sweep.
#[derive(Default)]
struct Totals {
    /// `(path, reason)` for each file that is not UTF-8.
    not_utf8: Vec<(String, String)>,
    /// `(path, reason)` for each file the parser rejected.
    unparseable: Vec<(String, String)>,
    /// `(path, kind)` for each file whose tree holds a cover node.
    cover: Vec<(String, String)>,
    /// `(path, message)` for each file that panicked.
    panicked: Vec<(String, String)>,
    /// `(path, mode-and-reason)` for each failing round trip.
    failures: Vec<(String, String)>,
    /// Files that reached the round-trip stage.
    round_tripped_files: usize,
    /// Round trips attempted.
    round_trips: usize,
    /// Occurrences per kind across every tree handed to the generator.
    kinds: HashMap<NodeKind, u64>,
}

/// Run the sweep over `root` and return the totals.
///
/// `show` echoes each file's generated text to stderr; it is meant for a
/// `root` that names a single file.
fn sweep(root: &Path, show: bool) -> (Vec<PathBuf>, Totals) {
    let files = js_files(root);
    let mut totals = Totals::default();

    // The parser panics on at least one input in this tree, and the default
    // hook would print a multi-line backtrace notice per panicking file into
    // the middle of the report. Panics are captured and reported as records
    // instead; restore the hook when the sweep is over.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    for path in &files {
        // Report paths relative to the root, so the records are stable across
        // checkouts. In single-file mode `strip_prefix` leaves nothing, so
        // fall back to the path as given.
        let stripped = path.strip_prefix(root).unwrap_or(path);
        let rel = if stripped.as_os_str().is_empty() {
            path.to_string_lossy().to_string()
        } else {
            stripped.to_string_lossy().to_string()
        };
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| process_file(path, show)));
        match result {
            Err(payload) => totals
                .panicked
                .push((rel, one_line(&panic_message(payload)))),
            Ok((outcome, counts)) => {
                for (kind, n) in counts {
                    *totals.kinds.entry(kind).or_insert(0) += n;
                }
                match outcome {
                    Outcome::NotUtf8(e) => totals.not_utf8.push((rel, one_line(&e))),
                    Outcome::Unparseable(e) => totals.unparseable.push((rel, one_line(&e))),
                    Outcome::Cover(kind) => totals.cover.push((rel, kind.to_string())),
                    Outcome::RoundTripped {
                        failures,
                        attempted,
                    } => {
                        totals.round_tripped_files += 1;
                        totals.round_trips += attempted;
                        for f in failures {
                            totals.failures.push((rel.clone(), one_line(&f)));
                        }
                    }
                }
            }
        }
    }

    std::panic::set_hook(previous_hook);
    (files, totals)
}

/// Print the report to stdout.
fn print_report(root: &Path, files: &[PathBuf], totals: &Totals, report: Report) {
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    if report != Report::KindsOnly {
        let _ = writeln!(out, "# gen_js Tier 2 sweep");
        let _ = writeln!(out, "root\t{}", root.display());
        let _ = writeln!(out, "files\t{}", files.len());
        let _ = writeln!(out, "round-tripped-files\t{}", totals.round_tripped_files);
        let _ = writeln!(out, "round-trips\t{}", totals.round_trips);
        let _ = writeln!(out, "skipped-unparseable\t{}", totals.unparseable.len());
        let _ = writeln!(out, "skipped-cover\t{}", totals.cover.len());
        let _ = writeln!(out, "not-utf8\t{}", totals.not_utf8.len());
        let _ = writeln!(out, "panicked\t{}", totals.panicked.len());
        let _ = writeln!(out, "failures\t{}", totals.failures.len());

        let _ = writeln!(out, "\n## failures ({})", totals.failures.len());
        for (path, reason) in &totals.failures {
            let _ = writeln!(out, "FAIL\t{path}\t{reason}");
        }

        let _ = writeln!(out, "\n## panicked ({})", totals.panicked.len());
        for (path, msg) in &totals.panicked {
            let _ = writeln!(out, "PANIC\t{path}\t{msg}");
        }
    }

    if report == Report::Full {
        let _ = writeln!(out, "\n## unparseable ({})", totals.unparseable.len());
        for (path, reason) in &totals.unparseable {
            let _ = writeln!(out, "UNPARSEABLE\t{path}\t{reason}");
        }

        let _ = writeln!(out, "\n## cover-grammar ({})", totals.cover.len());
        for (path, kind) in &totals.cover {
            let _ = writeln!(out, "COVER\t{path}\t{kind}");
        }

        let _ = writeln!(out, "\n## not-utf8 ({})", totals.not_utf8.len());
        for (path, reason) in &totals.not_utf8 {
            let _ = writeln!(out, "NOTUTF8\t{path}\t{reason}");
        }
    }

    if report != Report::FailuresOnly {
        // Name every kind, including the ones the sweep never saw: a zero row
        // is the point of the table.
        let mut by_name: HashMap<String, u64> =
            all_kind_names().into_iter().map(|n| (n, 0u64)).collect();
        for (kind, n) in &totals.kinds {
            let name = format!("{kind:?}");
            match by_name.get_mut(&name) {
                Some(slot) => *slot += n,
                None => panic!("kind {name} is not in the extracted kind list"),
            }
        }
        let mut rows: Vec<(String, u64)> = by_name.into_iter().collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let covered = rows.iter().filter(|(_, n)| *n > 0).count();
        let _ = writeln!(
            out,
            "\n## kinds ({covered} of {} with count >= 1)",
            rows.len()
        );
        for (name, n) in &rows {
            let _ = writeln!(out, "KIND\t{name}\t{n}");
        }
    }
    let _ = out.flush();
}

/// Parse the command line, run the sweep on a big stack, print the report.
fn main() {
    let mut root: Option<PathBuf> = None;
    let mut report = Report::Full;
    let mut show = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--kinds-only" => report = Report::KindsOnly,
            "--failures-only" => report = Report::FailuresOnly,
            "--show-generated" => show = true,
            "--help" | "-h" => {
                println!(
                    "usage: gen-js-sweep [TEST_ROOT] \
                     [--kinds-only|--failures-only] [--show-generated]\n\
                     TEST_ROOT defaults to the repository's test/ directory, \
                     and may name a single file.\n\
                     --show-generated echoes each file's generated text to \
                     stderr (for the single-file case)."
                );
                return;
            }
            other if other.starts_with('-') => {
                eprintln!("unknown option {other}");
                std::process::exit(2);
            }
            other => root = Some(PathBuf::from(other)),
        }
    }
    // `CARGO_MANIFEST_DIR` is `rust/crates/tools`; the lit tree is `test/` at
    // the repository root, three levels up.
    let root = root.unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("test")
    });

    // 64 MiB, for the same reason `crates/gen_js/tests/corpus.rs` does it:
    // the lit tree has deeply-nested fixtures whose recursive-descent parse
    // overflows a smaller stack, and a stack overflow is an abort that
    // `catch_unwind` cannot turn into a record.
    let sweep_root = root.clone();
    let (files, totals) = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || sweep(&sweep_root, show))
        .expect("spawn the sweep thread")
        .join()
        .expect("the sweep thread itself panicked");

    print_report(&root, &files, &totals, report);
    eprintln!(
        "swept {} files: {} round trips, {} failures, {} panicked",
        files.len(),
        totals.round_trips,
        totals.failures.len(),
        totals.panicked.len()
    );
}
