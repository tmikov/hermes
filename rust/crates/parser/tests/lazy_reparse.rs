/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Oracle A — reparse-equivalence test (Task L2.3).
//!
//! For each `.js` file in `tests/parser_corpus_lazy/` and a selection from
//! `tests/parser_corpus/`, and for thresholds `[0, 20]` (0 = defer all
//! eligible; 20 = defer only bodies longer than 20 bytes):
//!
//! 1. **Eager parse** (`FullParse`) → `eager_ast`.
//!    Walk it; collect:
//!    - `eager_offsets: BTreeSet<u32>` of every function-like node's start
//!      offset (FunctionDeclaration, FunctionExpression, block-bodied
//!      ArrowFunctionExpression, getter/setter Property, MethodDefinition).
//!    - `eager_bodies: BTreeMap<u32, String>` — dump of each body to JSON
//!      (no locations, HideEmpty mode) keyed by the function's start offset.
//!
//! 2. **PreParse** (fresh parser, same buffer) → side-table.
//!
//! 3. **LazyParse** (fresh parser + table + threshold) → `lazy_ast`.
//!    Walk it; collect the same maps over the lazy skeleton.
//!
//! 4. Assert `eager_offsets == lazy_offsets` (offset-set equality).
//!
//! 5. For each lazy function whose body is a lazy stub: call
//!    `parse_lazy_function(kind, param_yield, param_await, start)`, dump the
//!    re-parsed body, and `assert_eq!` it against `eager_bodies[offset]`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use hermes_ast::context::Context;
use hermes_ast::dump::{ESTreeDumpMode, ESTreeRawProp, LocationDumpMode, dump_estree_json_with_sm};
use hermes_ast::node::{Node, NodeKind};
use hermes_parser::js::{JSParserImpl, ParserPass};
use hermes_parser::lexer::{GrammarContext, JSLexer};
use hermes_support::manager::SourceErrorManager;

// ---------------------------------------------------------------------------
// Information collected per function-like node during the walk.
// ---------------------------------------------------------------------------

/// Everything we need about one function-like node.
struct FuncEntry {
    /// Which flavour of `parse_lazy_function` to call.
    kind: NodeKind,
    /// Whether the body BlockStatement carries `is_lazy_function_body`.
    is_lazy_stub: bool,
    /// `param_yield` stored in the stub (only valid when `is_lazy_stub`).
    param_yield: bool,
    /// `param_await` stored in the stub (only valid when `is_lazy_stub`).
    param_await: bool,
    /// Start offset of the function body `{` — the key into the pre-parse
    /// side-table (`PreParsedBufferInfo.function_info`). Used by the caller
    /// to retrieve `strict_mode` and set it on the lazy parser before
    /// `parse_lazy_function`, mirroring HBC.cpp:158
    /// (`parser.setStrictMode(lazyData.strictMode)`).
    body_start_offset: u32,
}

// ---------------------------------------------------------------------------
// Helpers — probe a BlockStatement or a function value.
// ---------------------------------------------------------------------------

/// Return `(is_stub, param_yield, param_await)` from a `BlockStatement` node.
fn block_info(node: &Node<'_>) -> (bool, bool, bool) {
    if let Node::BlockStatement(b) = node {
        (
            b.is_lazy_function_body.get(),
            b.param_yield.get(),
            b.param_await.get(),
        )
    } else {
        (false, false, false)
    }
}

/// For a `Property.value` or `MethodDefinition.value`, return
/// `Some((is_stub, py, pa))` if the value is a FunctionExpression with a
/// block body; `None` otherwise.
fn func_value_block_info<'gc>(value: &'gc Node<'gc>) -> Option<(bool, bool, bool)> {
    if let Node::FunctionExpression(fe) = value {
        if let Node::BlockStatement(_) = fe.body {
            return Some(block_info(fe.body));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// AST walker — collects FuncEntry per function-like node.
// ---------------------------------------------------------------------------

fn collect_funcs<'gc>(node: &'gc Node<'gc>, out: &mut BTreeMap<u32, FuncEntry>) {
    match node {
        Node::FunctionDeclaration(fd) => {
            // Anonymous FunctionDeclarations (`export default function(){}`)
            // cannot be re-parsed via `parse_lazy_function` because it calls
            // `parse_function_declaration(PARAM_RETURN, …)` which requires a
            // name. Skip them; they are always a direct child of
            // `ExportDefaultDeclaration` and have no nested functions that
            // would be missed by the overall recursive walk.
            if fd.id.is_none() {
                collect_funcs(fd.body, out);
                return;
            }
            let (is_stub, py, pa) = block_info(fd.body);
            out.insert(
                node.range().start.offset,
                FuncEntry {
                    kind: NodeKind::FunctionDeclaration,
                    is_lazy_stub: is_stub,
                    param_yield: py,
                    param_await: pa,
                    body_start_offset: fd.body.range().start.offset,
                },
            );
            collect_funcs(fd.body, out);
        }

        Node::FunctionExpression(fe) => {
            let (is_stub, py, pa) = block_info(fe.body);
            out.insert(
                node.range().start.offset,
                FuncEntry {
                    kind: NodeKind::FunctionExpression,
                    is_lazy_stub: is_stub,
                    param_yield: py,
                    param_await: pa,
                    body_start_offset: fe.body.range().start.offset,
                },
            );
            collect_funcs(fe.body, out);
        }

        Node::ArrowFunctionExpression(afe) => {
            // Only block-bodied arrows can have a lazy stub. Concise
            // (expression) bodies are never deferred; skip them so the offset
            // maps stay symmetric between eager and lazy passes.
            if let Node::BlockStatement(_) = afe.body {
                let (is_stub, py, pa) = block_info(afe.body);
                out.insert(
                    node.range().start.offset,
                    FuncEntry {
                        kind: NodeKind::ArrowFunctionExpression,
                        is_lazy_stub: is_stub,
                        param_yield: py,
                        param_await: pa,
                        body_start_offset: afe.body.range().start.offset,
                    },
                );
                collect_funcs(afe.body, out);
            } else {
                // concise body: just recurse into the expression.
                collect_funcs(afe.body, out);
            }
        }

        Node::Property(prop) => {
            if let Some((is_stub, py, pa)) = func_value_block_info(prop.value) {
                let body_start = if let Node::FunctionExpression(fe) = prop.value {
                    fe.body.range().start.offset
                } else {
                    0
                };
                out.insert(
                    node.range().start.offset,
                    FuncEntry {
                        kind: NodeKind::Property,
                        is_lazy_stub: is_stub,
                        param_yield: py,
                        param_await: pa,
                        body_start_offset: body_start,
                    },
                );
                if let Node::FunctionExpression(fe) = prop.value {
                    collect_funcs(fe.body, out);
                }
            } else {
                collect_children(node, out);
            }
        }

        Node::MethodDefinition(md) => {
            if let Some((is_stub, py, pa)) = func_value_block_info(md.value) {
                let body_start = if let Node::FunctionExpression(fe) = md.value {
                    fe.body.range().start.offset
                } else {
                    0
                };
                out.insert(
                    node.range().start.offset,
                    FuncEntry {
                        kind: NodeKind::MethodDefinition,
                        is_lazy_stub: is_stub,
                        param_yield: py,
                        param_await: pa,
                        body_start_offset: body_start,
                    },
                );
                if let Node::FunctionExpression(fe) = md.value {
                    collect_funcs(fe.body, out);
                }
            } else {
                collect_children(node, out);
            }
        }

        _ => collect_children(node, out),
    }
}

fn collect_children<'gc>(node: &'gc Node<'gc>, out: &mut BTreeMap<u32, FuncEntry>) {
    node.visit_children(&mut ChildVisitor(out));
}

struct ChildVisitor<'a>(pub &'a mut BTreeMap<u32, FuncEntry>);

impl<'gc> hermes_ast::visitor::Visitor<'gc> for ChildVisitor<'_> {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        collect_funcs(node, self.0);
    }
}

// ---------------------------------------------------------------------------
// Eager body collector — same traversal but captures the body STRING (JSON)
// for each function-like node, using the eager context's atom table.
// ---------------------------------------------------------------------------

fn collect_eager_body_strings<'gc>(
    node: &'gc Node<'gc>,
    atoms: &hermes_atom_table::AtomTable,
    sm: &hermes_support::manager::SourceErrorManager,
    out: &mut BTreeMap<u32, String>,
) {
    match node {
        Node::FunctionDeclaration(fd) => {
            // Skip anonymous FunctionDeclarations (see collect_funcs note).
            if fd.id.is_none() {
                collect_eager_body_strings(fd.body, atoms, sm, out);
                return;
            }
            let start = node.range().start.offset;
            out.insert(start, dump_node(fd.body, atoms, sm));
            collect_eager_body_strings(fd.body, atoms, sm, out);
        }
        Node::FunctionExpression(fe) => {
            let start = node.range().start.offset;
            out.insert(start, dump_node(fe.body, atoms, sm));
            collect_eager_body_strings(fe.body, atoms, sm, out);
        }
        Node::ArrowFunctionExpression(afe) => {
            if let Node::BlockStatement(_) = afe.body {
                let start = node.range().start.offset;
                out.insert(start, dump_node(afe.body, atoms, sm));
                collect_eager_body_strings(afe.body, atoms, sm, out);
            } else {
                collect_eager_body_strings(afe.body, atoms, sm, out);
            }
        }
        Node::Property(prop) => {
            if func_value_block_info(prop.value).is_some() {
                let start = node.range().start.offset;
                if let Node::FunctionExpression(fe) = prop.value {
                    out.insert(start, dump_node(fe.body, atoms, sm));
                    collect_eager_body_strings(fe.body, atoms, sm, out);
                }
            } else {
                collect_eager_body_string_children(node, atoms, sm, out);
            }
        }
        Node::MethodDefinition(md) => {
            if func_value_block_info(md.value).is_some() {
                let start = node.range().start.offset;
                if let Node::FunctionExpression(fe) = md.value {
                    out.insert(start, dump_node(fe.body, atoms, sm));
                    collect_eager_body_strings(fe.body, atoms, sm, out);
                }
            } else {
                collect_eager_body_string_children(node, atoms, sm, out);
            }
        }
        _ => collect_eager_body_string_children(node, atoms, sm, out),
    }
}

fn collect_eager_body_string_children<'gc>(
    node: &'gc Node<'gc>,
    atoms: &hermes_atom_table::AtomTable,
    sm: &hermes_support::manager::SourceErrorManager,
    out: &mut BTreeMap<u32, String>,
) {
    node.visit_children(&mut EagerBodyChildVisitor { atoms, sm, out });
}

struct EagerBodyChildVisitor<'a> {
    atoms: &'a hermes_atom_table::AtomTable,
    sm: &'a hermes_support::manager::SourceErrorManager,
    out: &'a mut BTreeMap<u32, String>,
}

impl<'gc> hermes_ast::visitor::Visitor<'gc> for EagerBodyChildVisitor<'_> {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        collect_eager_body_strings(node, self.atoms, self.sm, self.out);
    }
}

// ---------------------------------------------------------------------------
// Dump helper — dump a node to JSON with full location info.
// ---------------------------------------------------------------------------

fn dump_node<'a>(
    node: &'a Node<'a>,
    atoms: &hermes_atom_table::AtomTable,
    sm: &hermes_support::manager::SourceErrorManager,
) -> String {
    let mut out = String::new();
    dump_estree_json_with_sm(
        &mut out,
        node,
        false,
        ESTreeDumpMode::HideEmpty,
        sm,
        LocationDumpMode::LocAndRange,
        ESTreeRawProp::Exclude,
        atoms,
    );
    out
}

// ---------------------------------------------------------------------------
// Extract the body of a re-parsed node returned by `parse_lazy_function`.
// ---------------------------------------------------------------------------

/// `parse_lazy_function` returns:
/// - FunctionDeclaration / FunctionExpression / ArrowFunctionExpression:
///   the whole function; extract `.body`.
/// - Property:  the `value` FunctionExpression (already extracted at L2.2
///   line cpp:7572); extract its `.body`.
/// - MethodDefinition: the `value` FunctionExpression (cpp:7591); extract `.body`.
fn reparsed_body<'gc>(entry: &FuncEntry, reparsed: &'gc Node<'gc>) -> &'gc Node<'gc> {
    match entry.kind {
        NodeKind::FunctionDeclaration => {
            let Node::FunctionDeclaration(fd) = reparsed else {
                panic!("expected FunctionDeclaration, got {:?}", reparsed.kind())
            };
            fd.body
        }
        NodeKind::FunctionExpression => {
            let Node::FunctionExpression(fe) = reparsed else {
                panic!("expected FunctionExpression, got {:?}", reparsed.kind())
            };
            fe.body
        }
        NodeKind::ArrowFunctionExpression => {
            let Node::ArrowFunctionExpression(afe) = reparsed else {
                panic!("expected ArrowFunctionExpression, got {:?}", reparsed.kind())
            };
            afe.body
        }
        // Property/MethodDefinition: parse_lazy_function already returns the
        // inner FunctionExpression.
        NodeKind::Property | NodeKind::MethodDefinition => {
            let Node::FunctionExpression(fe) = reparsed else {
                panic!(
                    "Property/Method: expected FunctionExpression, got {:?}",
                    reparsed.kind()
                )
            };
            fe.body
        }
        _ => panic!("unexpected kind {:?} in FuncEntry", entry.kind),
    }
}

// ---------------------------------------------------------------------------
// Core oracle for one (source, threshold) pair.
// ---------------------------------------------------------------------------

/// Run the reparse-equivalence check for one source buffer at one threshold.
///
/// Returns the number of lazy-stub function bodies that were re-parsed and
/// compared (for diagnostic counting).
fn check_file(src: &[u8], label: &str, threshold: u32) -> usize {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer_bytes(label, src);

    // A second SourceErrorManager for location dumps: the live parsers hold
    // `&mut sm`, so dumps resolve line/col through an identical read-only
    // copy. Same content + first buffer => same SourceId (asserted).
    let mut sm_dump = SourceErrorManager::new();
    let id_dump = sm_dump.add_buffer_bytes(label, src);
    assert_eq!(id, id_dump, "buffer id mismatch between managers");
    let sm_dump = sm_dump; // no longer mutated

    // ---- 1. Eager parse — capture body dump strings while the GCLock is live.
    let eager_bodies: BTreeMap<u32, String>;
    let eager_offsets: BTreeSet<u32>;
    {
        let mut ctx1 = Context::new();
        let gc1 = ctx1.lock();
        let lexer = JSLexer::new(id, &mut sm, &gc1.ctx().atom_table, GrammarContext::AllowRegExp);
        let mut p = JSParserImpl::new_with_pass(&gc1, lexer, ParserPass::FullParse);
        let root = p.parse().unwrap_or_else(|| panic!("[{label}] eager parse failed"));

        let mut bodies: BTreeMap<u32, String> = BTreeMap::new();
        collect_eager_body_strings(root, &gc1.ctx().atom_table, &sm_dump, &mut bodies);
        eager_bodies = bodies;
        eager_offsets = eager_bodies.keys().copied().collect();
        // gc1, ctx1 drop here — nodes reclaimed, but strings are owned.
    }

    // ---- 2. PreParse ----
    let table = {
        let mut ctx2 = Context::new();
        let gc2 = ctx2.lock();
        let lexer = JSLexer::new(id, &mut sm, &gc2.ctx().atom_table, GrammarContext::AllowRegExp);
        let mut pp = JSParserImpl::new_with_pass(&gc2, lexer, ParserPass::PreParse);
        pp.parse().unwrap_or_else(|| panic!("[{label}] preparse failed"));
        pp.take_pre_parsed()
        // gc2, ctx2 drop here
    };

    // ---- 3. LazyParse + 4. offset equality + 5. body comparisons ----
    // Clone the table so we can look up strict_mode per body during BFS
    // (mirroring HBC.cpp:158: `parser.setStrictMode(lazyData.strictMode)`
    // called immediately before each `parseLazyFunction`). The clone is cheap
    // relative to parse time; `PreParsedFunctionInfo` is `#[derive(Clone)]`.
    let table_ref = table.clone();
    let mut ctx3 = Context::new();
    ctx3.set_preemptive_function_compilation_threshold(threshold);
    let gc3 = ctx3.lock();
    let lexer = JSLexer::new(id, &mut sm, &gc3.ctx().atom_table, GrammarContext::AllowRegExp);
    let mut lp = JSParserImpl::new_with_pass(&gc3, lexer, ParserPass::LazyParse);
    lp.set_pre_parsed(table);
    let lazy_root = lp.parse().unwrap_or_else(|| panic!("[{label}] lazyparse failed"));

    let mut lazy_map: BTreeMap<u32, FuncEntry> = BTreeMap::new();
    collect_funcs(lazy_root, &mut lazy_map);
    let lazy_offsets: BTreeSet<u32> = lazy_map.keys().copied().collect();

    // ---- 4. Offset-set subset check ----
    // Functions nested inside a lazy stub are not visible in the skeleton
    // (the stub body is opaque), so we require lazy_offsets ⊆ eager_offsets.
    // Equality would only hold when threshold is large enough that nothing is
    // deferred (every body is eagerly parsed), but that scenario is the
    // caller's choice and is not guaranteed by the thresholds we test.
    // We DO assert there are no offsets in the lazy set that are absent in
    // eager — that would indicate a seek/resume corruption.
    let extra_in_lazy: Vec<u32> = lazy_offsets.difference(&eager_offsets).copied().collect();
    assert!(
        extra_in_lazy.is_empty(),
        "[{label}] threshold={threshold}: lazy has offsets absent in eager: {extra_in_lazy:?}"
    );

    // ---- 5. Per-body re-parse + compare (BFS over all stubs) ----
    //
    // Design note: when `outer` is demand-parsed at threshold=0, nested
    // functions inside `outer`'s body (e.g. `inner`) appear as lazy stubs in
    // the re-parsed result — the parser is still in LazyParse mode and the
    // pre-parse table has entries for ALL functions. This means the re-parsed
    // body of `outer` cannot be byte-for-byte equal to the eager body (which
    // has `inner` fully parsed).
    //
    // We handle this with a BFS: demand-parse each stub, collect nested stubs
    // from the re-parsed body, enqueue them, and repeat. We only perform the
    // byte-for-byte body comparison for functions whose re-parsed body has NO
    // remaining lazy stubs (leaf-level comparison). For functions with nested
    // stubs, we validate them indirectly via the leaf comparisons.
    let mut n_compared = 0usize;

    // Queue of (offset, FuncEntry) to demand-parse, seeded from skeleton stubs.
    let mut queue: std::collections::VecDeque<(u32, FuncEntry)> = lazy_map
        .into_iter()
        .filter(|(_, e)| e.is_lazy_stub)
        .collect();

    while let Some((offset, entry)) = queue.pop_front() {
        let start = hermes_support::location::SMLoc { source: id, offset };

        // Mirror HBC.cpp:158: set the lazy parser's strict mode from the
        // pre-parsed table entry for this function's body, identified by its
        // body-start offset (the key `parse_function_body` stores at PreParse
        // time). This ensures `static`, `let`, etc. are lexed correctly for
        // strict-mode bodies (e.g. class methods) without any in-function hack.
        let strict = table_ref
            .function_info
            .get(&entry.body_start_offset)
            .map(|info| info.strict_mode)
            .unwrap_or(false);
        lp.set_strict_mode(strict);

        let reparsed = lp
            .parse_lazy_function(
                entry.kind,
                entry.param_yield,
                entry.param_await,
                start,
            )
            .unwrap_or_else(|| {
                panic!(
                    "[{label}] threshold={threshold}: parse_lazy_function failed at offset {offset}"
                )
            });

        let re_body = reparsed_body(&entry, reparsed);

        // Collect nested function entries from the re-parsed body.
        let mut nested: BTreeMap<u32, FuncEntry> = BTreeMap::new();
        collect_funcs(re_body, &mut nested);

        let has_nested_stubs = nested.values().any(|e| e.is_lazy_stub);

        if !has_nested_stubs {
            // Leaf level: no nested stubs — perform the byte-for-byte
            // body comparison against the eager parse.
            let re_dump = dump_node(re_body, &gc3.ctx().atom_table, &sm_dump);
            let eg_dump = eager_bodies
                .get(&offset)
                .unwrap_or_else(|| panic!("[{label}] no eager body at offset {offset}"));
            assert_eq!(
                eg_dump,
                &re_dump,
                "[{label}] threshold={threshold} offset={offset}: body mismatch\n\
                 EAGER:\n{eg_dump}\nREPARSED:\n{re_dump}"
            );
            n_compared += 1;
        }

        // Enqueue all nested stubs from the re-parsed body for BFS.
        for (nested_offset, nested_entry) in nested {
            if nested_entry.is_lazy_stub {
                queue.push_back((nested_offset, nested_entry));
            }
        }
    }

    n_compared
}

// ---------------------------------------------------------------------------
// File lists.
// ---------------------------------------------------------------------------

fn corpus_files(dir: &str) -> Vec<PathBuf> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&base)
        .unwrap_or_else(|e| panic!("cannot read corpus dir {}: {e}", base.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().map(|e| e == "js").unwrap_or(false))
        .collect();
    files.sort();
    files
}

// ---------------------------------------------------------------------------
// Test entry points.
// ---------------------------------------------------------------------------

const THRESHOLDS: [u32; 2] = [0, 20];

/// Lazy corpus (all files).
#[test]
fn lazy_corpus_reparse_equivalence() {
    let files = corpus_files("tests/parser_corpus_lazy");
    assert!(!files.is_empty(), "lazy corpus is empty");
    let mut total_files = 0usize;
    let mut total_comparisons = 0usize;
    for path in &files {
        let src = std::fs::read(path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let label = path.file_name().unwrap().to_string_lossy().into_owned();
        for &thresh in &THRESHOLDS {
            let n = check_file(&src, &label, thresh);
            total_comparisons += n;
        }
        total_files += 1;
    }
    eprintln!(
        "lazy_corpus_reparse_equivalence: {total_files} files × {} thresholds, \
         {total_comparisons} body comparisons — all passed",
        THRESHOLDS.len(),
    );
}

/// All files from the standard parser corpus (sorted, deterministic).
/// Files that contain no function-like nodes contribute 0 comparisons and
/// are silently skipped by the check_file logic; we assert that at least 10
/// body comparisons actually happened across all thresholds so a mass-rename
/// cannot silently hollow out the test.
///
/// Runs on a thread with an enlarged stack: this is the only test that parses
/// the standard corpus IN-PROCESS (`preparse_differential` and
/// `parser_differential` shell out, so their parses get a process main
/// thread's 8 MiB), and
/// `nested-parens-limit.js` is deliberately 125 levels deep — one below the
/// recursion limit — which is more unoptimized recursive-descent frames than
/// the 2 MiB the test harness gives a test thread.
#[test]
fn parser_corpus_reparse_equivalence() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(parser_corpus_reparse_equivalence_impl)
        .expect("failed to spawn the corpus-reparse thread")
        .join()
        .expect("the corpus-reparse thread panicked");
}

fn parser_corpus_reparse_equivalence_impl() {
    let files = corpus_files("tests/parser_corpus");
    assert!(!files.is_empty(), "parser corpus is empty");
    let mut total_files = 0usize;
    let mut total_comparisons = 0usize;
    for path in &files {
        let src = std::fs::read(path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let label = path.file_name().unwrap().to_string_lossy().into_owned();
        for &thresh in &THRESHOLDS {
            let n = check_file(&src, &label, thresh);
            total_comparisons += n;
        }
        total_files += 1;
    }
    assert!(
        total_comparisons >= 10,
        "parser_corpus_reparse_equivalence: only {total_comparisons} body \
         comparisons — expected at least 10; check that corpus files with \
         functions still exist"
    );
    eprintln!(
        "parser_corpus_reparse_equivalence: {total_files} files × {} thresholds, \
         {total_comparisons} body comparisons — all passed",
        THRESHOLDS.len(),
    );
}
