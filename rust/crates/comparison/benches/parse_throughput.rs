/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! parse_throughput: criterion benchmarks comparing the Hermes Rust port parser
//! against SWC, OXC, and Boa.
//!
//! ⚠  APPLES-TO-ORANGES CAVEAT ⚠
//! These parsers differ in:
//!   • Whether they build a concrete/dense or sparse/lazy AST.
//!   • Whether they intern atoms/identifiers.
//!   • Whether they resolve/validate during parsing.
//!   • Whether they allocate into an arena or the system heap.
//! Throughput numbers reflect total wall-time for parse() only; they say nothing
//! about correctness, feature coverage, or memory usage.  Use as a directional
//! signal only.
//!
//! Resolved competitor versions (from Cargo.lock):
//!   swc_ecma_parser = "41.1.1"
//!   oxc_parser       = "0.137.0"
//!   biome_js_parser  = NOT BENCHMARKED (biome_rowan/biome_js_syntax version
//!                       mismatch: FileSourceError variants changed from tuple
//!                       to unit between the published 0.5.7/0.5.8 crates,
//!                       causing compile errors inside the crate itself)
//!   boa_parser       = "0.21.1"

use std::hint::black_box;

use boa_ast::scope::Scope;
use boa_interner::Interner;
use boa_parser::{Parser as BoaParser, Source};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use hermes_ast::context::Context;
use hermes_ast::node::Node;
use hermes_parser::js::JSParserImpl;
use hermes_parser::lexer::{GrammarContext, JSLexer};
use hermes_parser::token_kinds::TokenKind;
use hermes_support::manager::SourceErrorManager;
use oxc_allocator::Allocator;
use oxc_span::SourceType;
use swc_common::sync::Lrc;
use swc_common::{FileName, SourceMap};
use swc_ecma_parser::{lexer::Lexer, Parser as SwcParser, StringInput, Syntax};

// ---------------------------------------------------------------------------
// Fixture loading
// ---------------------------------------------------------------------------

fn load_fixture(name: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = format!("{}/fixtures/{}", manifest_dir, name);
    std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "fixture not found: {}\nRun: bash {}/fetch_fixtures.sh",
            path, manifest_dir
        )
    })
}

// ---------------------------------------------------------------------------
// Parser wrappers (success path — black_box the result)
// ---------------------------------------------------------------------------

fn parse_hermes(src: &str) {
    let bytes = src.as_bytes();
    let mut sm = SourceErrorManager::new();
    let buf_id = sm.add_buffer_bytes("bench", bytes);
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let _result: Option<&Node> = {
        let atoms = &gc.ctx().atom_table;
        let lexer = JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut p = JSParserImpl::new(&gc, lexer);
        p.parse()
    };
    black_box(_result);
}

/// Lex-only benchmark: drive the lexer to EOF without building any AST.
/// This isolates lexing + identifier-interning from AST-node construction.
fn lex_only_hermes(src: &str) {
    let bytes = src.as_bytes();
    let mut sm = SourceErrorManager::new();
    let buf_id = sm.add_buffer_bytes("bench", bytes);
    // We need an AtomTable for the lexer to intern identifiers into.
    // Allocate it through a throw-away Context so the API matches parse_hermes.
    let ctx = Context::new();
    let atoms = &ctx.atom_table;
    let mut lexer = JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);
    // Advance through every token until EOF.
    loop {
        let tok = lexer.advance(GrammarContext::AllowRegExp);
        if black_box(tok.kind()) == TokenKind::eof {
            break;
        }
    }
}

fn parse_swc(src: &str) {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        FileName::Custom("bench.js".into()).into(),
        src.to_string(),
    );
    let lexer = Lexer::new(
        Syntax::Es(Default::default()),
        Default::default(),
        StringInput::from(&*fm),
        None,
    );
    let mut p = SwcParser::new_from(lexer);
    let result = p.parse_program();
    black_box(result.ok());
}

fn parse_oxc(src: &str) {
    let allocator = Allocator::default();
    let source_type = SourceType::default();
    let result = oxc_parser::Parser::new(&allocator, src, source_type).parse();
    black_box(result.program);
}

fn parse_boa(src: &str) {
    let scope = Scope::new_global();
    let mut interner = Interner::default();
    let result =
        BoaParser::new(Source::from_bytes(src.as_bytes())).parse_script(&scope, &mut interner);
    black_box(result.ok());
}

// ---------------------------------------------------------------------------
// Fairness assert helpers — each returns true if the parser errored as expected
// ---------------------------------------------------------------------------

/// Returns true if hermes reported a parse error (parse() returns None OR
/// the SourceErrorManager has at least one error recorded).
fn hermes_errors_on(src: &str) -> bool {
    let bytes = src.as_bytes();
    let mut sm = SourceErrorManager::new();
    let buf_id = sm.add_buffer_bytes("bench", bytes);
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let result: Option<&Node> = {
        let atoms = &gc.ctx().atom_table;
        let lexer = JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut p = JSParserImpl::new(&gc, lexer);
        p.parse()
    };
    result.is_none() || sm.error_count() > 0
}

/// Returns true if SWC produced a parse error (Err result).
fn swc_errors_on(src: &str) -> bool {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        FileName::Custom("bench.js".into()).into(),
        src.to_string(),
    );
    let lexer = Lexer::new(
        Syntax::Es(Default::default()),
        Default::default(),
        StringInput::from(&*fm),
        None,
    );
    let mut p = SwcParser::new_from(lexer);
    p.parse_program().is_err()
}

/// Returns true if OXC produced at least one parse error.
/// OXC 0.137.0 exposes diagnostics via `ParserReturn::diagnostics`.
fn oxc_errors_on(src: &str) -> bool {
    let allocator = Allocator::default();
    let source_type = SourceType::default();
    let result = oxc_parser::Parser::new(&allocator, src, source_type).parse();
    !result.diagnostics.is_empty()
}

/// Returns true if Boa produced a parse error (Err result).
fn boa_errors_on(src: &str) -> bool {
    let scope = Scope::new_global();
    let mut interner = Interner::default();
    BoaParser::new(Source::from_bytes(src.as_bytes()))
        .parse_script(&scope, &mut interner)
        .is_err()
}

/// Run the fairness check: assert all four parsers error on every `.err` file.
/// Panics with a clear diagnostic naming any parser that did NOT error.
fn assert_error_fixtures(err_fixtures: &[(&str, String)]) {
    let mut all_ok = true;
    for (name, src) in err_fixtures {
        let h = hermes_errors_on(src);
        let s = swc_errors_on(src);
        let o = oxc_errors_on(src);
        let b = boa_errors_on(src);
        if !h || !s || !o || !b {
            all_ok = false;
            eprintln!(
                "FAIRNESS FAILURE on fixture '{name}': hermes={h} swc={s} oxc={o} boa={b}"
            );
        }
    }
    if all_ok {
        println!(
            "fairness check: all parsers errored on .err fixtures ✓"
        );
    } else {
        panic!(
            "Fairness check failed: one or more parsers did NOT error on a .err fixture. \
             See stderr output above. A parser that succeeds on a corrupted fixture is \
             lazy/short-circuiting and its throughput is not comparable."
        );
    }
}

// ---------------------------------------------------------------------------
// Benchmark
// ---------------------------------------------------------------------------

fn bench_parse(c: &mut Criterion) {
    let fixtures: &[(&str, &str)] = &[
        ("react", "react.development.js"),
        ("jquery", "jquery-3.7.1.js"),
        ("three_min", "three.min.js"),
        ("typescript", "typescript.js"),
    ];

    // Load all fixtures upfront (panics with a helpful message if missing).
    let fixture_data: Vec<(&str, String)> = fixtures
        .iter()
        .map(|(name, file)| (*name, load_fixture(file)))
        .collect();

    let mut group = c.benchmark_group("parse");

    for (name, src) in &fixture_data {
        let bytes = src.len() as u64;
        group.throughput(Throughput::Bytes(bytes));

        group.bench_function(BenchmarkId::new("hermes", name), |b| {
            b.iter(|| parse_hermes(src));
        });

        group.bench_function(BenchmarkId::new("swc", name), |b| {
            b.iter(|| parse_swc(src));
        });

        group.bench_function(BenchmarkId::new("oxc", name), |b| {
            b.iter(|| parse_oxc(src));
        });

        group.bench_function(BenchmarkId::new("boa", name), |b| {
            b.iter(|| parse_boa(src));
        });
    }

    group.finish();

    // -----------------------------------------------------------------------
    // Error-variant group
    //
    // Each .err.js file is the corresponding valid fixture with a deliberately
    // broken statement appended at EOF:
    //     var __bench_parse_error__ = ;
    // This forces every eager parser to traverse the whole file and then fail.
    // A parser that succeeds on a .err file is lazy/short-circuiting; its
    // throughput is not comparable to eager parsers.  The fairness assert
    // below catches that case.
    // -----------------------------------------------------------------------

    let err_fixtures_spec: &[(&str, &str)] = &[
        ("react", "react.development.err.js"),
        ("jquery", "jquery-3.7.1.err.js"),
        ("three_min", "three.min.err.js"),
        ("typescript", "typescript.err.js"),
    ];

    let err_fixture_data: Vec<(&str, String)> = err_fixtures_spec
        .iter()
        .map(|(name, file)| (*name, load_fixture(file)))
        .collect();

    // Fairness gate: every parser must report a parse error on every .err file.
    assert_error_fixtures(&err_fixture_data);

    let mut err_group = c.benchmark_group("parse_err");

    for (name, src) in &err_fixture_data {
        let bytes = src.len() as u64;
        err_group.throughput(Throughput::Bytes(bytes));

        err_group.bench_function(BenchmarkId::new("hermes", name), |b| {
            b.iter(|| parse_hermes(src));
        });

        err_group.bench_function(BenchmarkId::new("swc", name), |b| {
            b.iter(|| parse_swc(src));
        });

        err_group.bench_function(BenchmarkId::new("oxc", name), |b| {
            b.iter(|| parse_oxc(src));
        });

        err_group.bench_function(BenchmarkId::new("boa", name), |b| {
            b.iter(|| parse_boa(src));
        });
    }

    err_group.finish();
}

// ---------------------------------------------------------------------------
// Lex-only decomposition benchmark (Task 5)
//
// Runs only the Hermes lexer to EOF on each fixture — no JSParserImpl, no AST
// construction.  Comparing `lex_only/hermes/<fixture>` to
// `parse/hermes/<fixture>` decomposes the full-parse cost into:
//   * lexing + identifier-interning   (lex_only portion)
//   * AST-node construction           (remainder)
//
// If the lex_only throughput is much higher than full-parse on large files but
// not on small files, AST-construction cost scales super-linearly (e.g. due to
// node footprint / cache pressure), which is the leading hypothesis for the
// ~32% Rust-vs-C++ gap on typescript.js.
// ---------------------------------------------------------------------------

fn bench_lex_only(c: &mut Criterion) {
    let fixtures: &[(&str, &str)] = &[
        ("react", "react.development.js"),
        ("jquery", "jquery-3.7.1.js"),
        ("three_min", "three.min.js"),
        ("typescript", "typescript.js"),
    ];

    let fixture_data: Vec<(&str, String)> = fixtures
        .iter()
        .map(|(name, file)| (*name, load_fixture(file)))
        .collect();

    let mut group = c.benchmark_group("lex_only");

    for (name, src) in &fixture_data {
        let bytes = src.len() as u64;
        group.throughput(Throughput::Bytes(bytes));

        group.bench_function(BenchmarkId::new("hermes", name), |b| {
            b.iter(|| lex_only_hermes(src));
        });
    }

    group.finish();
}

criterion_group!(benches, bench_parse, bench_lex_only);
criterion_main!(benches);
