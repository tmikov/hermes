/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Pins BOTH geometry arms of C++ `errorExpected`
//! (JSParserImpl.cpp:201-225) at the unit level, independent of whether
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

/// Parse `src` (which must fail) as `name` and return every diagnostic it
/// produced, rendered exactly as the driver would print it.
fn render_parse_errors(name: &str, src: &str) -> String {
    let mut sm = SourceErrorManager::new();
    let opts = sm.output_options();
    sm.set_handler(Box::new(CollectingHandler::new()));
    let buf_id = sm.add_buffer(name, src);

    let mut ctx = Context::new();
    let gc = ctx.lock();
    {
        let atoms = &gc.ctx().atom_table;
        let lexer =
            JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut parser = JSParserImpl::new(&gc, lexer);
        assert!(parser.parse().is_none(), "`{src}` must fail to parse");
    }

    sm.handler_as::<CollectingHandler>()
        .expect("CollectingHandler installed above")
        .messages()
        .iter()
        .map(|d| render_diagnostic(d, &opts))
        .collect()
}

#[test]
fn combined_range_caret_matches_hermesc() {
    // Verified byte-for-byte against `cmake-build-asan/bin/hermesc
    // -dump-ast var-1x.js` (the "Emitted N errors. exiting." epilogue is a
    // `CompilerDriver`/`sema-dump`-level concern, not the parser's, so it
    // is not part of this parser-level pin).
    assert_eq!(
        render_parse_errors("var-1x.js", "var 1x;\n"),
        "var-1x.js:1:5: error: invalid numeric literal\n\
         var 1x;\n    \
         ^~\n\
         var-1x.js:1:5: error: 'identifier' expected in declaration\n\
         var 1x;\n\
         ~~~~^\n"
    );
}

/// The same-line arm (cpp:212-219): `whatLoc` (the `(`) and the error token
/// (`;`) share a line, so C++ emits ONE diagnostic whose underline is
/// `combineIntoRange(whatLoc, errorLoc)` — tildes from the `(` up to the
/// caret. Verified byte-for-byte against `hermesc -dump-sema`.
#[test]
fn same_line_combines_what_loc_into_the_range() {
    assert_eq!(
        render_parse_errors("paren-expr.js", "var a = (1 + 2;\n"),
        "paren-expr.js:1:15: error: ')' expected at end of parenthesized \
         expression\n\
         var a = (1 + 2;\n        \
         ~~~~~~^\n"
    );
}

/// The different-line arm (cpp:220-225): `whatLoc` (the `try`) is on an
/// earlier line than the error token (`xyz`), so C++ emits a bare
/// point-caret error — NOT the error token's own range — followed by a
/// separate `note` carrying `what` at `whatLoc`. Verified byte-for-byte
/// against `hermesc -dump-sema`.
#[test]
fn different_line_emits_a_note_at_what_loc() {
    assert_eq!(
        render_parse_errors("try-nl.js", "try\nxyz;\n"),
        "try-nl.js:2:1: error: '{' expected after 'try'\n\
         xyz;\n\
         ^\n\
         try-nl.js:1:1: note: location of 'try'\n\
         try\n\
         ^\n"
    );
}
