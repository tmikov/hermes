//! Golden / differential tests for the SourceErrorManager port.
//!
//! These pin the Rust diagnostics to the behavior of the C++ Hermes
//! `SourceErrorManager`, using two faithful references:
//!
//!   1. The end-to-end *structured* result (kind, 1-based line/col, message,
//!      file name, source line) delivered to a handler — the robust oracle.
//!   2. The exact *rendered* `file:line:col: kind: message` + caret format that
//!      the C++ implementation emits, as documented by the existing lit tests
//!      under `test/Parser/` (e.g. `:N:9: error: ';' expected` followed by a
//!      caret line of 8 spaces and `^`, and range underlines of the form
//!      `^~~~~~~`).
//!
//! NOTE: A full byte-for-byte differential against a live `hermes` binary is
//! deferred until a C++ build is available; the caret geometry asserted here
//! was independently verified against the `buildSourceAndCaretLine` algorithm
//! in `lib/Support/SourceErrorManager.cpp`.

use support::diag::{CollectingHandler, DiagKind, OutputOptions};
use support::location::SMLoc;
use support::manager::SourceErrorManager;
use support::render::build_source_and_caret_line;

/// End-to-end: an error at a known offset resolves to 1-based (line, col) with
/// the message/kind/file/source-line delivered to the handler — mirroring how
/// the C++ `SourceErrorManager` reports `t.js:1:9: error: ';' expected`.
#[test]
fn structured_error_resolves_like_cxx() {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("t.js", "let x = ;\n");
    sm.set_handler(Box::new(CollectingHandler::new()));
    // The ';' is at byte offset 8 -> line 1, col 9 (1-based).
    sm.error(
        SMLoc {
            source: id,
            offset: 8,
        },
        "';' expected",
    );

    let h = sm.handler_as::<CollectingHandler>().unwrap();
    assert_eq!(h.messages().len(), 1);
    let m = &h.messages()[0];
    assert_eq!(m.kind, DiagKind::Error);
    assert_eq!((m.line, m.col), (1, 9));
    assert_eq!(m.message, "';' expected");
    assert_eq!(m.file_name, "t.js");
    // Source line is delivered without the trailing EOL.
    assert_eq!(m.source_line.as_deref(), Some("let x = ;"));
}

/// A note attached to a preceding error resolves on the correct line, the way
/// Hermes reports multi-part diagnostics.
#[test]
fn error_then_note_on_second_line() {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("t.js", "first\nsecond\n");
    sm.set_handler(Box::new(CollectingHandler::new()));
    sm.error(
        SMLoc {
            source: id,
            offset: 0,
        },
        "an error",
    ); // line 1 col 1
    sm.note(
        SMLoc {
            source: id,
            offset: 6,
        },
        "see here",
    ); // 's' of "second"

    let h = sm.handler_as::<CollectingHandler>().unwrap();
    assert_eq!(h.messages().len(), 2);
    assert_eq!(h.messages()[0].kind, DiagKind::Error);
    assert_eq!((h.messages()[0].line, h.messages()[0].col), (1, 1));
    assert_eq!(h.messages()[1].kind, DiagKind::Note);
    assert_eq!((h.messages()[1].line, h.messages()[1].col), (2, 1));
}

/// Byte-compat: single caret format. Hermes prints (see `test/Parser/*.js`
/// CHECK-NEXT lines) a caret line of `col-1` spaces then `^`.
#[test]
fn single_caret_format_matches_cxx() {
    let (src, caret) = build_source_and_caret_line("let x = ;", 9, &[], &OutputOptions::default());
    assert_eq!(src, "let x = ;");
    assert_eq!(caret, "        ^"); // 8 spaces + '^'
}

/// Byte-compat: range underline format. Hermes underlines a range as `^`
/// followed by a run of `~` (see the `^~~~~~~` CHECK-NEXT lines in
/// `test/Parser/`).
#[test]
fn range_caret_format_matches_cxx() {
    let (_src, caret) =
        build_source_and_caret_line("let x = 1;", 5, &[(4, 9)], &OutputOptions::default());
    assert_eq!(caret, "    ^~~~~"); // 4 spaces, '^', then '~' across the range
}
