/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Parse a JavaScript file, resolve it, and print what came out.
//!
//! ```text
//! cargo run -p hermes-sema --example resolve_and_dump -- file.js
//! cargo run -p hermes-sema --example resolve_and_dump -- --summary file.js
//! ```
//!
//! The default output is the `hermesc -dump-sema` text — the `SemContext`
//! followed by the AST annotated with each identifier's resolution — which is
//! the same dump this crate's differential gate compares byte-for-byte.
//! `--summary` prints a short human-readable count instead, to show the
//! `SemContext` being queried rather than dumped.
//!
//! This is the whole façade in ~20 lines of real code: `parse` (from
//! `hermes-parser`), `resolve_for_compile` (this crate), then read the result.

use std::io::Write;
use std::process::ExitCode;

use hermes_parser::{parse_named, ParseFlags};
use hermes_sema::{resolve_for_compile, CompileOptions, ResolvedJS};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut summary = false;
    let mut path = None;
    for arg in args.by_ref() {
        match arg.as_str() {
            "--summary" => summary = true,
            _ => path = Some(arg),
        }
    }
    let Some(path) = path else {
        eprintln!("usage: resolve_and_dump [--summary] <file.js>");
        return ExitCode::from(1);
    };

    let source = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("resolve_and_dump: cannot read '{path}': {e}");
            return ExitCode::from(1);
        }
    };

    // Step 1: parse. `ParseFlags::default()` is plain ECMAScript; set
    // `parse_flow`, `parse_ts` or `parse_jsx` for the other dialects.
    let parsed = match parse_named(&source, &path, ParseFlags::default()) {
        Ok(parsed) => parsed,
        Err(e) => {
            for m in e.messages() {
                eprint!("{m}");
            }
            return ExitCode::from(2);
        }
    };

    // Step 2: resolve. The compile path, with the standard globals declared —
    // what `hermesc -dump-sema` does. `hermes_sema::resolve` is the parser
    // path instead: no ambient declarations and no AST rewrites.
    let options = CompileOptions::default();
    let mut resolved = match resolve_for_compile(parsed, &options) {
        Ok(resolved) => resolved,
        Err(e) => {
            for m in e.messages() {
                eprint!("{m}");
            }
            return ExitCode::from(2);
        }
    };

    // Warnings, if any: resolution succeeded, so none of these is an error.
    for d in resolved.diagnostics() {
        eprintln!("{}:{}:{}: {}", d.file_name, d.line, d.col, d.message);
    }

    // Step 3: read the result.
    if summary {
        print_summary(&mut resolved);
    } else {
        // Bytes, not a `String`: an identifier can be an unpaired surrogate,
        // which the dumper writes as WTF-8.
        let dump = resolved.to_sema_dump();
        std::io::stdout().write_all(&dump).expect("write failed");
    }
    ExitCode::SUCCESS
}

/// Walk the tree and count what resolution decided, using the `SemContext`
/// the way a consumer would.
fn print_summary(resolved: &mut ResolvedJS) {
    use hermes_parser::ast::node::Node;
    use hermes_parser::ast::visitor::Visitor;

    /// Counts identifier *expressions* by whether they resolved.
    struct Counter<'a> {
        sem: &'a hermes_sema::sem_context::SemContext,
        resolved: usize,
        unresolved: usize,
    }
    impl<'gc> Visitor<'gc> for Counter<'_> {
        fn visit_node(&mut self, node: &'gc Node<'gc>) {
            if let Node::Identifier(id) = node {
                match self.sem.get_expression_decl(id) {
                    Some(_) => self.resolved += 1,
                    None => self.unresolved += 1,
                }
            }
            node.visit_children(self);
        }
    }

    let (resolved_ids, unresolved_ids) =
        resolved.with_program(|_gc, root, sem| {
            let mut c = Counter {
                sem,
                resolved: 0,
                unresolved: 0,
            };
            c.visit_node(root);
            (c.resolved, c.unresolved)
        });

    let sem = resolved.sem_context();
    println!("functions:            {}", sem.functions_len());
    println!("resolved references:  {resolved_ids}");
    println!("other identifiers:    {unresolved_ids}");
}
