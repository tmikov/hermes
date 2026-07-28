/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Pins the `errorExpected` combined-range caret fix (JSParserImpl.cpp:212-
//! 219, `combineIntoRange`) at the unit level, independent of whether
//! `hermesc` is present. `sema_differential.rs`'s `parse-error.js` (S1 task
//! 2) already exercises this end to end against the real oracle; this test
//! exists so the behavior stays pinned even in environments without a
//! `cmake-build-asan/bin/hermesc` (e.g. a plain `cargo test`).
//!
//! `var 1x;` reproduces the exact bug found while wiring up S1 task 2's
//! error-epilogue parity: `parseVariableDeclaration`'s
//! `errorExpected(identifier, "in declaration", "declaration started
//! here", declLoc)` call (JSParserImpl.cpp:1244-1250) has `declLoc` (the
//! `var` keyword's start) on the SAME source line as the error token
//! (`1`), so C++ renders ONE combined-range diagnostic: the caret sits at
//! the error token but the underline stretches back to `declLoc`
//! (`~~~~^`). The Rust port previously dropped `declLoc` entirely and
//! rendered only the current token's own range (`    ^~`) — this test
//! pins the fixed, byte-identical-to-hermesc output.

use ast::context::Context;
use parser::js::JSParserImpl;
use parser::lexer::{GrammarContext, JSLexer};
use support::diag::CollectingHandler;
use support::manager::SourceErrorManager;
use support::render::render_diagnostic;

#[test]
fn combined_range_caret_matches_hermesc() {
    let mut sm = SourceErrorManager::new();
    let opts = sm.output_options();
    sm.set_handler(Box::new(CollectingHandler::new()));
    let buf_id = sm.add_buffer("var-1x.js", "var 1x;\n");

    let mut ctx = Context::new();
    let gc = ctx.lock();
    {
        let atoms = &gc.ctx().atom_table;
        let lexer =
            JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut parser = JSParserImpl::new(&gc, lexer);
        assert!(parser.parse().is_none(), "`var 1x;` must fail to parse");
    }

    let rendered: String = sm
        .handler_as::<CollectingHandler>()
        .expect("CollectingHandler installed above")
        .messages()
        .iter()
        .map(|d| render_diagnostic(d, &opts))
        .collect();

    // Verified byte-for-byte against `cmake-build-asan/bin/hermesc
    // -dump-ast var-1x.js` (the "Emitted N errors. exiting." epilogue is a
    // `CompilerDriver`/`sema-dump`-level concern, not the parser's, so it
    // is not part of this parser-level pin).
    assert_eq!(
        rendered,
        "var-1x.js:1:5: error: invalid numeric literal\n\
         var 1x;\n    \
         ^~\n\
         var-1x.js:1:5: error: 'identifier' expected in declaration\n\
         var 1x;\n\
         ~~~~^\n"
    );
}
