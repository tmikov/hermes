/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Memory-shape oracle for the PreParse scoped reclamation
//! (specs/2026-07-15-preparse-scoped-reclamation-design.md §5).
//!
//! `Context::num_nodes()` counts allocated slots and `AllocationScope`
//! truncation shrinks it, so post-pass counts expose the reclamation shape:
//! after `pre_parse_buffer` (whole-pass scope, cpp:7523) the retained count
//! must be near zero — NOT O(file), and not even O(keepers).

use hermes_ast::context::Context;
use hermes_parser::js::JSParserImpl;
use hermes_parser::lexer::{GrammarContext, JSLexer};
use hermes_support::manager::SourceErrorManager;

fn gen_source(n: usize) -> Vec<u8> {
    let mut src = Vec::new();
    for f in 0..n {
        src.extend_from_slice(format!("function f{f}(a, b) {{\n").as_bytes());
        for i in 0..20 {
            src.extend_from_slice(format!("  var x{i} = a + b * {i};\n").as_bytes());
        }
        src.extend_from_slice(b"  return a;\n}\n");
    }
    src
}

fn eager_nodes(src: &[u8]) -> usize {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer_bytes("t", src);
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let atoms = &gc.ctx().atom_table;
    let lexer = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
    let mut p = JSParserImpl::new(&gc, lexer);
    assert!(p.parse().is_some());
    gc.ctx().num_nodes()
}

fn preparse_nodes(src: &[u8]) -> (usize, usize) {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer_bytes("t", src);
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let atoms = &gc.ctx().atom_table;
    let lexer = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
    let p = JSParserImpl::pre_parse_buffer(&gc, lexer, false)
        .expect("preparse failed");
    let n = (gc.ctx().num_nodes(), gc.ctx().num_list_elements());
    // The table must still be populated (reclamation must not eat it).
    let mut p = p;
    assert_eq!(p.take_pre_parsed().function_info.len(), 200);
    n
}

#[test]
fn preparse_retains_no_ast() {
    let one = gen_source(1);
    let many = gen_source(200);
    let e1 = eager_nodes(&one);
    let e200 = eager_nodes(&many);
    let (p200_nodes, p200_elems) = preparse_nodes(&many);
    // After the whole-pass scope, essentially nothing is retained: less
    // than a single function's AST, and >20x under the full AST.
    assert!(p200_nodes < e1, "retained nodes: {p200_nodes} (one-fn = {e1})");
    assert!(p200_nodes * 20 < e200, "nodes {p200_nodes} vs eager {e200}");
    assert!(p200_elems < e1, "retained list elements: {p200_elems}");
}
