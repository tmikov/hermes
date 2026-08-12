/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Pins the three parser-side upstream C++ defect fixes mirrored into the
//! port, one test per fix. Each is the Rust mirror of the C++ test the fix
//! commit added:
//!
//!   - `37520ccef` "Fix rejection of member expressions as JSX attribute
//!     names" (`test/Parser/jsx-error-attr-member.js`) — the
//!     `parseJSXElementName` check tested `MemberExpressionNode`, which can
//!     never match a JSX name, so `<foo a.b="1"/>` was silently accepted.
//!   - `550aafe33` "Fix crash after reporting a bad match binding pattern"
//!     (`test/Parser/flow/match/pattern-binding-error.js`) — after reporting
//!     `'identifier' expected in match binding pattern` the parser kept going
//!     and read the identifier off the current, non-identifier token,
//!     tripping `Token::getResWordOrIdentifier`'s assert (defect 11 in
//!     `doc/superpowers/CppDefectsFound.md`; the port panicked identically in
//!     `Token::get_res_word_or_identifier`, bug-for-bug). The pin is flipped
//!     here: the same input must now produce the diagnostic and recover
//!     cleanly, with no panic.
//!   - `b21856de4` "Add a recursion limit to the compiler-side JSONParser"
//!     (`unittests/AST/JSONTest.cpp`'s `DeepNestingTest`) — deeply nested
//!     JSON overflowed the native stack; it must report `Too many nested JSON
//!     values` instead.
//!
//! Every expected rendering below was captured from
//! `cmake-build-asan/bin/hermesc` built from the cherry-picked fixes (the
//! `Emitted N errors. exiting.` epilogue is a driver-level concern, not the
//! parser's, so it is not part of these parser-level pins). The JSX case is
//! additionally pinned end-to-end against the live oracle by
//! `sema/tests/sema_corpus/jsx-error-attr-member.js`, and the match case by
//! `sema/tests/sema_corpus/flow-match-pattern-binding-error.js`.

use ast::context::Context;
use atom_table::AtomTable;
use bumpalo::Bump;
use parser::js::JSParserImpl;
use parser::json::{JSONFactory, JSONParser};
use parser::lexer::{GrammarContext, JSLexer};
use support::diag::CollectingHandler;
use support::manager::SourceErrorManager;
use support::render::render_diagnostic;

/// Which dialect flags to enable for [`render_parse_errors`], named after the
/// `hermesc` flags they mirror.
enum Dialect {
    /// `-parse-jsx`.
    Jsx,
    /// `-parse-flow -Xparse-flow-match`.
    FlowMatch,
}

/// Parse `src` (which must fail) as `name` in `dialect` and return every
/// diagnostic it produced, rendered exactly as the driver would print it.
fn render_parse_errors(dialect: Dialect, name: &str, src: &str) -> String {
    let mut sm = SourceErrorManager::new();
    let opts = sm.output_options();
    sm.set_handler(Box::new(CollectingHandler::new()));
    let buf_id = sm.add_buffer(name, src);

    let mut ctx = Context::new();
    match dialect {
        Dialect::Jsx => ctx.set_parse_jsx(true),
        Dialect::FlowMatch => {
            // `-parse-flow` defaults to `ParseFlowSetting::ALL`, i.e. the
            // ambiguous-expression grammar too; `-Xparse-flow-match` implies
            // `-parse-flow` (matching `tools/src/bin/ast_dump.rs`'s wiring).
            ctx.set_parse_flow(true);
            ctx.set_parse_flow_ambiguous(true);
            ctx.set_parse_flow_match(true);
        }
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

/// `37520ccef`: a JSX member expression is not a valid attribute name. The
/// error's range is the whole `a.b` (caret at its start, tildes over the
/// rest) because it is reported at `name->getSourceRange()`.
#[test]
fn jsx_member_expression_attribute_name_is_rejected() {
    assert_eq!(
        render_parse_errors(
            Dialect::Jsx,
            "jsx-error-attr-member.js",
            "<foo a.b=\"1\"></foo>\n"
        ),
        "jsx-error-attr-member.js:1:6: error: unexpected member expression\n\
         <foo a.b=\"1\"></foo>\n     \
         ^~~\n"
    );
}

/// `550aafe33`: the binding pattern's identifier is missing. Exactly ONE
/// diagnostic, and — the point of the fix — no panic on the way out: the
/// pattern parser returns `None` instead of falling through to
/// `parse_match_binding_identifier_flow`, which would call
/// `Token::get_res_word_or_identifier` on the `[` token.
///
/// The geometry is `errorExpected`'s same-line arm: `whatLoc` (the `const`
/// that started the binding pattern) and the error token (`[`) share a line,
/// so the underline runs from the `const` up to the caret.
#[test]
fn match_binding_pattern_without_identifier_recovers_cleanly() {
    assert_eq!(
        render_parse_errors(
            Dialect::FlowMatch,
            "pattern-binding-error.js",
            "const e = match (x) { const [y]: 2 };\n"
        ),
        "pattern-binding-error.js:1:29: error: 'identifier' expected in \
         match binding pattern\n\
         const e = match (x) { const [y]: 2 };\n                      \
         ~~~~~~^\n"
    );
}

/// Parse `src` as JSON and return `(parsed, error messages)`.
fn parse_json(src: &str) -> (bool, Vec<String>) {
    let arena = Bump::new();
    let atoms = AtomTable::new();
    let mut sm = SourceErrorManager::new();
    sm.set_handler(Box::new(CollectingHandler::new()));
    let buf_id = sm.add_buffer("deep.json", src);
    let parsed = {
        let f = arena.alloc(JSONFactory::new(&arena, &atoms));
        let mut p = JSONParser::new(f, buf_id, &mut sm, &atoms, false);
        p.parse().is_some()
    };
    let messages = sm
        .handler_as::<CollectingHandler>()
        .expect("CollectingHandler installed above")
        .messages()
        .iter()
        .map(|d| d.message.clone())
        .collect();
    (parsed, messages)
}

/// `b21856de4`: nesting past `MAX_RECURSION_DEPTH` is an error, not a stack
/// overflow. 2000 is past the limit in BOTH build profiles (128 debug / 1024
/// release), so this test does not need the profile gate that
/// `recursion_depth_limit.rs` carries; the C++ `DeepNestingTest` uses 100000
/// for the same reason.
#[test]
fn deeply_nested_json_is_rejected() {
    let (parsed, messages) = parse_json(&"[".repeat(2000));
    assert!(!parsed, "deeply nested JSON must not parse");
    assert_eq!(messages, vec!["Too many nested JSON values".to_string()]);
}

/// The clean side of the same boundary: 100 levels are under the limit in
/// both profiles and must still parse, so the check above cannot pass by
/// rejecting everything.
#[test]
fn moderately_nested_json_still_parses() {
    let src = format!("{}{}", "[".repeat(100), "]".repeat(100));
    let (parsed, messages) = parse_json(&src);
    assert!(parsed, "100 levels must parse: {messages:?}");
    assert!(messages.is_empty(), "{messages:?}");
}
