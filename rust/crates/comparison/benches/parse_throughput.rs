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

use ast::context::Context;
use ast::node::Node;
use boa_ast::scope::Scope;
use boa_interner::Interner;
use boa_parser::{Parser as BoaParser, Source};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxc_allocator::Allocator;
use oxc_span::SourceType;
use parser::js::JSParserImpl;
use parser::lexer::{GrammarContext, JSLexer};
use support::manager::SourceErrorManager;
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
// Parser wrappers
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
// Benchmark
// ---------------------------------------------------------------------------

fn bench_parse(c: &mut Criterion) {
    let fixtures: &[(&str, &str)] = &[
        ("react", "react.development.js"),
        ("jquery", "jquery-3.7.1.js"),
        ("three_min", "three.min.js"),
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
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
