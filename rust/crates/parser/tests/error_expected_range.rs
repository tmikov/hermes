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
    render_parse_errors_impl(name, src, false)
}

/// Like `render_parse_errors`, but with Flow syntax enabled (`-parse-flow`
/// on both `ctx.set_parse_flow` and `ctx.set_parse_flow_ambiguous`, matching
/// how `bin/ast_dump.rs` wires up `--parse-flow`).
fn render_flow_parse_errors(name: &str, src: &str) -> String {
    render_parse_errors_impl(name, src, true)
}

fn render_parse_errors_impl(name: &str, src: &str, flow: bool) -> String {
    let mut sm = SourceErrorManager::new();
    let opts = sm.output_options();
    sm.set_handler(Box::new(CollectingHandler::new()));
    let buf_id = sm.add_buffer(name, src);

    let mut ctx = Context::new();
    if flow {
        ctx.set_parse_flow(true);
        ctx.set_parse_flow_ambiguous(true);
    }
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

/// The Flow generic-arrow "no `=>` follows" diagnostic (cpp:6468-6477):
/// `errorExpected(equalgreater, "in generic arrow function", "start of
/// function", typeParams->getStartLoc())`. `typeParams` (the `<T>`) and the
/// error token (`foobar`) share line 1, so this is the same-line arm: ONE
/// combined-range diagnostic, tildes from `<` through one past `foobar`'s
/// start.
///
/// ORACLE-FREE BY NECESSITY, not preference: `hermesc -dump-ast
/// -dump-source-location=both -parse-flow` on this exact input exits 2 with
/// EMPTY stdout/stderr (verified directly) — C++ reaches this call while
/// still inside the `CollectMessagesRAII` scope opened for the type-param
/// speculative retry (JSParserImpl.cpp:6292; `parseAssignmentExpression`,
/// the flow-typed-arrow-head backtrack), and that scope's destructor
/// discards every message on this path (only the retry's success path calls
/// `setDiscardMessages(false)`). A corpus/differential file compares actual
/// hermesc stderr byte-for-byte, so it cannot pin a rendering hermesc itself
/// never produces.
///
/// The second message below ("type parameters must be used...", the
/// pre-existing port of cpp:6329-6330) is real, deterministic output of the
/// current parser, not test noise: both messages fire while the Rust port's
/// own `begin_collecting`/`end_collecting` pairing around the retry
/// (`expressions.rs`, `parse_assignment_expression`'s `run_level` closure,
/// ~line 442-461) is already closed by the time either one is emitted — the
/// Rust port ends+discards the FIRST attempt's collection scope before the
/// retry starts, rather than keeping ONE collection scope open across the
/// whole speculative block the way C++'s single RAII object does. So,
/// unlike C++, neither message here is ever collected/discardable in the
/// Rust port — both reach the handler directly. That ordering gap is a
/// separate, pre-existing bug (predates this call site's geometry
/// restoration); this test pins today's real rendering and documents the
/// gap rather than silently masking it. Fixing it is a
/// `begin_collecting`/`end_collecting` restructure (keep one collection
/// scope open across the whole retry, mirroring the single C++
/// `CollectMessagesRAII`), not a geometry change — tracked here pending a
/// dedicated backlog entry.
#[test]
fn flow_generic_arrow_missing_fat_arrow_combines_into_range() {
    assert_eq!(
        render_flow_parse_errors(
            "generic-arrow.js",
            "const f = <T>(x: T) foobar;\n"
        ),
        "generic-arrow.js:1:21: error: '=>' expected in generic arrow \
         function\n\
         const f = <T>(x: T) foobar;\n          \
         ~~~~~~~~~~^\n\
         generic-arrow.js:1:11: error: type parameters must be used in an \
         arrow function expression\n\
         const f = <T>(x: T) foobar;\n          \
         ^~~\n"
    );
}
