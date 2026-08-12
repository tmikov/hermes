/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! node_count: parse a JS fixture and report AST node/storage statistics.
//!
//! Used for Task 5 profiling: measures AST node count and storage footprint
//! for large fixtures (e.g. typescript.js) to help decompose the Rust-vs-C++
//! parse-throughput gap.
//!
//! Usage:
//!   cargo run --release --manifest-path rust/crates/comparison/Cargo.toml \
//!       --example node_count -- fixtures/typescript.js

use hermes_ast::context::Context;
use hermes_ast::node::Node;
use hermes_parser::js::JSParserImpl;
use hermes_parser::lexer::{GrammarContext, JSLexer};
use hermes_support::manager::SourceErrorManager;

/// Print the size of a type in bytes.
macro_rules! print_sizeof {
    ($t:ty) => {
        println!("sizeof({}) = {} bytes", stringify!($t), std::mem::size_of::<$t>());
    };
}

fn main() {
    // Print key type sizes for analysis.
    print_sizeof!(Node<'static>);
    println!();

    let args: Vec<String> = std::env::args().collect();
    let path = if args.len() > 1 {
        args[1].clone()
    } else {
        // Default: typescript fixture relative to the comparison crate.
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        format!("{}/fixtures/typescript.js", manifest_dir)
    };

    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read '{}': {}", path, e));
    let file_bytes = src.len();

    let bytes = src.as_bytes();
    let mut sm = SourceErrorManager::new();
    let buf_id = sm.add_buffer_bytes("bench", bytes);
    let mut ctx = Context::new();
    let parse_ok = {
        let gc = ctx.lock();
        let result: Option<&Node> = {
            let atoms = &gc.ctx().atom_table;
            let lexer = JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);
            let mut p = JSParserImpl::new(&gc, lexer);
            p.parse()
        };
        result.is_some()
        // gc drops here, releasing the GCLock so ctx can be borrowed below
    };

    if !parse_ok {
        eprintln!("Parse failed (errors: {})", sm.error_count());
        std::process::exit(1);
    }

    let num_nodes = ctx.num_nodes();
    let num_free  = ctx.num_free_nodes();
    let live_nodes = num_nodes.saturating_sub(num_free);
    let storage_bytes = ctx.storage_size();

    // Size of one StorageEntry: ctx_id_markbit (4) + count (4) + Node (varies).
    // We can compute it from storage_size / num_nodes if num_nodes > 0.
    let bytes_per_slot = if num_nodes > 0 {
        storage_bytes / num_nodes
    } else {
        0
    };

    println!("File:              {}", path);
    println!("File size:         {:.2} MiB  ({} bytes)", file_bytes as f64 / (1024.0 * 1024.0), file_bytes);
    println!("Node slots total:  {}", num_nodes);
    println!("  Free (GC freed): {}", num_free);
    println!("  Live nodes:      {}", live_nodes);
    println!("Storage bytes:     {:.2} MiB  ({} bytes)", storage_bytes as f64 / (1024.0 * 1024.0), storage_bytes);
    println!("Bytes / slot:      {} bytes", bytes_per_slot);
    println!("Nodes / KiB file:  {:.1}", live_nodes as f64 / (file_bytes as f64 / 1024.0));
    println!("Storage overhead:  {:.1}x file size", storage_bytes as f64 / file_bytes as f64);
}
