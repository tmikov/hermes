/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The Tier 1 corpus gate: every checked-in parser/sema corpus file, parsed,
//! regenerated, and reparsed, with the two ASTs required to be identical.
//!
//! This is Task 15, Step 3 of
//! `doc/superpowers/plans/2026-08-15-gen-js-port.md`. Up to here every test
//! in this crate was written by the same person who wrote the arm under
//! test, from the same reading of juno and the grammar; a blind spot shared
//! by the arm and its test is invisible to all of them. The 420 corpus files
//! were written for other purposes entirely (the parser differential, the
//! sema differential), by people who had never heard of this generator, and
//! they are real Hermes/Flow/TypeScript code. They are the first adversarial
//! input this crate meets.
//!
//! ## What "the same AST" means here
//!
//! The oracle is [`ParsedJS::to_estree_json_with`] with
//! [`ESTreeRawProp::Exclude`] — the ESTree dump with the `"raw"` property
//! omitted, and without locations. `"raw"` is verbatim source text for a
//! numeric literal (`0x10`, `1_000`, `50.`), so it necessarily changes when
//! the literal is reprinted from its `f64` value; no correct generator can
//! preserve it, and C++ `AST2JS`'s own round-trip harness normalizes it away
//! the same way (`-Xinclude-raw-ast-prop=0`). Locations are dropped for the
//! same reason at a coarser grain: regenerated text has different offsets by
//! construction. **Everything else is compared byte for byte**, including
//! every node kind, every field, and the whole tree shape. This is the only
//! normalization applied anywhere in this file — there is no skip list, no
//! per-file exception, and no narrowing of the comparison.
//!
//! ## Failure reporting
//!
//! Each directory is one `#[test]`, and each test accumulates **all** its
//! failures before asserting, naming the file and the [`Pretty`] mode. A
//! gate that aborted on the first failure would take one round trip through
//! the whole suite per bug.
//!
//! ## The two things that are not programs
//!
//! 420 files go in; 392 are round-tripped, in both modes, for 784 round
//! trips. The 28 that are not fall into exactly two structurally-detected
//! buckets, and each bucket is **pinned per directory as a sorted list of
//! file names** (see [`Expected`]) so that a new member — a parser
//! regression, a corpus file that stops parsing — fails this gate instead of
//! quietly shrinking its coverage. Names rather than counts, deliberately: a
//! count lets one file drop out while another drops in and keeps the gate
//! green, with the enumeration below going stale unnoticed.
//!
//! **25 files the parser rejects.** There is no tree to generate from.
//! Every one is an intentional error fixture, and says so in its name: 23 in
//! `sema_corpus` (`await-get-error.js`, `class-static-block-return-error.js`,
//! `class-static-block-yield-error.js`, `declare-error.js`,
//! `decorator-error.js`, `error-expected-cross-line-note.js`,
//! `error-expected-same-line.js`, `error-in-decl-rest-property.js`,
//! `error-return-outside-function.js`,
//! `flow-match-pattern-binding-error.js`,
//! `flow-match-pattern-object-binding-error.js`,
//! `flow-match-pattern-object-value-error.js`, `for-of-error.js`,
//! `if-function-gen-error.js`, `jsx-error-attr-member.js`,
//! `method-type-error.js`, `nested-expressions.js`,
//! `nested-tagged-template-limit.js`, `nested-unary-multichar-limit.js`,
//! `parse-error.js`, `using-declaration-pattern-error.js`,
//! `yield-field-error.js`, `yield-paren-error.js`) and 2 in
//! `sema_corpus_parser` (`parse-error-no-ast.js`,
//! `parse-error-recoverable.js`). The three `nested-*` files are the
//! *error* side of the recursion-depth boundary whose clean side is
//! `parser_corpus/nested-parens-limit.js`, which is in the sweep.
//!
//! **3 files whose tree holds a cover-grammar node** — `cover_init.js`,
//! `error-cover-nodes.js`, `flow-typecast-cover.js`. See
//! [`contains_cover_node`]: these parse, but what they parse to is not a
//! program.

use std::path::{Path, PathBuf};

use hermes_ast::dump::{ESTreeDumpMode, ESTreeRawProp, LocationDumpMode};
use hermes_ast::node::Node;
use hermes_ast::visitor::Visitor;
use hermes_gen_js::{generate, Opt, Pretty};
use hermes_parser::{ParseFlags, ParsedJS};

/// Finds cover-grammar nodes: the parser's placeholders for syntax that is
/// only legal inside arrow parameters or a destructuring target, left in the
/// tree for sema to reject.
///
/// See [`contains_cover_node`] for why this gate treats a tree containing one
/// as "not a program" rather than as a generator failure.
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

/// The name of the first cover-grammar node in `parsed`'s tree, if any.
///
/// A tree containing one of these is **not a JavaScript program**. The parser
/// builds a `Cover*` node when it reads something that would only be legal as
/// an arrow parameter or a destructuring target and then finds it was
/// neither; the node survives into the tree precisely so that sema can report
/// the error with a good location. `({a = 1});` is the canonical case — it is a
/// SyntaxError in every conforming engine, and the corpus files that produce
/// these nodes exist to pin exactly that rejection
/// (`sema_corpus/error-cover-nodes.js`: "leaving the rejection to sema";
/// `sema_corpus/flow-typecast-cover.js`: "`(x?: number);` errors here").
///
/// The generator's documented domain excludes these 5 kinds — they are 5 of
/// the 7 internal kinds that report `GenJsError::UnsupportedKind` by design
/// (`src/dispatch.rs:86-96`, spec §4) — so "generation refused" is the
/// correct outcome here, not a bug. Rather than name the three affected
/// files in a skip list, this gate detects the condition **structurally**,
/// so that a future corpus file with the same shape is classified the same
/// way and, more importantly, a file that stops containing a cover node is
/// pulled back into the sweep automatically.
fn contains_cover_node(parsed: &mut ParsedJS) -> Option<&'static str> {
    parsed.with_program(|_gc, root| {
        let mut finder = CoverFinder::default();
        finder.visit_node(root);
        finder.found
    })
}

/// The ESTree dump used as the round-trip oracle: pretty-printed JSON,
/// empty fields hidden, no locations, and **no `"raw"`** (see the module
/// doc comment for why that one property is dropped).
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

/// Every `.js` file directly in `dir` (relative to the crate root's parent,
/// i.e. `rust/crates`), sorted by name so the report order is stable.
fn corpus_files(dir: &str) -> Vec<PathBuf> {
    // `CARGO_MANIFEST_DIR` is `rust/crates/gen_js`; the corpora live in
    // sibling crates, so go up one level.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("gen_js crate has a parent directory");
    let full = root.join(dir);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&full)
        .unwrap_or_else(|e| panic!("cannot read corpus directory {}: {e}", full.display()))
        .map(|e| e.expect("directory entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "js"))
        .collect();
    files.sort();
    files
}

/// Read a corpus file's first-line `// FLAGS:` comment into [`ParseFlags`],
/// mirroring `crates/sema/tests/facade_agreement.rs:74-107`.
///
/// Flags that do not affect parsing (`-fno-std-globals`, `-ferror-limit=0`,
/// `-enable-eval=false`) are accepted as no-ops: they select sema behavior,
/// and this gate never runs sema. An unrecognized flag panics rather than
/// being ignored, so that a corpus file added with a new dialect flag is
/// noticed here instead of being round-tripped under the wrong grammar.
fn flags_from_source(path: &Path, src: &str, base: ParseFlags) -> ParseFlags {
    let mut flags = base;
    // Scan for `// FLAGS:` ANYWHERE, not just on line 1. Every corpus file
    // today puts it first, and both `sema_differential` and
    // `facade_agreement.rs:78-80` read only the first line — but those two
    // compare a Rust run against a C++ run of the *same* file, so a missed
    // flag makes both sides wrong identically and the comparison still holds.
    // Here a missed flag would silently round-trip the file under the wrong
    // grammar and quietly weaken the gate, so a `// FLAGS:` line that this
    // function would have ignored is a hard error instead.
    let mut found: Option<&str> = None;
    for line in src.split('\n') {
        let line = line.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix("// FLAGS: ") {
            assert!(
                found.is_none(),
                "{}: two `// FLAGS:` lines; this gate applies exactly one",
                path.display()
            );
            found = Some(rest);
        }
    }
    let Some(rest) = found else {
        return flags;
    };
    for arg in rest.split_whitespace() {
        match arg {
            "-parse-flow" | "--parse-flow" => flags.parse_flow = true,
            "--Xparse-flow-match" | "-Xparse-flow-match" => {
                flags.parse_flow = true;
                flags.parse_flow_match = true;
            }
            "-parse-jsx" | "--parse-jsx" => flags.parse_jsx = true,
            "-parse-ts" | "--parse-ts" => flags.parse_ts = true,
            // Sema-only or driver-only; no effect on the grammar.
            "-fno-std-globals" | "-ferror-limit=0" | "-enable-eval=false" => {}
            other => panic!(
                "{}: unrecognized `// FLAGS:` argument {other:?} — teach \
                 `flags_from_source` about it rather than letting the file \
                 round-trip under the wrong grammar",
                path.display()
            ),
        }
    }
    flags
}

/// What one directory's sweep is expected to cover: how many `.js` files it
/// holds, and how many of those are not round-trippable programs. All three
/// are pinned so that coverage cannot silently shrink.
#[derive(Clone, Copy)]
struct Expected {
    /// Total `.js` files in the directory.
    files: usize,
    /// Exactly which files fail to parse under the directory's flags, sorted
    /// by name: an error fixture has no tree to generate from.
    ///
    /// A **name list**, not a count. A count would let one file start failing
    /// to parse while another started parsing and keep the gate green, with
    /// the module doc comment's enumeration silently going stale — the whole
    /// point of pinning this is that the excluded set is auditable.
    unparseable: &'static [&'static str],
    /// Exactly which files parse but yield a tree containing a cover-grammar
    /// node, i.e. a tree that is not a program, sorted by name and tagged
    /// with the kind found. See [`contains_cover_node`]. A name list for the
    /// same reason as [`Self::unparseable`].
    cover: &'static [&'static str],
}

/// Run `f` on a thread with a 64 MiB stack.
///
/// `parser_corpus/nested-parens-limit.js` is 125 nested parentheses, which is
/// deliberately just under the parser's recursion limit; a debug-build
/// recursive-descent parse of it overflows the 2 MiB stack libtest gives a
/// test thread, before this crate's code ever runs. The in-tree drivers never
/// hit it because a process's main thread starts with 8 MiB. Same fix, same
/// reason, as `sema/tests/facade_agreement.rs:328-336`,
/// `parser/tests/recursion_depth_limit.rs:102`, and
/// `parser/tests/lazy_reparse.rs:605`.
fn on_big_stack<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("the sweep panicked (see the assertion above)")
}

/// Run the round-trip sweep over `dir` with `base` flags, per-file `// FLAGS:`
/// overrides applied on top, and both [`Pretty`] modes.
///
/// Accumulates every failure and asserts once at the end.
fn run_corpus(dir: &'static str, base: ParseFlags, expected: Expected) {
    on_big_stack(move || run_corpus_inner(dir, base, expected))
}

/// [`run_corpus`]'s body, run on the big stack.
fn run_corpus_inner(dir: &str, base: ParseFlags, expected: Expected) {
    let files = corpus_files(dir);
    assert_eq!(
        files.len(),
        expected.files,
        "{dir}: corpus size changed; update the pinned count"
    );

    let mut failures: Vec<String> = Vec::new();
    let mut unparseable: Vec<String> = Vec::new();
    let mut cover: Vec<String> = Vec::new();
    let mut compared = 0usize;

    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            // Not all corpus files are valid UTF-8 (the lexer is byte
            // oriented and some fixtures carry raw bytes on purpose).
            Err(e) => {
                unparseable.push(format!("{name}: not UTF-8 ({e})"));
                continue;
            }
        };
        let flags = flags_from_source(path, &src, base);

        let mut parsed = match hermes_parser::parse(&src, flags) {
            Ok(p) => p,
            Err(_) => {
                unparseable.push(name);
                continue;
            }
        };
        if let Some(kind) = contains_cover_node(&mut parsed) {
            cover.push(format!("{name} ({kind})"));
            continue;
        }
        let before = ast_json(&mut parsed);

        for pretty in [Pretty::Yes, Pretty::No] {
            compared += 1;
            let js = match gen(&mut parsed, pretty) {
                Ok(js) => js,
                Err(e) => {
                    failures.push(format!("{name} [{pretty:?}]: {e}"));
                    continue;
                }
            };
            let mut reparsed = match hermes_parser::parse(&js, flags) {
                Ok(p) => p,
                Err(e) => {
                    failures.push(format!(
                        "{name} [{pretty:?}]: generated source does not parse: {e:?}"
                    ));
                    continue;
                }
            };
            let after = ast_json(&mut reparsed);
            if after != before {
                failures.push(format!(
                    "{name} [{pretty:?}]: reparses to a DIFFERENT AST{}",
                    first_json_difference(&before, &after)
                ));
            }
        }
    }

    // Round-trip failures are reported BEFORE the coverage pin: a
    // never-before-seen unparseable file is a curiosity, a broken round trip
    // is the finding this gate exists for, and whichever assertion fires
    // first is the only one anybody reads.
    assert!(
        failures.is_empty(),
        "{dir}: {} of {} round trips failed:\n{}",
        failures.len(),
        compared,
        failures.join("\n")
    );

    unparseable.sort();
    assert_eq!(
        unparseable, expected.unparseable,
        "{dir}: the set of files that fail to parse changed"
    );

    cover.sort();
    assert_eq!(
        cover, expected.cover,
        "{dir}: the set of files holding a cover-grammar node changed"
    );

    assert_eq!(
        compared,
        2 * (files.len() - unparseable.len() - cover.len()),
        "{dir}: every round-trippable file must be tried in both Pretty modes"
    );

    eprintln!(
        "gen_js corpus ({dir}): {} files, {} round trips, \
         {} unparseable, {} cover-grammar",
        files.len(),
        compared,
        unparseable.len(),
        cover.len()
    );
}

/// The first differing line of two ESTree dumps, with a little context, so a
/// failure message points at the divergent node instead of dumping two
/// multi-thousand-line JSON blobs.
fn first_json_difference(before: &str, after: &str) -> String {
    let (mut b_lines, mut a_lines) = (before.lines(), after.lines());
    let mut i = 0usize;
    loop {
        i += 1;
        // `zip` would stop at the shorter side and report nothing but two
        // lengths when one dump is a strict prefix of the other — which is
        // exactly what a dropped trailing node looks like. Step both
        // iterators by hand so the first *extra* line is named too.
        match (b_lines.next(), a_lines.next()) {
            (Some(b), Some(a)) if b == a => continue,
            (Some(b), Some(a)) => {
                return format!(
                    "\n  first difference at dump line {i}:\n    \
                     original:    {b}\n    regenerated: {a}"
                )
            }
            (Some(b), None) => {
                return format!(
                    "\n  regenerated dump ENDS at line {}; original continues:\n    \
                     original:    {b}",
                    i - 1
                )
            }
            (None, Some(a)) => {
                return format!(
                    "\n  original dump ENDS at line {}; regenerated continues:\n    \
                     regenerated: {a}",
                    i - 1
                )
            }
            (None, None) => {
                return "\n  dumps are identical (should not happen here)".to_string()
            }
        }
    }
}

/// Plain JavaScript.
const PLAIN: ParseFlags = ParseFlags {
    parse_flow: false,
    parse_flow_component_syntax: false,
    parse_flow_records: false,
    parse_flow_match: false,
    parse_ts: false,
    parse_jsx: false,
    strict_mode: false,
};

/// `-parse-flow`.
const FLOW: ParseFlags = ParseFlags {
    parse_flow: true,
    ..PLAIN
};

// ---------------------------------------------------------------------------
// One test per corpus directory. The dialect flags mirror
// `crates/parser/tests/parser_differential.rs:122-176`, which is where these
// corpora's `hermesc` invocations are defined.
// ---------------------------------------------------------------------------

/// `crates/sema/tests/sema_corpus` — 224 files, per-file `// FLAGS:`.
#[test]
fn corpus_sema() {
    run_corpus(
        "sema/tests/sema_corpus",
        PLAIN,
        Expected {
            files: 224,
            unparseable: &[
                "await-get-error.js",
                "class-static-block-return-error.js",
                "class-static-block-yield-error.js",
                "declare-error.js",
                "decorator-error.js",
                "error-expected-cross-line-note.js",
                "error-expected-same-line.js",
                "error-in-decl-rest-property.js",
                "error-return-outside-function.js",
                "flow-match-pattern-binding-error.js",
                "flow-match-pattern-object-binding-error.js",
                "flow-match-pattern-object-value-error.js",
                "for-of-error.js",
                "if-function-gen-error.js",
                "jsx-error-attr-member.js",
                "method-type-error.js",
                "nested-expressions.js",
                "nested-tagged-template-limit.js",
                "nested-unary-multichar-limit.js",
                "parse-error.js",
                "using-declaration-pattern-error.js",
                "yield-field-error.js",
                "yield-paren-error.js",
            ],
            cover: &[
                "error-cover-nodes.js (CoverEmptyArgs)",
                "flow-typecast-cover.js (CoverTypedIdentifier)",
            ],
        },
    );
}

/// `crates/sema/tests/sema_corpus_parser` — 17 files, per-file `// FLAGS:`.
#[test]
fn corpus_sema_parser() {
    run_corpus(
        "sema/tests/sema_corpus_parser",
        PLAIN,
        Expected {
            files: 17,
            unparseable: &[
                "parse-error-no-ast.js",
                "parse-error-recoverable.js",
            ],
            cover: &[],
        },
    );
}

/// `crates/parser/tests/parser_corpus` — 77 files, plain JS.
#[test]
fn corpus_parser() {
    run_corpus(
        "parser/tests/parser_corpus",
        PLAIN,
        Expected {
            files: 77,
            unparseable: &[],
            cover: &["cover_init.js (CoverInitializer)"],
        },
    );
}

/// `crates/parser/tests/parser_corpus_lazy` — 13 files, plain JS.
#[test]
fn corpus_parser_lazy() {
    run_corpus(
        "parser/tests/parser_corpus_lazy",
        PLAIN,
        Expected {
            files: 13,
            unparseable: &[],
            cover: &[],
        },
    );
}

/// `crates/parser/tests/parser_corpus_flow` — 42 files, `-parse-flow`.
#[test]
fn corpus_parser_flow() {
    run_corpus(
        "parser/tests/parser_corpus_flow",
        FLOW,
        Expected {
            // 43 since `declare_predicate.js` was added alongside the
            // `%checks` parser fix (`b"checks"` -> `b"%checks"` in
            // `flow/declarations.rs`), which is what first gave
            // `DeclaredPredicate`/`InferredPredicate` any corpus coverage —
            // before it, `declare function f(): boolean %checks;` did not
            // parse at all, so no corpus file could contain one.
            files: 43,
            unparseable: &[],
            cover: &[],
        },
    );
}

/// `crates/parser/tests/parser_corpus_flow_component` — 8 files,
/// `-parse-flow -Xparse-component-syntax`.
#[test]
fn corpus_parser_flow_component() {
    run_corpus(
        "parser/tests/parser_corpus_flow_component",
        ParseFlags {
            parse_flow_component_syntax: true,
            ..FLOW
        },
        Expected {
            files: 8,
            unparseable: &[],
            cover: &[],
        },
    );
}

/// `crates/parser/tests/parser_corpus_flow_records` — 5 files,
/// `-parse-flow -Xparse-flow-records`.
#[test]
fn corpus_parser_flow_records() {
    run_corpus(
        "parser/tests/parser_corpus_flow_records",
        ParseFlags {
            parse_flow_records: true,
            ..FLOW
        },
        Expected {
            files: 5,
            unparseable: &[],
            cover: &[],
        },
    );
}

/// `crates/parser/tests/parser_corpus_flow_match` — 7 files,
/// `-parse-flow -Xparse-flow-match`.
#[test]
fn corpus_parser_flow_match() {
    run_corpus(
        "parser/tests/parser_corpus_flow_match",
        ParseFlags {
            parse_flow_match: true,
            ..FLOW
        },
        Expected {
            files: 7,
            unparseable: &[],
            cover: &[],
        },
    );
}

/// `crates/parser/tests/parser_corpus_ts` — 20 files, `-parse-ts`.
#[test]
fn corpus_parser_ts() {
    run_corpus(
        "parser/tests/parser_corpus_ts",
        ParseFlags {
            parse_ts: true,
            ..PLAIN
        },
        Expected {
            files: 20,
            unparseable: &[],
            cover: &[],
        },
    );
}

/// `crates/parser/tests/parser_corpus_jsx` — 6 files, `-parse-jsx`.
#[test]
fn corpus_parser_jsx() {
    run_corpus(
        "parser/tests/parser_corpus_jsx",
        ParseFlags {
            parse_jsx: true,
            ..PLAIN
        },
        Expected {
            files: 6,
            unparseable: &[],
            cover: &[],
        },
    );
}

/// `crates/parser/tests/parser_corpus_jsx_flow` — 1 file,
/// `-parse-jsx -parse-flow`.
#[test]
fn corpus_parser_jsx_flow() {
    run_corpus(
        "parser/tests/parser_corpus_jsx_flow",
        ParseFlags {
            parse_jsx: true,
            ..FLOW
        },
        Expected {
            files: 1,
            unparseable: &[],
            cover: &[],
        },
    );
}
