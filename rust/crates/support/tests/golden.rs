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
//! The `hermesc_*` tests below are a true byte-for-byte differential: the
//! expected strings were captured from a real C++ Hermes `hermesc` build
//! (cmake-build-asan, hermesc 1.96.0) via
//! `(! cmake-build-asan/bin/hermesc -dump-ast FILE 2>&1)` with the file path
//! normalized to `FILE`. The Rust port must reproduce that stderr exactly.

use hermes_support::diag::{CollectingHandler, DiagKind, OutputOptions};
use hermes_support::location::{SMLoc, SMRange};
use hermes_support::manager::SourceErrorManager;
use hermes_support::render::{build_source_and_caret_line, render_diagnostic};

/// Render the single diagnostic emitted into a fresh manager with `FILE` as the
/// buffer name, using default (color-off) options — matching how `hermesc`
/// prints to a non-TTY stderr.
fn render_one(
    source: &str,
    emit: impl FnOnce(&mut SourceErrorManager, hermes_support::location::SourceId),
) -> String {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("FILE", source);
    sm.set_handler(Box::new(CollectingHandler::new()));
    emit(&mut sm, id);
    let h = sm.handler_as::<CollectingHandler>().unwrap();
    assert_eq!(h.messages().len(), 1, "expected exactly one diagnostic");
    render_diagnostic(&h.messages()[0], &OutputOptions::default())
}

/// Differential vs hermesc: single caret, no range.
#[test]
fn hermesc_single_caret() {
    let out = render_one("let x = ;", |sm, id| {
        sm.error(
            SMLoc {
                source: id,
                offset: 8,
            },
            "invalid expression",
        );
    });
    assert_eq!(
        out,
        "FILE:1:9: error: invalid expression\nlet x = ;\n        ^\n"
    );
}

/// Differential vs hermesc: ranged underline `^~~~~` over `await` (5 chars).
#[test]
fn hermesc_ranged_underline() {
    let out = render_one("(async await => 3);", |sm, id| {
        sm.error_range(
            SMRange {
                start: SMLoc {
                    source: id,
                    offset: 7,
                },
                end: SMLoc {
                    source: id,
                    offset: 12,
                },
            },
            "Unexpected usage of 'await' as an identifier",
        );
    });
    assert_eq!(
        out,
        "FILE:1:8: error: Unexpected usage of 'await' as an identifier\n\
         (async await => 3);\n       ^~~~~\n"
    );
}

/// Differential vs hermesc: a source line with tabs is tab-expanded (TabStop 8)
/// in both the source and caret lines.
#[test]
fn hermesc_tab_expansion() {
    let out = render_one("\tlet y =\t;", |sm, id| {
        sm.error(
            SMLoc {
                source: id,
                offset: 9,
            },
            "invalid expression",
        );
    });
    assert_eq!(
        out,
        "FILE:1:10: error: invalid expression\n        let y = ;\n                ^\n"
    );
}

/// Differential vs hermesc: a non-ASCII source line prints the source but NO
/// caret line (Hermes punts on caret widths for non-ASCII).
#[test]
fn hermesc_non_ascii_no_caret() {
    let out = render_one("var héllo = ;", |sm, id| {
        sm.error(
            SMLoc {
                source: id,
                offset: 13,
            },
            "invalid expression",
        );
    });
    assert_eq!(out, "FILE:1:14: error: invalid expression\nvar héllo = ;\n");
}

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
