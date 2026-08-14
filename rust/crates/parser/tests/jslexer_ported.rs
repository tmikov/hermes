//! Faithful Rust port of the C++ `unittests/Parser/JSLexerTest.cpp` suite.
//!
//! Each test below mirrors one `TEST(JSLexerTest, ...)` case, asserting the
//! SAME observable behavior (token kinds, numeric/string/identifier/template/
//! regexp/bigint values, the newline-before-current-token flag, error/warning
//! counts and — where the C++ checks them — message text, lookahead results,
//! SavePoint, directives, stored comments, prevTokenEndLoc).
//!
//! ## Mapping notes (C++ -> Rust)
//! - `DiagContext diag(sm)` + `diag.getErrCountClear()` / `getWarnCountClear()`
//!   are modeled by [`Diag`], a small delta-counter over the manager's
//!   cumulative `error_count()` / `warning_count()`. Reading and clearing a
//!   delta is exactly the DiagContext semantics. When the C++ checks a message
//!   STRING/coords, a `CollectingHandler` is installed and the captured
//!   `ResolvedDiagnostic` is asserted.
//! - C++ `lex.advance()` defaults to `AllowRegExp`; `lex.advance(AllowDiv)` is
//!   explicit. The Rust `advance` takes the grammar context explicitly.
//! - C++ `lookahead1(llvh::None)` uses the parser default
//!   `RequireNoNewLine=true`, so the Rust call is `lookahead1::<true>(None)`.
//! - Interned values are `AtomBytes`; `tab.bytes(atom)` yields the bytes the
//!   C++ `->str()` / `->c_str()` returns. Comparisons are against byte strings.
//! - Tests whose source contains raw (non-UTF-8 / control) bytes use
//!   `add_buffer_bytes`.
//!
//! DEVIATION (pointer -> offset): `PrevTokenEndLocTest` compares `SMLoc`
//! offsets instead of raw `const char *` pointers; the Rust lexer is
//! offset-based. The behavior verified (which token's end the prev-end tracks)
//! is identical.

use hermes_atom_table::AtomTable;
use hermes_parser::lexer::{GrammarContext, JSLexer};
use hermes_parser::token_kinds::TokenKind;
use hermes_support::diag::{CollectingHandler, DiagKind};
use hermes_support::manager::SourceErrorManager;

/// Delta error/warning counter that mirrors C++ `DiagContext`.
/// `err_clear()` / `warn_clear()` return the number of errors/warnings emitted
/// since the previous clear, exactly like `getErrCountClear`/`getWarnCountClear`.
struct Diag {
    last_err: u32,
    last_warn: u32,
}

impl Diag {
    fn new() -> Diag {
        Diag {
            last_err: 0,
            last_warn: 0,
        }
    }
    /// Errors emitted since the last clear (and reset the delta baseline).
    fn err_clear(&mut self, lex: &JSLexer) -> u32 {
        let cur = lex.get_source_mgr().error_count();
        let d = cur - self.last_err;
        self.last_err = cur;
        d
    }
    /// Warnings emitted since the last clear (and reset the delta baseline).
    fn warn_clear(&mut self, lex: &JSLexer) -> u32 {
        let cur = lex.get_source_mgr().warning_count();
        let d = cur - self.last_warn;
        self.last_warn = cur;
        d
    }
}

/// Make a (SourceErrorManager, buffer-id) pair for a UTF-8 source.
fn mk(src: &str) -> (SourceErrorManager, hermes_support::location::SourceId) {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("t", src);
    (sm, id)
}

/// Make a (SourceErrorManager, buffer-id) pair for a raw-byte source.
fn mk_bytes(src: &[u8]) -> (SourceErrorManager, hermes_support::location::SourceId) {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer_bytes("t", src);
    (sm, id)
}

// ---------------------------------------------------------------------------
// PunctuatorTest / PunctuatorDivTest
// ---------------------------------------------------------------------------

/// The ordered (kind, spelling) pairs the C++ `#define PUNCTUATOR(name, str)`
/// iteration covers in TokenKinds.def. BINOP expands to PUNCTUATOR; the two
/// PUNCTUATOR_FLOW tokens (`{|`, `|}`) are NOT part of the PUNCTUATOR macro and
/// are therefore excluded, exactly as in the C++ test.
fn punctuators() -> Vec<(TokenKind, &'static str)> {
    use TokenKind::*;
    vec![
        (l_brace, "{"),
        (r_brace, "}"),
        (l_paren, "("),
        (r_paren, ")"),
        (l_square, "["),
        (r_square, "]"),
        (period, "."),
        (questiondot, "?."),
        (dotdotdot, "..."),
        (semi, ";"),
        (comma, ","),
        (plusplus, "++"),
        (minusminus, "--"),
        // BINOP run.
        (starstar, "**"),
        (star, "*"),
        (percent, "%"),
        (slash, "/"),
        (plus, "+"),
        (minus, "-"),
        (lessless, "<<"),
        (greatergreater, ">>"),
        (greatergreatergreater, ">>>"),
        (less, "<"),
        (greater, ">"),
        (lessequal, "<="),
        (greaterequal, ">="),
        (equalequal, "=="),
        (exclaimequal, "!="),
        (equalequalequal, "==="),
        (exclaimequalequal, "!=="),
        (amp, "&"),
        (caret, "^"),
        (pipe, "|"),
        (ampamp, "&&"),
        (pipepipe, "||"),
        (questionquestion, "??"),
        // remaining PUNCTUATORs.
        (exclaim, "!"),
        (tilde, "~"),
        (question, "?"),
        (colon, ":"),
        (equal, "="),
        (plusequal, "+="),
        (minusequal, "-="),
        (starequal, "*="),
        (starstarequal, "**="),
        (percentequal, "%="),
        (slashequal, "/="),
        (lesslessequal, "<<="),
        (greatergreaterequal, ">>="),
        (greatergreatergreaterequal, ">>>="),
        (ampequal, "&="),
        (pipeequal, "|="),
        (ampampequal, "&&="),
        (pipepipeequal, "||="),
        (questionquestionequal, "??="),
        (caretequal, "^="),
        (equalgreater, "=>"),
        (at, "@"),
    ]
}

#[test]
fn punctuator_test() {
    // Build "str str str ..." like the C++ `puncts[]` (each str + " ").
    let puncts = punctuators();
    let mut src = String::new();
    for (_, s) in &puncts {
        src.push_str(s);
        src.push(' ');
    }
    let (mut sm, id) = mk(&src);
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);

    // "/=" and "/" require AllowDiv or they could be a regexp literal.
    for (kind, _) in &puncts {
        assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), *kind);
    }
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);

    // Mirror the `isPunctuatorDbg` assertions (Rust: is_punctuator).
    for (kind, _) in &puncts {
        assert!(kind.is_punctuator());
    }
}

#[test]
fn punctuator_div_test() {
    let (mut sm, id) = mk("a / b /= c");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::slash);
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::slashequal);
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
}

// ---------------------------------------------------------------------------
// WhiteSpaceTest / UnicodeWhiteSpaceTest
// ---------------------------------------------------------------------------

#[test]
fn white_space_test() {
    let (mut sm, id) = mk("{  ; \n} \n \n ;");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::l_brace);
    assert!(!lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);
    assert!(!lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::r_brace);
    assert!(lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);
    assert!(lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
    assert!(!lex.is_new_line_before_current_token());
}

#[test]
fn unicode_white_space_test() {
    // Source with unicode whitespace characters (same shape as WhiteSpaceTest).
    let (mut sm, id) = mk_bytes(b"{\xe2\x80\x80;\xe2\x80\x8a \n} \xe2\x81\x9f\n \n ;");
    let tab = AtomTable::new();
    let mut diag = Diag::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::l_brace);
    assert!(!lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);
    assert!(!lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::r_brace);
    assert!(lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);
    assert!(lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
    assert!(!lex.is_new_line_before_current_token());

    assert_eq!(diag.err_clear(&lex), 0);
}

// ---------------------------------------------------------------------------
// CommentTest / HashbangTest
// ---------------------------------------------------------------------------

#[test]
fn comment_test() {
    let (mut sm, id) = mk(
        "; /* foo */ { /* bar \n\
         \x20   *****      */ } // hello\n\
         \x20/* comment */ ;\n\
         \x20/* not closed",
    );
    let tab = AtomTable::new();
    let mut diag = Diag::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);
    assert!(!lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::l_brace);
    assert!(!lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::r_brace);
    assert!(lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);
    assert!(lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
    assert_eq!(diag.err_clear(&lex), 1); // comment not closed
    assert!(lex.is_new_line_before_current_token());
}

#[test]
fn hashbang_test() {
    let (mut sm, id) = mk(
        "#! hashbang comment\n\
         ;\n\
         #! // not a hashbang comment\n",
    );
    let tab = AtomTable::new();
    let mut diag = Diag::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);
    assert!(lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::exclaim);
    assert_eq!(diag.err_clear(&lex), 1);
    assert!(lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
    assert!(lex.is_new_line_before_current_token());
}

// ---------------------------------------------------------------------------
// NumberTest / NumericSeparatorTest
// ---------------------------------------------------------------------------

/// The (string, expected f64) pairs from the C++ `_GEN_TESTS` macro. `_FLT`
/// parses with the C++ float strtod path; `_DEC` parses hex/octal integers.
/// The expected values are literal Rust f64 constants computed the same way.
fn number_cases() -> Vec<(&'static str, f64)> {
    vec![
        ("1235", 1235.0),
        ("1234567890123", 1234567890123.0),
        ("0", 0.0),
        ("0x10", 16.0),
        ("1.2", 1.2),
        ("055", 45.0), // octal 055 == 45
        (".1", 0.1),
        ("1.", 1.0),
        ("1e2", 1e2),
        ("5e+3", 5e3),
        ("4e-3", 4e-3),
        (".1e-3", 0.1e-3),
        ("12.34e+5", 12.34e5),
    ]
}

#[test]
fn number_test() {
    // C++ constructs with strictMode=false (the `false` ctor arg) so that the
    // legacy octal `055` lexes without a strict-mode error.
    let mut src = String::new();
    for (s, _) in number_cases() {
        src.push(' ');
        src.push_str(s);
    }
    let (mut sm, id) = mk(&src);
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
    lex.set_strict_mode(false);

    for (s, expected) in number_cases() {
        let tok = lex.advance(GrammarContext::AllowDiv);
        assert_eq!(tok.kind(), TokenKind::numeric_literal, "src={s:?}");
        // Bitwise-equal comparison, mirroring `bitwiseIsEqual`.
        assert_eq!(
            tok.get_numeric_literal().to_bits(),
            expected.to_bits(),
            "value mismatch for {s:?}"
        );
    }
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::eof);
}

#[test]
fn numeric_separator_test() {
    // Pairs: with-separator then without; both must lex to the same f64.
    let (mut sm, id) = mk(
        " 1_2 12\
         \x20 0x1_2 0x12\
         \x20 0xdead_beef 0xdeadbeef\
         \x20 0b1_1 0b11\
         \x20 0o1_1 0o11\
         \x20 123_456_789 123456789\
         \x20 12_345e1_2 12345e12\
         \x20 1_1.1_2 1_1.12",
    );
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);

    loop {
        let tok = lex.advance(GrammarContext::AllowDiv);
        if tok.kind() == TokenKind::eof {
            break;
        }
        assert_eq!(tok.kind(), TokenKind::numeric_literal);
        let with_sep = tok.get_numeric_literal();

        let tok = lex.advance(GrammarContext::AllowDiv);
        assert_eq!(tok.kind(), TokenKind::numeric_literal);
        let no_sep = tok.get_numeric_literal();

        assert_eq!(with_sep, no_sep);
    }
}

// ---------------------------------------------------------------------------
// BigIntTest / BigIntegerTest
// ---------------------------------------------------------------------------

#[test]
fn bigint_test() {
    {
        let (mut sm, id) = mk(
            " 0n 1n 1000n 12_34n 1928371289378129381212398n 0xdeadbeefn 0b10101100101n",
        );
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);

        let expect = |lex: &mut JSLexer, tab: &AtomTable, s: &[u8]| {
            assert_eq!(
                lex.advance(GrammarContext::AllowDiv).kind(),
                TokenKind::bigint_literal
            );
            assert_eq!(tab.bytes(lex.token().get_bigint_literal()), s);
        };
        expect(&mut lex, &tab, b"0");
        expect(&mut lex, &tab, b"1");
        expect(&mut lex, &tab, b"1000");
        expect(&mut lex, &tab, b"1234");
        expect(&mut lex, &tab, b"1928371289378129381212398");
        expect(&mut lex, &tab, b"0xdeadbeef");
        expect(&mut lex, &tab, b"0b10101100101");
    }

    // Malformed bigints each produce exactly one error.
    for src in ["09n", "1.1n", "1e2n"] {
        let (mut sm, id) = mk(src);
        let tab = AtomTable::new();
        let mut diag = Diag::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.advance(GrammarContext::AllowDiv);
        assert_eq!(diag.err_clear(&lex), 1, "src={src:?}");
    }
}

#[test]
fn biginteger_test() {
    let (mut sm, id) = mk(" 0xFFFFFFFFFFFFFFFF 99999999999999999999");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);

    // More than 52 bits of integer.
    let tok = lex.advance(GrammarContext::AllowDiv);
    assert_eq!(tok.kind(), TokenKind::numeric_literal);
    // C++ checks the dtoa string "18446744073709551616". Without dtoa here, we
    // assert the exact f64 the IEEE rounding produces, which is the same value
    // that string denotes.
    assert_eq!(tok.get_numeric_literal(), 18446744073709551616.0_f64);

    let tok = lex.advance(GrammarContext::AllowDiv);
    assert_eq!(tok.kind(), TokenKind::numeric_literal);
    assert_eq!(tok.get_numeric_literal(), 100000000000000000000.0_f64);

    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::eof);
}

// ---------------------------------------------------------------------------
// BadNumbersTest
// ---------------------------------------------------------------------------

#[test]
fn bad_numbers_test() {
    let (mut sm, id) =
        mk("123hhhh; 123e ; .4.5 ; 0_7 1__23 0b_11 123_ 1._2 12e_3");
    let tab = AtomTable::new();
    let mut diag = Diag::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);

    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::numeric_literal);
    assert_eq!(diag.err_clear(&lex), 1);

    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::semi);

    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::numeric_literal);
    assert_eq!(diag.err_clear(&lex), 1);

    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::semi);

    // `.4` then `.5` (two valid numbers).
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::numeric_literal);
    assert_eq!(diag.err_clear(&lex), 0);
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::numeric_literal);
    assert_eq!(diag.err_clear(&lex), 0);

    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::semi);

    lex.set_strict_mode(false);
    // 0_7
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::numeric_literal);
    assert_eq!(diag.err_clear(&lex), 1);
    // 1__23
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::numeric_literal);
    assert_eq!(diag.err_clear(&lex), 2);
    // 0b_11
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::numeric_literal);
    assert_eq!(diag.err_clear(&lex), 1);
    // 123_
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::numeric_literal);
    assert_eq!(diag.err_clear(&lex), 1);
    // 1._2
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::numeric_literal);
    assert_eq!(diag.err_clear(&lex), 1);
    // 12e_3
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::numeric_literal);
    assert_eq!(diag.err_clear(&lex), 1);

    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::eof);
}

// ---------------------------------------------------------------------------
// ZeroRadixTest / OctalLiteralTest / FlowOctalLiteralTest / BinaryLiteralTest
// ---------------------------------------------------------------------------

#[test]
fn zero_radix_test() {
    let (mut sm, id) = mk(" 0x");
    let tab = AtomTable::new();
    let mut diag = Diag::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    // Malformed hex number -> one error, still a numeric_literal.
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::numeric_literal);
    assert_eq!(diag.err_clear(&lex), 1);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
}

#[test]
fn octal_literal_test() {
    {
        let (mut sm, id) = mk("01 010 09 019 0o11 0O11");
        let tab = AtomTable::new();
        let mut diag = Diag::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

        let tok = lex.advance(GrammarContext::AllowRegExp);
        assert_eq!(tok.kind(), TokenKind::numeric_literal);
        assert_eq!(tok.get_numeric_literal(), 1.0);

        let tok = lex.advance(GrammarContext::AllowRegExp);
        assert_eq!(tok.kind(), TokenKind::numeric_literal);
        assert_eq!(tok.get_numeric_literal(), 8.0);

        assert_eq!(diag.warn_clear(&lex), 0);

        let tok = lex.advance(GrammarContext::AllowRegExp);
        assert_eq!(tok.kind(), TokenKind::numeric_literal);
        assert_eq!(tok.get_numeric_literal(), 9.0);
        assert_eq!(diag.warn_clear(&lex), 1);

        let tok = lex.advance(GrammarContext::AllowRegExp);
        assert_eq!(tok.kind(), TokenKind::numeric_literal);
        assert_eq!(tok.get_numeric_literal(), 19.0);
        assert_eq!(diag.warn_clear(&lex), 1);

        let tok = lex.advance(GrammarContext::AllowRegExp);
        assert_eq!(tok.kind(), TokenKind::numeric_literal);
        assert_eq!(tok.get_numeric_literal(), 9.0);

        let tok = lex.advance(GrammarContext::AllowRegExp);
        assert_eq!(tok.kind(), TokenKind::numeric_literal);
        assert_eq!(tok.get_numeric_literal(), 9.0);

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
    }

    {
        let (mut sm, id) = mk("08.1_1 07.11 07.9 08.9");
        let tab = AtomTable::new();
        let mut diag = Diag::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        lex.set_strict_mode(false);

        // 08.1_1 -> 8.11, one warning (NonOctalDecimalIntegerLiteral-ish).
        let tok = lex.advance(GrammarContext::AllowRegExp);
        assert_eq!(tok.kind(), TokenKind::numeric_literal);
        assert_eq!(tok.get_numeric_literal(), 8.11);
        assert_eq!(diag.warn_clear(&lex), 1);

        // 07.11 -> octal-with-fraction, one error.
        let tok = lex.advance(GrammarContext::AllowRegExp);
        assert_eq!(tok.kind(), TokenKind::numeric_literal);
        assert_eq!(diag.err_clear(&lex), 1);

        // 07.9 -> one error.
        let tok = lex.advance(GrammarContext::AllowRegExp);
        assert_eq!(tok.kind(), TokenKind::numeric_literal);
        assert_eq!(diag.err_clear(&lex), 1);

        // 08.9 -> 8.9, one warning.
        let tok = lex.advance(GrammarContext::AllowRegExp);
        assert_eq!(tok.kind(), TokenKind::numeric_literal);
        assert_eq!(tok.get_numeric_literal(), 8.9);
        assert_eq!(diag.warn_clear(&lex), 1);

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
    }

    {
        let (mut sm, id) = mk("08.1_1 07.11 07.9 08.9");
        let tab = AtomTable::new();
        let mut diag = Diag::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        lex.set_strict_mode(true);

        for _ in 0..4 {
            let tok = lex.advance(GrammarContext::AllowRegExp);
            assert_eq!(tok.kind(), TokenKind::numeric_literal);
            assert_eq!(diag.err_clear(&lex), 1);
        }

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
    }
}

#[test]
fn flow_octal_literal_test() {
    // HERMES_PARSE_FLOW: an octal-looking literal in the Flow Type context is an
    // error but still lexes as a numeric_literal with value 1.0.
    let (mut sm, id) = mk("01");
    let tab = AtomTable::new();
    let mut diag = Diag::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::Type);
    let tok = lex.advance(GrammarContext::Type);
    assert_eq!(tok.kind(), TokenKind::numeric_literal);
    assert_eq!(tok.get_numeric_literal(), 1.0);
    assert_eq!(diag.err_clear(&lex), 1);
}

#[test]
fn binary_literal_test() {
    let (mut sm, id) = mk("0b1 0B1 0b101");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    let tok = lex.advance(GrammarContext::AllowRegExp);
    assert_eq!(tok.kind(), TokenKind::numeric_literal);
    assert_eq!(tok.get_numeric_literal(), 1.0);

    let tok = lex.advance(GrammarContext::AllowRegExp);
    assert_eq!(tok.kind(), TokenKind::numeric_literal);
    assert_eq!(tok.get_numeric_literal(), 1.0);

    let tok = lex.advance(GrammarContext::AllowRegExp);
    assert_eq!(tok.kind(), TokenKind::numeric_literal);
    assert_eq!(tok.get_numeric_literal(), 5.0);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
}

// ---------------------------------------------------------------------------
// SimpleIdentifierTest / IdentifierTest / PrivateIdentifierTest
// ---------------------------------------------------------------------------

#[test]
fn simple_identifier_test() {
    let (mut sm, id) = mk("true foo bar foo");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::rw_true);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
    assert_eq!(tab.bytes(lex.token().get_identifier()), b"foo");
    let foo = lex.token().get_identifier();

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
    assert_eq!(tab.bytes(lex.token().get_identifier()), b"bar");

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
    assert_eq!(tab.bytes(lex.token().get_identifier()), b"foo");
    // Identical interned handle (the C++ `UniqueString *` pointer equality).
    assert_eq!(foo, lex.token().get_identifier());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
}

#[test]
fn identifier_test() {
    let (mut sm, id) = mk(" _foo$123 $123 a\\u0061 \\u0061\\u0061 \\u0061a");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
    assert_eq!(tab.bytes(lex.token().get_identifier()), b"_foo$123");
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
    assert_eq!(tab.bytes(lex.token().get_identifier()), b"$123");

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
    assert_eq!(tab.bytes(lex.token().get_identifier()), b"aa");
    let aa = lex.token().get_identifier();
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
    assert_eq!(tab.bytes(lex.token().get_identifier()), b"aa");
    assert_eq!(aa, lex.token().get_identifier());
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
    assert_eq!(tab.bytes(lex.token().get_identifier()), b"aa");
    assert_eq!(aa, lex.token().get_identifier());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
}

#[test]
fn private_identifier_test() {
    let (mut sm, id) = mk(" #foo # foo #64");
    let tab = AtomTable::new();
    let mut diag = Diag::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::private_identifier);
    assert_eq!(tab.bytes(lex.token().get_private_identifier()), b"foo");

    // `# foo` -> error (space after #), then `foo` lexes as a normal identifier.
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
    assert_eq!(diag.err_clear(&lex), 1);
    assert_eq!(tab.bytes(lex.token().get_identifier()), b"foo");

    // `#64` -> error (digit after #), then `64` lexes as a number.
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::numeric_literal);
    assert_eq!(diag.err_clear(&lex), 1);
    assert_eq!(lex.token().get_numeric_literal(), 64.0);
}

// ---------------------------------------------------------------------------
// StringTest1 / StringLineParaSepTest / StringTest2 / StringOctalTest
// ---------------------------------------------------------------------------

#[test]
fn string_test1() {
    let (mut sm, id) = mk_bytes(b"'aa' \"bb\" 'open1\n\"open2");
    let tab = AtomTable::new();
    let mut diag = Diag::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(diag.err_clear(&lex), 0);
    assert_eq!(tab.bytes(lex.token().get_string_literal()), b"aa");

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(diag.err_clear(&lex), 0);
    assert_eq!(tab.bytes(lex.token().get_string_literal()), b"bb");

    // Unterminated 'open1 -> one error; value is "open1". No newline before it.
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(diag.err_clear(&lex), 1);
    assert_eq!(tab.bytes(lex.token().get_string_literal()), b"open1");
    assert!(!lex.is_new_line_before_current_token());

    // "open2 -> one error; value is "open2". Preceded by a newline.
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(diag.err_clear(&lex), 1);
    assert_eq!(tab.bytes(lex.token().get_string_literal()), b"open2");
    assert!(lex.is_new_line_before_current_token());

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
    assert!(!lex.is_new_line_before_current_token());
}

#[test]
fn string_line_para_sep_test() {
    // Unicode line/paragraph separators are valid inside a string (since ES10).
    let (mut sm, id) = mk_bytes(
        b"'\xe2\x80\xa8' '\xe2\x80\xa9' '\\\xe2\x80\xa8' '\\\xe2\x80\xa9' ",
    );
    let tab = AtomTable::new();
    let mut diag = Diag::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(diag.err_clear(&lex), 0);
    assert_eq!(tab.bytes(lex.token().get_string_literal()), b"\xe2\x80\xa8");

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(diag.err_clear(&lex), 0);
    assert_eq!(tab.bytes(lex.token().get_string_literal()), b"\xe2\x80\xa9");

    // A backslash + separator is a line continuation -> empty cooked value.
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(diag.err_clear(&lex), 0);
    assert_eq!(tab.bytes(lex.token().get_string_literal()), b"");

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(diag.err_clear(&lex), 0);
    assert_eq!(tab.bytes(lex.token().get_string_literal()), b"");

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
}

#[test]
fn string_test2() {
    // The C++ source is `'a\\u0061\x62\143' ...`, where in the C string the
    // `\x62` and `\143` are C escapes that become the literal bytes 'b' and 'c'
    // (octal 0o143 == 99 == 'c'). So the JS source the lexer actually sees is
    // `'aabc'` (no JS octal escape). We reproduce the resulting bytes.
    let (mut sm, id) = mk_bytes(
        b"'a\\u0061\x62\x63' '\\w\\'\\\"\\b\\f\\n\\r\\t\\v\\\na' '\\x1g' '\\u123g'",
    );
    let tab = AtomTable::new();
    let mut diag = Diag::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(diag.err_clear(&lex), 0);
    assert_eq!(tab.bytes(lex.token().get_string_literal()), b"aabc");

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(diag.err_clear(&lex), 0);
    assert_eq!(
        tab.bytes(lex.token().get_string_literal()),
        b"w'\"\x08\x0c\n\r\t\x0ba"
    );

    // '\x1g' -> bad hex escape -> one error.
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(diag.err_clear(&lex), 1);

    // '\u123g' -> bad unicode escape -> one error.
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(diag.err_clear(&lex), 1);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
}

#[test]
fn string_octal_test() {
    {
        // non-strict mode (ctor strictMode=false).
        let (mut sm, id) = mk("'\\0' '\\000' '\\05'");
        let tab = AtomTable::new();
        let mut diag = Diag::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        lex.set_strict_mode(false);

        for _ in 0..3 {
            assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
            assert_eq!(diag.err_clear(&lex), 0);
        }
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
    }

    {
        // strict mode (ctor strictMode=true, the Rust default).
        let (mut sm, id) = mk("'\\0' '\\000' '\\05'");
        let tab = AtomTable::new();
        let mut diag = Diag::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        lex.set_strict_mode(true);

        // '\0' alone is allowed even in strict mode.
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
        assert_eq!(diag.err_clear(&lex), 0);

        // '\000' and '\05' are octal escapes -> one error each in strict mode.
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
        assert_eq!(diag.err_clear(&lex), 1);

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
        assert_eq!(diag.err_clear(&lex), 1);

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
    }
}

// ---------------------------------------------------------------------------
// UnicodeEscapeTest
// ---------------------------------------------------------------------------

#[test]
fn unicode_escape_test() {
    {
        let (mut sm, id) = mk("'\\u0f3b' '\\u{0f3b}' '\\u{0062}'");
        let tab = AtomTable::new();
        let mut diag = Diag::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
        assert_eq!(diag.err_clear(&lex), 0);
        assert_eq!(tab.bytes(lex.token().get_string_literal()), b"\xe0\xbc\xbb");

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
        assert_eq!(diag.err_clear(&lex), 0);
        assert_eq!(tab.bytes(lex.token().get_string_literal()), b"\xe0\xbc\xbb");

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
        assert_eq!(diag.err_clear(&lex), 0);
        assert_eq!(tab.bytes(lex.token().get_string_literal()), b"\x62");

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
        assert_eq!(diag.err_clear(&lex), 0);
    }

    {
        // Code point out of range -> one error.
        let (mut sm, id) = mk("'\\u{ffffffff}'");
        let tab = AtomTable::new();
        let mut diag = Diag::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
        assert_eq!(diag.err_clear(&lex), 1);
    }

    {
        // Empty braced code point -> one error.
        let (mut sm, id) = mk("'\\u{}'");
        let tab = AtomTable::new();
        let mut diag = Diag::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
        assert_eq!(diag.err_clear(&lex), 1);
    }
}

// ---------------------------------------------------------------------------
// RegexpSmoke / RegexpSmoke2 / classInRegexp
// ---------------------------------------------------------------------------

#[test]
fn regexp_smoke() {
    let (mut sm, id) = mk("; /aa/bc");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::regexp_literal);
    let re = lex.token().get_regexp_literal();
    assert_eq!(tab.bytes(re.body()), b"aa");
    assert_eq!(tab.bytes(re.flags()), b"bc");

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
}

#[test]
fn regexp_smoke2() {
    let (mut sm, id) = mk("; /(\\w+)\\s(\\w+)/g");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::regexp_literal);
    let re = lex.token().get_regexp_literal();
    assert_eq!(tab.bytes(re.body()), b"(\\w+)\\s(\\w+)");
    assert_eq!(tab.bytes(re.flags()), b"g");

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
}

#[test]
fn class_in_regexp() {
    // `/` can be used in a regexp class.
    let (mut sm, id) = mk("/[a/]/");
    let tab = AtomTable::new();
    let mut diag = Diag::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::regexp_literal);
    assert_eq!(tab.bytes(lex.token().get_regexp_literal().body()), b"[a/]");

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
    assert_eq!(diag.err_clear(&lex), 0);
}

// ---------------------------------------------------------------------------
// UTF16BadSurrogatePairs / normalizeUTF8
// ---------------------------------------------------------------------------

#[test]
fn utf16_bad_surrogate_pairs() {
    // We *allow* invalid UTF-16 surrogate pairs; they round-trip as WTF-8.
    let (mut sm, id) = mk("' \\udc01 \\ud805 '");
    let tab = AtomTable::new();
    let mut diag = Diag::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(
        tab.bytes(lex.token().get_string_literal()),
        b"\x20\xed\xb0\x81\x20\xed\xa0\x85\x20"
    );

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
    assert_eq!(diag.err_clear(&lex), 0);
}

#[test]
fn normalize_utf8() {
    // A UTF-8 codepoint > 0xFFFF is normalized to a WTF-8 surrogate pair.
    let (mut sm, id) = mk_bytes(b"'\xf0\x90\x80\x81'");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(
        tab.bytes(lex.token().get_string_literal()),
        b"\xed\xa0\x80\xed\xb0\x81"
    );

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
}

// ---------------------------------------------------------------------------
// templateLiterals
// ---------------------------------------------------------------------------

#[test]
fn template_literals() {
    {
        let (mut sm, id) = mk_bytes(b"`abc` `\\x41` `\\u0041` `\\\xe2\x80\xa8`");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::no_substitution_template);
        assert_eq!(tab.bytes(lex.token().get_template_value().unwrap()), b"abc");
        assert_eq!(tab.bytes(lex.token().get_template_raw_value()), b"abc");

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::no_substitution_template);
        assert_eq!(tab.bytes(lex.token().get_template_value().unwrap()), b"\x41");
        assert_eq!(tab.bytes(lex.token().get_template_raw_value()), b"\\x41");

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::no_substitution_template);
        assert_eq!(tab.bytes(lex.token().get_template_value().unwrap()), b"\x41");
        assert_eq!(tab.bytes(lex.token().get_template_raw_value()), b"\\u0041");

        // `\<LS>` is a line continuation -> empty cooked value, raw keeps it.
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::no_substitution_template);
        assert_eq!(tab.bytes(lex.token().get_template_value().unwrap()), b"");
        assert_eq!(tab.bytes(lex.token().get_template_raw_value()), b"\\\xe2\x80\xa8");
    }

    {
        let (mut sm, id) = mk("`abc${x}def${y}ghi`");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::template_head);
        assert_eq!(tab.bytes(lex.token().get_template_value().unwrap()), b"abc");
        assert_eq!(tab.bytes(lex.token().get_template_raw_value()), b"abc");

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
        assert_eq!(tab.bytes(lex.token().get_identifier()), b"x");

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::r_brace);
        lex.rescan_rbrace_in_template_literal();
        assert_eq!(lex.token().kind(), TokenKind::template_middle);
        assert_eq!(tab.bytes(lex.token().get_template_value().unwrap()), b"def");
        assert_eq!(tab.bytes(lex.token().get_template_raw_value()), b"def");

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
        assert_eq!(tab.bytes(lex.token().get_identifier()), b"y");

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::r_brace);
        lex.rescan_rbrace_in_template_literal();
        assert_eq!(lex.token().kind(), TokenKind::template_tail);
        assert_eq!(tab.bytes(lex.token().get_template_value().unwrap()), b"ghi");
        assert_eq!(tab.bytes(lex.token().get_template_raw_value()), b"ghi");
    }

    {
        // `\0` -> cooked NUL, raw "\0".
        let (mut sm, id) = mk("`\\0`");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::no_substitution_template);
        assert_eq!(tab.bytes(lex.token().get_template_value().unwrap()), b"\0");
        assert_eq!(tab.bytes(lex.token().get_template_raw_value()), b"\\0");
    }

    {
        // CR / CRLF normalization to LF in both cooked and raw.
        let (mut sm, id) = mk_bytes(b"`\r\n \n \r`");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::no_substitution_template);
        assert_eq!(tab.bytes(lex.token().get_template_value().unwrap()), b"\n \n \n");
        assert_eq!(tab.bytes(lex.token().get_template_raw_value()), b"\n \n \n");
    }
}

// ---------------------------------------------------------------------------
// reservedTokens
// ---------------------------------------------------------------------------

#[test]
fn reserved_tokens() {
    let src = "implements private public interface package protected static yield";

    // Strict mode: recognized as reserved words.
    {
        let (mut sm, id) = mk(src);
        let tab = AtomTable::new();
        let mut diag = Diag::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        lex.set_strict_mode(true);
        for k in [
            TokenKind::rw_implements,
            TokenKind::rw_private,
            TokenKind::rw_public,
            TokenKind::rw_interface,
            TokenKind::rw_package,
            TokenKind::rw_protected,
            TokenKind::rw_static,
            TokenKind::rw_yield,
        ] {
            assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), k);
        }
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
        assert_eq!(diag.err_clear(&lex), 0);
    }

    // Non-strict mode: NOT recognized — all plain identifiers.
    {
        let (mut sm, id) = mk(src);
        let tab = AtomTable::new();
        let mut diag = Diag::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        lex.set_strict_mode(false);
        for _ in 0..8 {
            assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
        }
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
        assert_eq!(diag.err_clear(&lex), 0);
    }
}

// ---------------------------------------------------------------------------
// SourceMappingUrl
// ---------------------------------------------------------------------------

#[test]
fn source_mapping_url() {
    // End of file.
    {
        let (mut sm, id) = mk(
            "var x = 1;//# sourceMappingURL=localhost:8000/this_is_the_url.map",
        );
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::rw_var);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::equal);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::numeric_literal);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);
        lex.advance(GrammarContext::AllowRegExp);
        assert_eq!(
            lex.get_source_mapping_url(),
            Some("localhost:8000/this_is_the_url.map")
        );
    }

    // Middle of file.
    {
        let (mut sm, id) = mk(
            "var x = 1;\n//# sourceMappingURL=second-map.map\nvar y = 2;",
        );
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::rw_var);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::equal);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::numeric_literal);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);
        lex.advance(GrammarContext::AllowRegExp);
        assert_eq!(lex.get_source_mapping_url(), Some("second-map.map"));
    }

    // Invalid source map comments are ignored.
    {
        let (mut sm, id) = mk(
            "var x = 1;\n\
             // sourceMappingURL=localhost:8000/this_is_the_url.map\n\
             //# sourceMappingURL =localhost:8000/this_is_the_url.map\n\
             //#sourceMappingURL=localhost:8000/this_is_the_url.map\n\
             //# sourceMappingURL=\nlocalhost:8000/this_is_the_url.map\n",
        );
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::rw_var);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::equal);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::numeric_literal);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);
        lex.advance(GrammarContext::AllowRegExp);
        lex.advance(GrammarContext::AllowRegExp);
        lex.advance(GrammarContext::AllowRegExp);
        lex.advance(GrammarContext::AllowRegExp);
        // The C++ checks `sm.getSourceMappingUrl(3).empty()`: of the four lines,
        // the only one with valid `//# sourceMappingURL=` syntax has an EMPTY
        // value (`//# sourceMappingURL=\n`), so the stored URL is the empty
        // string. The three malformed variants are ignored. So the resulting
        // URL must be empty (either unset or "").
        let url = lex.get_source_mapping_url();
        assert!(url.is_none() || url == Some(""), "url={url:?}");
        let mgr_url = lex.get_source_mgr().source_mapping_url(id);
        assert!(
            mgr_url.is_none() || mgr_url == Some(""),
            "mgr_url={mgr_url:?}"
        );
    }

    // A later URL overwrites the first.
    {
        let (mut sm, id) = mk(
            "var x = 1;\n//# sourceMappingURL=url1\n//# sourceMappingURL=url2\n",
        );
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::rw_var);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::equal);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::numeric_literal);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);
        lex.advance(GrammarContext::AllowRegExp);
        lex.advance(GrammarContext::AllowRegExp);
        assert_eq!(lex.get_source_mapping_url(), Some("url2"));
    }
}

// ---------------------------------------------------------------------------
// LookaheadNewlineTest / LookaheadTest
// ---------------------------------------------------------------------------

#[test]
fn lookahead_newline_test() {
    // C++ lookahead1(llvh::None) uses RequireNoNewLine=true (parser default).
    let (mut sm, id) = mk("function\n(");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::rw_function);

    // No expected token; newline before next token -> None, state reverted.
    let opt_next = lex.lookahead1::<true>(None);
    assert_eq!(opt_next, None);

    assert_eq!(lex.token().kind(), TokenKind::rw_function);
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::l_paren);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
}

#[test]
fn lookahead_test() {
    let (mut sm, id) = mk("function( foo,");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::rw_function);

    // Without an expected token, always revert.
    let opt_next = lex.lookahead1::<true>(None);
    assert_eq!(opt_next, Some(TokenKind::l_paren));

    assert_eq!(lex.token().kind(), TokenKind::rw_function);
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::l_paren);

    // With an expected token, revert iff it doesn't match.
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);

    // Expect plus, see comma -> revert to identifier.
    let opt_next = lex.lookahead1::<true>(Some(TokenKind::plus));
    assert_eq!(opt_next, Some(TokenKind::comma));
    assert_eq!(lex.token().kind(), TokenKind::identifier);

    // Expect comma, see comma -> keep the lookahead token.
    let opt_next = lex.lookahead1::<true>(Some(TokenKind::comma));
    assert_eq!(opt_next, Some(TokenKind::comma));
    assert_eq!(lex.token().kind(), TokenKind::comma);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
}

// ---------------------------------------------------------------------------
// RegressConsumeBadHexTest / ConsumeBadBracedCodePoint
// ---------------------------------------------------------------------------

#[test]
fn regress_consume_bad_hex_test() {
    // Hex escape where the two following chars are ('5' & ~32) == 0x15. Catches
    // a bug where 32 was or-ed before checking digit-ness.
    let (mut sm, id) = mk_bytes(b"'\\x\x15\x15'");
    let tab = AtomTable::new();
    let mut diag = Diag::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(diag.err_clear(&lex), 1);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
    assert!(!lex.is_new_line_before_current_token());

    assert_eq!(diag.err_clear(&lex), 0);
}

#[test]
fn consume_bad_braced_code_point() {
    // Invalid braced escape with no terminating brace. Catches an OOB read where
    // the error limit is reached, curCharPtr_ is at eof, but consumeBracedCodePoint
    // kept operating. Configure a low error limit to reach it fast.
    let (mut sm, id) = mk("'\\u{12XXXXXXXXXXX'");
    sm.set_error_limit(1);
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::string_literal);
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
}

// ---------------------------------------------------------------------------
// AtSignTest
// ---------------------------------------------------------------------------

#[test]
fn at_sign_test() {
    for limit in [10u32, 1u32] {
        let (mut sm, id) = mk("`${{}@");
        sm.set_error_limit(limit);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::template_head);

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::l_brace);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::r_brace);

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::at);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);
        assert_eq!(lex.get_source_mgr().error_count(), 0, "limit={limit}");
    }
}

// ---------------------------------------------------------------------------
// JSXTest
// ---------------------------------------------------------------------------

#[test]
fn jsx_test() {
    let (mut sm, id) = mk("abc def{xyz<qwerty");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance_in_jsx_child().kind(), TokenKind::jsx_text);
    assert_eq!(tab.bytes(lex.token().get_jsx_text_raw()), b"abc def");

    assert_eq!(lex.advance_in_jsx_child().kind(), TokenKind::l_brace);
    assert_eq!(lex.advance_in_jsx_child().kind(), TokenKind::jsx_text);
    assert_eq!(tab.bytes(lex.token().get_jsx_text_raw()), b"xyz");

    assert_eq!(lex.advance_in_jsx_child().kind(), TokenKind::less);
    assert_eq!(lex.advance_in_jsx_child().kind(), TokenKind::jsx_text);
    assert_eq!(tab.bytes(lex.token().get_jsx_text_raw()), b"qwerty");
}

// ---------------------------------------------------------------------------
// StoreCommentsTest
// ---------------------------------------------------------------------------

#[test]
fn store_comments_test() {
    use hermes_parser::token::CommentKind;

    // Helper: stored-comment (kind, stripped-string) pairs given the source.
    // The stripped string is computed via StoredComment::get_string over the
    // buffer bytes (the C++ getString strips delimiters).
    {
        let (mut sm, id) = mk("// hello\n;\n// world");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        lex.set_store_comments(true);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);

        let buf = lex.get_source_mgr().source_buffer(id);
        let raw = buf.raw();
        let cs = lex.get_stored_comments();
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0].kind(), CommentKind::Line);
        assert_eq!(cs[0].get_string(raw), b" hello");
        assert_eq!(cs[1].kind(), CommentKind::Line);
        assert_eq!(cs[1].get_string(raw), b" world");
    }

    {
        let (mut sm, id) = mk("/* hello */;/*world*/");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        lex.set_store_comments(true);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);

        let buf = lex.get_source_mgr().source_buffer(id);
        let raw = buf.raw();
        let cs = lex.get_stored_comments();
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0].kind(), CommentKind::Block);
        assert_eq!(cs[0].get_string(raw), b" hello ");
        assert_eq!(cs[1].kind(), CommentKind::Block);
        assert_eq!(cs[1].get_string(raw), b"world");
    }

    {
        let (mut sm, id) = mk("#! hello world\n;");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        lex.set_store_comments(true);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);

        let buf = lex.get_source_mgr().source_buffer(id);
        let raw = buf.raw();
        let cs = lex.get_stored_comments();
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].kind(), CommentKind::Hashbang);
        assert_eq!(cs[0].get_string(raw), b" hello world");
    }

    {
        let (mut sm, id) = mk("/**/;//\n");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        lex.set_store_comments(true);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::semi);
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);

        let buf = lex.get_source_mgr().source_buffer(id);
        let raw = buf.raw();
        let cs = lex.get_stored_comments();
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0].kind(), CommentKind::Block);
        assert_eq!(cs[0].get_string(raw), b"");
        assert_eq!(cs[1].kind(), CommentKind::Line);
        assert_eq!(cs[1].get_string(raw), b"");
    }

    // SavePoint between two comments: restoring removes the comment after it.
    {
        let (mut sm, id) = mk("/*one*/ < /*two*/ >");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        lex.set_store_comments(true);

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::less);
        let save_point = lex.save_point();
        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::greater);

        {
            let buf = lex.get_source_mgr().source_buffer(id);
            let raw = buf.raw();
            let cs = lex.get_stored_comments();
            assert_eq!(cs.len(), 2);
            assert_eq!(cs[0].kind(), CommentKind::Block);
            assert_eq!(cs[0].get_string(raw), b"one");
            assert_eq!(cs[1].kind(), CommentKind::Block);
            assert_eq!(cs[1].get_string(raw), b"two");
        }

        save_point.restore(&mut lex);

        let buf = lex.get_source_mgr().source_buffer(id);
        let raw = buf.raw();
        let cs = lex.get_stored_comments();
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].kind(), CommentKind::Block);
        assert_eq!(cs[0].get_string(raw), b"one");
    }

    // Lookahead does NOT store comments.
    {
        let (mut sm, id) = mk("/*one*/ A /*two*/ >");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        lex.set_store_comments(true);

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
        {
            let buf = lex.get_source_mgr().source_buffer(id);
            let raw = buf.raw();
            let cs = lex.get_stored_comments();
            assert_eq!(cs.len(), 1);
            assert_eq!(cs[0].get_string(raw), b"one");
        }

        // Lookahead should not add the `/*two*/` comment.
        lex.lookahead1::<true>(Some(TokenKind::semi));
        {
            let buf = lex.get_source_mgr().source_buffer(id);
            let raw = buf.raw();
            let cs = lex.get_stored_comments();
            assert_eq!(cs.len(), 1);
            assert_eq!(cs[0].get_string(raw), b"one");
        }

        assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::greater);
        assert_eq!(lex.get_stored_comments().len(), 2);
    }
}

// ---------------------------------------------------------------------------
// PrevTokenEndLocTest
// ---------------------------------------------------------------------------

#[test]
fn prev_token_end_loc_test() {
    // DEVIATION: the C++ compares raw `const char *` end pointers; the Rust lexer
    // is offset-based, so we compare `SMLoc` (buffer + offset). The behavior
    // verified — which token's end the prev-end tracks — is identical.
    let (mut sm, id) = mk("var x = 1");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::rw_var);
    let var_end_loc = lex.token().end_loc();

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::identifier);
    assert_eq!(lex.prev_token_end(), var_end_loc);
    let id_end_loc = lex.token().end_loc();

    // Create a save point at the identifier.
    let save_point = lex.save_point();

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::equal);
    assert_eq!(lex.prev_token_end(), id_end_loc);
    let equal_end_loc = lex.token().end_loc();

    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::numeric_literal);
    assert_eq!(lex.prev_token_end(), equal_end_loc);

    // Restoring to identifier: the last token is now the `var` keyword.
    save_point.restore(&mut lex);
    assert_eq!(lex.prev_token_end(), var_end_loc);
}

// ---------------------------------------------------------------------------
// A message-text check (mirrors a representative C++ `diag.getMessageCount` +
// message-string assertion). The C++ DiagContext counts; where it inspects a
// message string we capture via CollectingHandler. CommentTest's "non-terminated
// block comment" is a good representative.
// ---------------------------------------------------------------------------

#[test]
fn comment_error_message_text() {
    let (mut sm, id) = mk("/* not closed");
    sm.set_handler(Box::new(CollectingHandler::new()));
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
    assert_eq!(lex.advance(GrammarContext::AllowRegExp).kind(), TokenKind::eof);

    let h = lex
        .get_source_mgr()
        .handler_as::<CollectingHandler>()
        .unwrap();
    // One error ("non-terminated block comment") plus its note
    // ("comment started here").
    let errs: Vec<_> = h
        .messages()
        .iter()
        .filter(|m| m.kind == DiagKind::Error)
        .collect();
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].message, "non-terminated block comment");
    let notes: Vec<_> = h
        .messages()
        .iter()
        .filter(|m| m.kind == DiagKind::Note)
        .collect();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].message, "comment started here");
}
