/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Pins the parser recursion-depth boundary at the unit level, independent of
//! whether `hermesc` is present: the LAST depth that parses cleanly and the
//! FIRST depth that reports `Too many nested expressions/statements/
//! declarations`, with the diagnostic rendered byte-for-byte the way the
//! oracle renders it.
//!
//! Three things can move what this file pins, and all three were real bugs
//! found by the recursion-parity audit (2026-08-04):
//!
//!   - the comparison in `check_recursion`: C++ increments first, then
//!     `recursionDepthCheck()` (JSParserImpl.h:699-704) errors unless the
//!     POST-increment depth is still `< MAX_RECURSION_DEPTH`, so the Rust
//!     test must be `>=`, not `>`. A `>` allows one extra level and shifts
//!     every recursion error by one production.
//!   - `MAX_RECURSION_DEPTH` itself.
//!   - the diagnostic's caret geometry — point vs range; see
//!     [`at_the_limit_renders_a_bare_caret_on_a_wide_token`], the only test
//!     here whose trip token is wider than one character.
//!
//! `#![cfg(debug_assertions)]`: `MAX_RECURSION_DEPTH` is profile-selected
//! (128 in debug, mirroring the C++ `HERMES_LIMIT_STACK_DEPTH` branch that
//! the project's standard ASan oracle build takes; 1024 in release — see the
//! constant's doc in `js/mod.rs`). The exact depths below therefore only hold
//! in the debug profile, which is the profile `cargo test` uses and the one
//! every differential gate runs. The corpus files
//! `parser/tests/parser_corpus/nested-parens-limit.js` (clean side),
//! `sema/tests/sema_corpus/nested-expressions.js` (error side) and
//! `sema/tests/sema_corpus/nested-{unary-multichar,tagged-template}-limit.js`
//! (the geometry, on both emitters) pin the same behavior against the real
//! oracles.
#![cfg(debug_assertions)]

use ast::context::Context;
use parser::js::JSParserImpl;
use parser::lexer::{GrammarContext, JSLexer};
use support::diag::CollectingHandler;
use support::manager::SourceErrorManager;
use support::render::render_diagnostic;

/// `N` nested parentheses around `1`, one token per line so the rendered
/// caret line stays short enough to pin literally.
fn paren_ladder(n: usize) -> String {
    let mut s = String::new();
    for _ in 0..n {
        s.push_str("(\n");
    }
    s.push_str("1\n");
    for _ in 0..n {
        s.push_str(")\n");
    }
    s
}

/// `N` `typeof` levels applied to a MULTI-CHARACTER identifier, one token per
/// line. Same depth accounting as [`paren_ladder`], but the token the limit
/// trips on is 5 characters wide, which is what makes the caret geometry
/// observable (see [`at_the_limit_renders_a_bare_caret_on_a_wide_token`]).
fn typeof_ladder(n: usize) -> String {
    "typeof\n".repeat(n) + "xyzzy;\n"
}

/// Parse `src` and return `(parsed_ok, rendered_diagnostics)`.
fn parse(src: &str) -> (bool, String) {
    let mut sm = SourceErrorManager::new();
    let opts = sm.output_options();
    sm.set_handler(Box::new(CollectingHandler::new()));
    let buf_id = sm.add_buffer("nested-parens.js", src);

    let mut ctx = Context::new();
    let gc = ctx.lock();
    let ok = {
        let atoms = &gc.ctx().atom_table;
        let lexer =
            JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut parser = JSParserImpl::new(&gc, lexer);
        parser.parse().is_some()
    };

    let rendered: String = sm
        .handler_as::<CollectingHandler>()
        .expect("CollectingHandler installed above")
        .messages()
        .iter()
        .map(|d| render_diagnostic(d, &opts))
        .collect();
    (ok, rendered)
}

/// Run `f` on a thread with an enlarged stack. 126 levels of *unoptimized*
/// recursive-descent frames exceed the 2 MiB the test harness gives a test
/// thread (the binaries do not hit this: a process main thread gets 8 MiB).
/// That is a debug-build property of the descent, not of the limit — which is
/// exactly what the limit is there to keep bounded.
fn on_a_big_stack(f: fn()) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(f)
        .expect("failed to spawn the deep-recursion test thread")
        .join()
        .expect("the deep-recursion test thread panicked");
}

/// `N* - 1` = 125 levels: the deepest ladder of this shape that still parses.
#[test]
fn one_below_the_limit_parses_cleanly() {
    on_a_big_stack(one_below_the_limit_parses_cleanly_impl);
}

fn one_below_the_limit_parses_cleanly_impl() {
    let (ok, rendered) = parse(&paren_ladder(125));
    assert!(ok, "125 nested parens must parse");
    assert_eq!(rendered, "", "125 nested parens must emit no diagnostic");
}

/// `N*` = 126 levels: the first ladder of this shape that trips the limit.
///
/// Verified byte-for-byte against `cmake-build-asan/bin/hermesc -dump-ast`
/// on the same source (the `Emitted N errors. exiting.` epilogue is a
/// `CompilerDriver` concern, not the parser's, so it is not part of this
/// parser-level pin). The reported token is the innermost `1`, not the last
/// `(`: the 126th `parsePrimaryExpression` is entered at depth 127, consumes
/// its `(`, and the recursive descent to the literal is the increment that
/// reaches 128.
#[test]
fn at_the_limit_reports_the_recursion_error() {
    on_a_big_stack(at_the_limit_reports_the_recursion_error_impl);
}

fn at_the_limit_reports_the_recursion_error_impl() {
    let (ok, rendered) = parse(&paren_ladder(126));
    assert!(!ok, "126 nested parens must fail to parse");
    assert_eq!(
        rendered,
        "nested-parens.js:127:1: error: Too many nested expressions/\
         statements/declarations\n\
         1\n\
         ^\n"
    );
}

/// The trip token's WIDTH must not reach the rendering: C++
/// `recursionDepthExceeded` (JSParserImpl.cpp:348-352) reports through
/// `error(tok_->getStartLoc(), …)`, the `error(SMLoc, Twine)` overload
/// (JSParserImpl.h:472-474), which renders a bare `^` — not the token's range,
/// which would underline it (`^~~~~`). Every other pin in this file and in the
/// parser corpus trips on a one-character token, where the two renderings are
/// indistinguishable; this one trips on `xyzzy`.
///
/// Verified byte-for-byte against `cmake-build-asan/bin/hermesc -dump-ast` on
/// the same source.
#[test]
fn at_the_limit_renders_a_bare_caret_on_a_wide_token() {
    on_a_big_stack(at_the_limit_renders_a_bare_caret_on_a_wide_token_impl);
}

fn at_the_limit_renders_a_bare_caret_on_a_wide_token_impl() {
    let (ok, rendered) = parse(&typeof_ladder(125));
    assert!(ok, "125 nested `typeof` must parse");
    assert_eq!(rendered, "", "125 nested `typeof` must emit no diagnostic");

    let (ok, rendered) = parse(&typeof_ladder(126));
    assert!(!ok, "126 nested `typeof` must fail to parse");
    assert_eq!(
        rendered,
        "nested-parens.js:127:1: error: Too many nested expressions/\
         statements/declarations\n\
         xyzzy;\n\
         ^\n"
    );
}
