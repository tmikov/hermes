/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Walk a parsed AST with the read-only `Visitor` and print a node-kind
//! histogram.
//!
//! ```text
//! cargo run -p parser --example walk_ast
//! ```

use std::collections::HashMap;

use ast::node::{Node, NodeKind};
use ast::visitor::Visitor;
use parser::{parse, ParseFlags};

/// Counts how many nodes of each kind the tree contains.
struct Histogram {
    counts: HashMap<NodeKind, usize>,
}

impl<'gc> Visitor<'gc> for Histogram {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        *self.counts.entry(node.kind()).or_default() += 1;
        // The default `visit_node` does exactly this; recursion is ours to
        // control, so a visitor can prune subtrees by not calling it.
        node.visit_children(self);
    }
}

const SOURCE: &str = r#"
class Counter {
  #n = 0;
  increment(by = 1) {
    this.#n += by;
    return this.#n;
  }
}

const c = new Counter();
for (const step of [1, 2, 3]) {
  console.log(`${step} -> ${c.increment(step)}`);
}
"#;

fn main() {
    let flags = ParseFlags::default();
    let mut parsed = parse(SOURCE, flags).expect("snippet must parse");

    // The AST is only reachable while the arena is locked, so collect owned
    // data inside the closure and return it.
    let counts = parsed.with_program(|_gc, program| {
        let mut hist = Histogram {
            counts: HashMap::new(),
        };
        hist.visit_node(program);
        hist.counts
    });

    let mut rows: Vec<(NodeKind, usize)> = counts.into_iter().collect();
    // Most frequent first; ties broken by kind name for a stable listing.
    rows.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| format!("{:?}", a.0).cmp(&format!("{:?}", b.0)))
    });

    let total: usize = rows.iter().map(|(_, n)| n).sum();
    println!("{total} nodes, {} distinct kinds", rows.len());
    for (kind, n) in rows {
        println!("{:<24} {:>3} {}", format!("{kind:?}"), n, "#".repeat(n));
    }
}
