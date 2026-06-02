//! JSLexer, ported from lib/Parser/JSLexer.cpp.
//!
//! Phase 1a implements the lexer skeleton and `advance()` for punctuators,
//! whitespace, line/block comments, and EOF. Identifiers, numbers, strings,
//! templates, regexp, private identifiers and the `\` identifier-escape arm are
//! stubbed: they report a "not yet implemented (phase 1b)" error and force EOF,
//! so the lexer is well-defined for those inputs but they are not part of the
//! phase-1a corpus.

use std::rc::Rc;

use atom_table::AtomTable;
use support::buffer::SourceBuffer;
use support::diag::Subsystem;
use support::location::{SMLoc, SMRange, SourceId};
use support::manager::SourceErrorManager;

use unicode::{
    is_unicode_id_continue, is_unicode_id_start, is_unicode_only_id_start,
    is_unicode_only_space, UNICODE_MAX_VALUE, UNICODE_REPLACEMENT_CHARACTER,
};

use crate::cursor::Cursor;
use crate::token::{StoredComment, StoredToken, Token};
use crate::token_kinds::{
    match_reserved_word, variant_name, TokenKind,
};
use crate::utf8::{
    append_unicode_to_storage, decode_utf8, is_utf8_start,
    match_unicode_line_terminator_offset1, UTF8_LINE_TERMINATOR_CHAR0,
};

/// The grammar context affecting how some tokens are lexed (e.g. `/` as a
/// division operator vs. a regular-expression literal). Port of
/// `JSLexer::GrammarContext`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GrammarContext {
    AllowRegExp,
    AllowDiv,
    AllowJSXIdentifier,
    Type,
}

/// The Hermes JavaScript lexer. Port of `hermes::parser::JSLexer`.
///
/// The lexer borrows the `SourceErrorManager` (for diagnostics) and the
/// `AtomTable` interner, and owns a `Cursor` over a clone of the source buffer.
pub struct JSLexer<'a> {
    sm: &'a mut SourceErrorManager,
    /// ID of the buffer in the SourceErrorManager.
    buf_id: SourceId,
    /// The scan cursor over the (NUL-terminated) source buffer.
    cursor: Cursor,
    /// Interner shared with the rest of the front end.
    #[allow(dead_code)]
    strtab: &'a AtomTable,

    /// The current token.
    token: Token,
    /// The end location of the previous token (port of `prevTokenEndLoc_`).
    prev_token_end: SMLoc,
    /// True if there was a line terminator before the current token.
    new_line_before_current_token: bool,

    /// Whether the lexer is in strict mode (affects reserved-word recognition;
    /// used by the identifier scanner in phase 1b).
    #[allow(dead_code)]
    strict_mode: bool,
    /// Whether to convert surrogate pairs while decoding (phase 1b+).
    #[allow(dead_code)]
    convert_surrogates: bool,

    /// Scratch storage for assembling identifier/string values (phase 1b+).
    #[allow(dead_code)]
    tmp_storage: Vec<u8>,

    /// `//# sourceURL=` value, if seen. NOTE: magic-comment URL parsing is
    /// deferred to a later phase; this field carries the faithful shape.
    #[allow(dead_code)]
    source_url: Option<String>,
    /// `//# sourceMappingURL=` value, if seen. See `source_url`.
    #[allow(dead_code)]
    source_mapping_url: Option<String>,

    /// Whether to store comments encountered while lexing. The flag is wired
    /// but comment STORAGE itself is deferred to a later phase, so it is not yet
    /// read.
    #[allow(dead_code)]
    store_comments: bool,
    /// Stored comments (only populated when `store_comments`). NOTE: comment
    /// STORAGE is deferred — the flag is wired but the storage stays empty in 1a.
    #[allow(dead_code)]
    comment_storage: Vec<StoredComment>,
    /// Whether to store every token encountered while lexing.
    store_tokens: bool,
    /// Stored tokens (only populated when `store_tokens`).
    #[allow(dead_code)]
    token_storage: Vec<StoredToken>,
}

impl<'a> JSLexer<'a> {
    /// Construct a lexer over the buffer identified by `buf_id` in `sm`.
    /// Port of `JSLexer::JSLexer` + `initializeWithBufferId`. The reserved-word
    /// pre-interning (`initializeReservedIdentifiers`) is deferred to phase 1b
    /// along with identifier scanning.
    pub fn new(
        buf_id: SourceId,
        sm: &'a mut SourceErrorManager,
        strtab: &'a AtomTable,
        _grammar_context: GrammarContext,
    ) -> JSLexer<'a> {
        let buffer: Rc<SourceBuffer> = sm.source_buffer(buf_id);
        let cursor = Cursor::new(buffer);
        let start = SMLoc {
            source: buf_id,
            offset: 0,
        };
        JSLexer {
            sm,
            buf_id,
            cursor,
            strtab,
            token: Token::new(buf_id),
            prev_token_end: start,
            new_line_before_current_token: false,
            strict_mode: true,
            convert_surrogates: false,
            tmp_storage: Vec::new(),
            source_url: None,
            source_mapping_url: None,
            store_comments: false,
            comment_storage: Vec::new(),
            store_tokens: false,
            token_storage: Vec::new(),
        }
    }

    /// \return the current token.
    pub fn token(&self) -> &Token {
        &self.token
    }

    /// \return whether a line terminator preceded the current token.
    pub fn is_new_line_before_current_token(&self) -> bool {
        self.new_line_before_current_token
    }

    /// \return the end location of the previous token.
    pub fn prev_token_end(&self) -> SMLoc {
        self.prev_token_end
    }

    /// A location at the current cursor offset.
    #[inline]
    fn cur_loc(&self) -> SMLoc {
        SMLoc {
            source: self.buf_id,
            offset: self.cursor.offset(),
        }
    }

    /// Record the current cursor offset as the start of the current token.
    #[inline]
    fn set_token_start(&mut self) {
        let loc = self.cur_loc();
        self.token.set_start(loc);
    }

    /// Move the cursor to EOF. Port of `forceEOF`.
    #[inline]
    fn force_eof(&mut self) {
        self.cursor.seek_end();
    }

    /// Emit an error at `loc` (Lexer subsystem). If the error limit was reached,
    /// force EOF and return false; otherwise return true. Port of
    /// `JSLexer::error(SMLoc, Twine)` (JSLexer.cpp:2497-2503).
    fn error(&mut self, loc: SMLoc, msg: impl Into<String>) -> bool {
        self.sm.error_at(loc, None, msg.into(), Subsystem::Lexer);
        if !self.sm.is_error_limit_reached() {
            return true;
        }
        self.force_eof();
        false
    }

    /// Emit an error over `range` (Lexer subsystem). Port of
    /// `JSLexer::error(SMRange, Twine)` (JSLexer.cpp:2505-2511).
    #[allow(dead_code)]
    fn error_range(&mut self, range: SMRange, msg: impl Into<String>) -> bool {
        self.sm
            .error_at(range.start, Some(range), msg.into(), Subsystem::Lexer);
        if !self.sm.is_error_limit_reached() {
            return true;
        }
        self.force_eof();
        false
    }

    /// Finish a new token, setting the new token's end location and saving the
    /// previous token's end location. Port of `finishToken` (JSLexer.h:1077).
    #[inline]
    fn finish_token(&mut self) {
        self.prev_token_end = self.token.end_loc();
        let end = self.cur_loc();
        self.token.set_end(end);
        if self.store_tokens {
            self.store_current_token();
        }
    }

    fn store_current_token(&mut self) {
        self.token_storage.push(StoredToken::new(
            self.token.kind(),
            self.token.source_range(),
        ));
    }

    // ---- Punctuator helpers (port of the PUNC_* macros) ---------------------

    /// `PUNC_L1_1`: single-char punctuator.
    #[inline]
    fn punc_l1_1(&mut self, tok: TokenKind) {
        self.set_token_start();
        self.token.set_punctuator(tok);
        self.cursor.advance(1);
    }

    /// `PUNC_L2_2`: `ch1` -> `tok1`, `ch1 ch2` -> `tok2`.
    #[inline]
    fn punc_l2_2(&mut self, ch2: u8, tok1: TokenKind, tok2: TokenKind) {
        self.set_token_start();
        if self.cursor.peek_at(1) == ch2 {
            self.token.set_punctuator(tok2);
            self.cursor.advance(2);
        } else {
            self.token.set_punctuator(tok1);
            self.cursor.advance(1);
        }
    }

    /// `PUNC_L2_3`: `ch1`->`tok1`, `ch1 ch2a`->`tok2a`, `ch1 ch2b`->`tok2b`.
    #[inline]
    fn punc_l2_3(
        &mut self,
        ch2a: u8,
        tok2a: TokenKind,
        ch2b: u8,
        tok2b: TokenKind,
        tok1: TokenKind,
    ) {
        self.set_token_start();
        let c1 = self.cursor.peek_at(1);
        if c1 == ch2a {
            self.token.set_punctuator(tok2a);
            self.cursor.advance(2);
        } else if c1 == ch2b {
            self.token.set_punctuator(tok2b);
            self.cursor.advance(2);
        } else {
            self.token.set_punctuator(tok1);
            self.cursor.advance(1);
        }
    }

    /// `PUNC_L3_3`: `ch1`->`tok1`, `ch1 ch2`->`tok2`, `ch1 ch2 ch3`->`tok3`.
    #[inline]
    fn punc_l3_3(
        &mut self,
        ch2: u8,
        tok2: TokenKind,
        ch3: u8,
        tok3: TokenKind,
        tok1: TokenKind,
    ) {
        self.set_token_start();
        if self.cursor.peek_at(1) != ch2 {
            self.token.set_punctuator(tok1);
            self.cursor.advance(1);
        } else if self.cursor.peek_at(2) == ch3 {
            self.token.set_punctuator(tok3);
            self.cursor.advance(3);
        } else {
            self.token.set_punctuator(tok2);
            self.cursor.advance(2);
        }
    }

    // ---- Unicode escape consumers -------------------------------------------

    /// Consume `required_len` hex digits, accumulating them into a code point.
    /// Port of `consumeHex` (JSLexer.cpp:1329-1353). On failure, if
    /// `error_on_fail`, report "invalid hex number" at the offending position;
    /// returns `None`.
    fn consume_hex(&mut self, required_len: u32, error_on_fail: bool) -> Option<u32> {
        let mut cp: u32 = 0;
        for _ in 0..required_len {
            let mut ch = self.cursor.peek() as u32;
            if (b'0' as u32..=b'9' as u32).contains(&ch) {
                ch -= b'0' as u32;
            } else {
                // Now that we know it is not a digit, it is safe to lowercase.
                ch |= 32;
                if (b'a' as u32..=b'f' as u32).contains(&ch) {
                    ch -= b'a' as u32 - 10;
                } else {
                    if error_on_fail {
                        let loc = self.cur_loc();
                        self.error(loc, "invalid hex number");
                    }
                    return None;
                }
            }
            cp = (cp << 4) + ch;
            self.cursor.advance(1);
        }
        Some(cp)
    }

    /// Consume a braced code point escape `{HHHH}` (the cursor is on `{`).
    /// Port of `consumeBracedCodePoint` (JSLexer.cpp:1355-1428). Reproduces the
    /// empty / invalid-character / too-large / non-terminated error paths and
    /// the `failed` flag + `error_on_fail` gating.
    fn consume_braced_code_point(&mut self, error_on_fail: bool) -> Option<u32> {
        debug_assert!(self.cursor.peek() == b'{', "braced codepoint must begin with {{");
        self.cursor.advance(1);
        let start = self.cur_loc();
        let start_offset = self.cursor.offset();

        // Set to true if we failed to get a code point that is in bounds or saw
        // an invalid character.
        let mut failed = false;

        // Loop until we hit the } or eof, max out the value, or see an invalid
        // char.
        let mut cp: u32 = 0;
        while self.cursor.peek() != b'}' {
            let raw = self.cursor.peek();
            let ch_val: u32;
            if (b'0'..=b'9').contains(&raw) {
                ch_val = (raw - b'0') as u32;
            } else if (b'a'..=b'f').contains(&raw) {
                ch_val = (raw - (b'a' - 10)) as u32;
            } else if (b'A'..=b'F').contains(&raw) {
                ch_val = (raw - (b'A' - 10)) as u32;
            } else {
                // The only way this can be the end of the buffer is if this is a
                // \0. Check if this is the end of the buffer, else continue so
                // that we may report more errors after this braced code point.
                if self.cursor.at_end() {
                    if !failed && error_on_fail {
                        self.error(start, "non-terminated unicode codepoint escape");
                    }
                    return None;
                }
                // Invalid character, set the failed flag and continue.
                if !failed && error_on_fail {
                    let loc = self.cur_loc();
                    if !self.error(loc, "invalid character in unicode codepoint escape") {
                        return None;
                    }
                }
                failed = true;
                self.cursor.advance(1);
                continue;
            }
            cp = (cp << 4) + ch_val;
            if cp > UNICODE_MAX_VALUE {
                // Number grew too big, set the failed flag and continue.
                if !failed && error_on_fail {
                    if !self.error(start, "unicode codepoint escape is too large") {
                        return None;
                    }
                }
                failed = true;
            }
            self.cursor.advance(1);
        }

        debug_assert!(
            !self.cursor.at_end(),
            "bufferEnd_ should cause early return"
        );

        // An empty escape sequence is invalid.
        if self.cursor.offset() == start_offset {
            if !failed && error_on_fail {
                if !self.error(start, "empty unicode codepoint escape") {
                    return None;
                }
            }
            failed = true;
        }

        // Consume the final } and return.
        self.cursor.advance(1);
        if failed {
            None
        } else {
            Some(cp)
        }
    }

    /// Consume a `\u`/`\u{}` escape (the cursor is on `\`). Port of
    /// `consumeUnicodeEscape` (JSLexer.cpp:1159-1190). On error reports a
    /// diagnostic and returns `UNICODE_REPLACEMENT_CHARACTER`.
    fn consume_unicode_escape(&mut self) -> u32 {
        debug_assert!(self.cursor.peek() == b'\\');
        let backslash_offset = self.cursor.offset();
        self.cursor.advance(1);

        if self.cursor.peek() != b'u' {
            let range = SMRange {
                start: SMLoc {
                    source: self.buf_id,
                    offset: backslash_offset,
                },
                end: SMLoc {
                    source: self.buf_id,
                    offset: backslash_offset + 2,
                },
            };
            self.error_range(range, "invalid Unicode escape");
            return UNICODE_REPLACEMENT_CHARACTER;
        }
        self.cursor.advance(1);

        if self.cursor.peek() == b'{' {
            return match self.consume_braced_code_point(true) {
                // consumeBracedCodePoint has reported an error.
                None => UNICODE_REPLACEMENT_CHARACTER,
                Some(cp) => cp,
            };
        }

        match self.consume_hex(4, true) {
            None => UNICODE_REPLACEMENT_CHARACTER,
            // We don't need to check for valid UTF-16. JavaScript allows invalid
            // surrogate pairs, so we just encode every UTF-16 code into a UTF-8
            // sequence, even though theoretically it is not a valid UTF-8.
            Some(cp) => cp,
        }
    }

    /// Optionally consume a `\u`/`\u{}` escape: on ANY failure, reset the cursor
    /// to the start and return `None`. Port of `consumeUnicodeEscapeOptional`
    /// (JSLexer.cpp:1192-1226). Used by regexp/templates (ported now for
    /// completeness).
    #[allow(dead_code)]
    fn consume_unicode_escape_optional(&mut self) -> Option<u32> {
        let start = self.cursor.offset();
        debug_assert!(self.cursor.peek() == b'\\');
        self.cursor.advance(1);

        if self.cursor.peek() != b'u' {
            self.cursor.seek(start);
            return None;
        }
        self.cursor.advance(1);

        if self.cursor.peek() == b'{' {
            // Avoid reporting an error because we are consuming the escape
            // optionally.
            match self.consume_braced_code_point(false) {
                None => {
                    self.cursor.seek(start);
                    None
                }
                Some(cp) => Some(cp),
            }
        } else {
            match self.consume_hex(4, false) {
                None => {
                    self.cursor.seek(start);
                    None
                }
                Some(cp) => Some(cp),
            }
        }
    }

    // ---- The phase-1b stubs -------------------------------------------------

    /// Stub for the not-yet-implemented scanners (identifiers, numbers, strings,
    /// templates, regexp, private identifiers, the `\` escape and the non-ASCII
    /// default arm). Reports an error and forces EOF so the lexer is
    /// well-defined; these inputs are not part of the phase-1a corpus.
    fn scan_not_implemented(&mut self) {
        let loc = self.token.start_loc();
        self.error(loc, "not yet implemented (phase 1b)");
        self.force_eof();
        self.token.set_eof();
    }

    /// Advance to the next token and return it. Port of `JSLexer::advance`
    /// (JSLexer.cpp:255-745). Only the punctuator/whitespace/comment/EOF arms
    /// are implemented in phase 1a; the literal/identifier arms are stubbed.
    pub fn advance(&mut self, grammar_context: GrammarContext) -> &Token {
        self.new_line_before_current_token = false;

        loop {
            debug_assert!(self.cursor.offset() <= self.cursor.raw().len() as u32 - 1);
            let c = self.cursor.peek();
            match c {
                0 => {
                    self.set_token_start();
                    // Faithful to JSLexer.cpp case 0: both the at-EOF branch and the
                    // post-error branch set EOF (clippy flags the duplicate arms).
                    #[allow(clippy::if_same_then_else)]
                    if self.cursor.at_end() {
                        self.token.set_eof();
                    } else if !self.error(self.token.start_loc(), "unrecognized Unicode character \\u0000")
                    {
                        self.token.set_eof();
                    } else {
                        self.cursor.advance(1);
                        continue;
                    }
                }

                // PUNC_L1_1 single-char punctuators.
                b'}' => self.punc_l1_1(TokenKind::r_brace),
                b'(' => self.punc_l1_1(TokenKind::l_paren),
                b')' => self.punc_l1_1(TokenKind::r_paren),
                b'[' => self.punc_l1_1(TokenKind::l_square),
                b']' => self.punc_l1_1(TokenKind::r_square),
                b';' => self.punc_l1_1(TokenKind::semi),
                b',' => self.punc_l1_1(TokenKind::comma),
                b'~' => self.punc_l1_1(TokenKind::tilde),
                b':' => self.punc_l1_1(TokenKind::colon),

                // { {|  (the `{|` form is Flow-only Type context; not exercised)
                b'{' => {
                    self.set_token_start();
                    self.token.set_punctuator(TokenKind::l_brace);
                    self.cursor.advance(1);
                }

                // = => == ===
                b'=' => {
                    self.set_token_start();
                    if self.cursor.peek_at(1) == b'>' {
                        self.token.set_punctuator(TokenKind::equalgreater);
                        self.cursor.advance(2);
                    } else if self.cursor.peek_at(1) != b'=' {
                        self.token.set_punctuator(TokenKind::equal);
                        self.cursor.advance(1);
                    } else if self.cursor.peek_at(2) == b'=' {
                        self.token.set_punctuator(TokenKind::equalequalequal);
                        self.cursor.advance(3);
                    } else {
                        self.token.set_punctuator(TokenKind::equalequal);
                        self.cursor.advance(2);
                    }
                }

                // ! != !==
                b'!' => self.punc_l3_3(
                    b'=',
                    TokenKind::exclaimequal,
                    b'=',
                    TokenKind::exclaimequalequal,
                    TokenKind::exclaim,
                ),

                // + ++ +=
                b'+' => self.punc_l2_3(
                    b'+',
                    TokenKind::plusplus,
                    b'=',
                    TokenKind::plusequal,
                    TokenKind::plus,
                ),
                // - -- -=
                b'-' => self.punc_l2_3(
                    b'-',
                    TokenKind::minusminus,
                    b'=',
                    TokenKind::minusequal,
                    TokenKind::minus,
                ),

                // & && &= &&=
                b'&' => {
                    self.set_token_start();
                    if self.cursor.peek_at(1) == b'&' {
                        if self.cursor.peek_at(2) == b'=' {
                            self.token.set_punctuator(TokenKind::ampampequal);
                            self.cursor.advance(3);
                        } else {
                            self.token.set_punctuator(TokenKind::ampamp);
                            self.cursor.advance(2);
                        }
                    } else if self.cursor.peek_at(1) == b'=' {
                        self.token.set_punctuator(TokenKind::ampequal);
                        self.cursor.advance(2);
                    } else {
                        self.token.set_punctuator(TokenKind::amp);
                        self.cursor.advance(1);
                    }
                }

                // | || |= ||=  (the `|}` form is Flow-only Type context)
                b'|' => {
                    self.set_token_start();
                    if self.cursor.peek_at(1) == b'|' {
                        if self.cursor.peek_at(2) == b'=' {
                            self.token.set_punctuator(TokenKind::pipepipeequal);
                            self.cursor.advance(3);
                        } else {
                            self.token.set_punctuator(TokenKind::pipepipe);
                            self.cursor.advance(2);
                        }
                    } else if self.cursor.peek_at(1) == b'=' {
                        self.token.set_punctuator(TokenKind::pipeequal);
                        self.cursor.advance(2);
                    } else {
                        self.token.set_punctuator(TokenKind::pipe);
                        self.cursor.advance(1);
                    }
                }

                // ? ?? ?. ??=
                b'?' => {
                    self.set_token_start();
                    if self.cursor.peek_at(1) == b'.' && !is_ascii_digit(self.cursor.peek_at(2)) {
                        // OptionalChainingPunctuator ::
                        // ?. [lookahead does not contain DecimalDigit]
                        // This is done to prevent `x?.3:y` from being recognized
                        // as `x ?. 3 : y` instead of `x ? .3 : y`.
                        self.token.set_punctuator(TokenKind::questiondot);
                        self.cursor.advance(2);
                    } else if self.cursor.peek_at(1) == b'?'
                        && grammar_context != GrammarContext::Type
                    {
                        if self.cursor.peek_at(2) == b'=' {
                            self.token.set_punctuator(TokenKind::questionquestionequal);
                            self.cursor.advance(3);
                        } else {
                            self.token.set_punctuator(TokenKind::questionquestion);
                            self.cursor.advance(2);
                        }
                    } else {
                        self.token.set_punctuator(TokenKind::question);
                        self.cursor.advance(1);
                    }
                }

                // * *= ** **=
                b'*' => {
                    self.set_token_start();
                    if self.cursor.peek_at(1) == b'=' {
                        self.token.set_punctuator(TokenKind::starequal);
                        self.cursor.advance(2);
                    } else if self.cursor.peek_at(1) != b'*' {
                        self.token.set_punctuator(TokenKind::star);
                        self.cursor.advance(1);
                    } else if self.cursor.peek_at(2) == b'=' {
                        self.token.set_punctuator(TokenKind::starstarequal);
                        self.cursor.advance(3);
                    } else {
                        self.token.set_punctuator(TokenKind::starstar);
                        self.cursor.advance(2);
                    }
                }

                // ^ ^=
                b'^' => self.punc_l2_2(b'=', TokenKind::caret, TokenKind::caretequal),

                // % %=
                b'%' => {
                    self.set_token_start();
                    if self.cursor.peek_at(1) == b'=' {
                        self.token.set_punctuator(TokenKind::percentequal);
                        self.cursor.advance(2);
                    } else {
                        self.token.set_punctuator(TokenKind::percent);
                        self.cursor.advance(1);
                    }
                }

                // \r \n : line terminators set the newline flag.
                b'\r' | b'\n' => {
                    self.cursor.advance(1);
                    self.new_line_before_current_token = true;
                    continue;
                }

                // Line separator U+2028 (e2 80 a8) / Paragraph separator U+2029
                // (e2 80 a9), or fall through to the default (non-ASCII) arm.
                UTF8_LINE_TERMINATOR_CHAR0 => {
                    if match_unicode_line_terminator_offset1(&self.cursor.raw()[self.cursor.offset() as usize..])
                    {
                        self.cursor.advance(3);
                        self.new_line_before_current_token = true;
                        continue;
                    } else {
                        self.scan_not_implemented();
                    }
                }

                // \v \f : whitespace.
                0x0b | 0x0c => {
                    self.cursor.advance(1);
                    continue;
                }

                // \t and space: tight loop to skip runs.
                b'\t' | b' ' => {
                    // Spaces frequently come in groups, so use a tight inner loop.
                    loop {
                        self.cursor.advance(1);
                        let n = self.cursor.peek();
                        if n != b'\t' && n != b' ' {
                            break;
                        }
                    }
                    continue;
                }

                // No-break space U+00A0 is UTF8 encoded as: c2 a0
                0xc2 => {
                    if self.cursor.peek_at(1) == 0xa0 {
                        self.cursor.advance(2);
                        continue;
                    } else {
                        self.scan_not_implemented();
                    }
                }

                // Byte-order mark U+FEFF is encoded as: ef bb bf
                0xef => {
                    if self.cursor.peek_at(1) == 0xbb && self.cursor.peek_at(2) == 0xbf {
                        self.cursor.advance(3);
                        continue;
                    } else {
                        self.scan_not_implemented();
                    }
                }

                // / // /* /=  and (in AllowRegExp) regexp.
                b'/' => {
                    if self.cursor.peek_at(1) == b'/' {
                        // Line comment.
                        self.scan_line_comment();
                        continue;
                    } else if self.cursor.peek_at(1) == b'*' {
                        // Block comment.
                        self.skip_block_comment();
                        continue;
                    } else {
                        self.set_token_start();
                        // NOTE: regexp scanning is deferred to a later phase. The
                        // differential only drives AllowDiv, so we treat
                        // AllowRegExp identically to AllowDiv here (slash /
                        // slashequal) with this TODO. When regexp lands,
                        // AllowRegExp must call scanRegExp() instead.
                        if self.cursor.peek_at(1) == b'=' {
                            self.token.set_punctuator(TokenKind::slashequal);
                            self.cursor.advance(2);
                        } else {
                            self.token.set_punctuator(TokenKind::slash);
                            self.cursor.advance(1);
                        }
                    }
                }

                // # : hashbang (only at buffer start) or private identifier
                // (deferred to phase 1b).
                b'#' => {
                    if self.cursor.offset() == 0 && self.cursor.peek_at(1) == b'!' {
                        // #! (hashbang) at the very start of the buffer.
                        self.scan_line_comment();
                        continue;
                    }
                    self.set_token_start();
                    // scanPrivateIdentifier is deferred to phase 1b.
                    self.scan_not_implemented();
                }

                // < <= << <<=
                b'<' => {
                    self.set_token_start();
                    if self.cursor.peek_at(1) == b'=' {
                        self.token.set_punctuator(TokenKind::lessequal);
                        self.cursor.advance(2);
                    } else if self.cursor.peek_at(1) == b'<' {
                        if self.cursor.peek_at(2) == b'=' {
                            self.token.set_punctuator(TokenKind::lesslessequal);
                            self.cursor.advance(3);
                        } else {
                            self.token.set_punctuator(TokenKind::lessless);
                            self.cursor.advance(2);
                        }
                    } else {
                        self.token.set_punctuator(TokenKind::less);
                        self.cursor.advance(1);
                    }
                }

                // > >= >> >>> >>= >>>=
                b'>' => {
                    self.set_token_start();
                    if self.cursor.peek_at(1) == b'=' {
                        // >=
                        self.token.set_punctuator(TokenKind::greaterequal);
                        self.cursor.advance(2);
                    } else if self.cursor.peek_at(1) == b'>' {
                        // >>
                        if self.cursor.peek_at(2) == b'=' {
                            // >>=
                            self.token.set_punctuator(TokenKind::greatergreaterequal);
                            self.cursor.advance(3);
                        } else if self.cursor.peek_at(2) == b'>' {
                            // >>>
                            if self.cursor.peek_at(3) == b'=' {
                                // >>>=
                                self.token
                                    .set_punctuator(TokenKind::greatergreatergreaterequal);
                                self.cursor.advance(4);
                            } else {
                                self.token.set_punctuator(TokenKind::greatergreatergreater);
                                self.cursor.advance(3);
                            }
                        } else {
                            self.token.set_punctuator(TokenKind::greatergreater);
                            self.cursor.advance(2);
                        }
                    } else {
                        self.token.set_punctuator(TokenKind::greater);
                        self.cursor.advance(1);
                    }
                }

                // . ... or .NNN (a number — deferred to phase 1b).
                b'.' => {
                    self.set_token_start();
                    if self.cursor.peek_at(1) >= b'0' && self.cursor.peek_at(1) <= b'9' {
                        // scanNumber is deferred to phase 1b.
                        self.scan_not_implemented();
                    } else if self.cursor.peek_at(1) == b'.' && self.cursor.peek_at(2) == b'.' {
                        self.token.set_punctuator(TokenKind::dotdotdot);
                        self.cursor.advance(3);
                    } else {
                        self.token.set_punctuator(TokenKind::period);
                        self.cursor.advance(1);
                    }
                }

                // 0-9 : numbers (deferred to phase 1b).
                b'0'..=b'9' => {
                    self.set_token_start();
                    self.scan_not_implemented();
                }

                // Identifier fast path (deferred to phase 1b).
                b'_' | b'$' | b'a'..=b'z' | b'A'..=b'Z' => {
                    self.set_token_start();
                    self.scan_not_implemented();
                }

                // @ : decorator punctuator (non-Type context).
                b'@' => {
                    self.set_token_start();
                    self.token.set_punctuator(TokenKind::at);
                    self.cursor.advance(1);
                }

                // \ : identifier with a leading unicode escape (deferred).
                b'\\' => {
                    self.set_token_start();
                    self.scan_not_implemented();
                }

                // ' " : string literals (deferred to phase 1b).
                b'\'' | b'"' => {
                    self.set_token_start();
                    self.scan_not_implemented();
                }

                // ` : template literal (deferred to phase 1b).
                b'`' => {
                    self.set_token_start();
                    self.scan_not_implemented();
                }

                // Default: non-ASCII identifier-start / unicode-only space /
                // unrecognized character. Deferred to phase 1b.
                _ => {
                    self.set_token_start();
                    self.scan_not_implemented();
                }
            }

            // Always terminate the loop unless "continue" was used.
            break;
        } // loop

        self.finish_token();
        &self.token
    }

    // ---- Comment scanners ---------------------------------------------------

    /// Skip a line comment (or `#!` hashbang), tracking the newline flag.
    /// Port of `lineCommentHelper` + `scanLineComment` (JSLexer.cpp:1430-1510).
    ///
    /// NOTE: comment STORAGE and magic-comment URL parsing (`//# sourceURL=` /
    /// `//# sourceMappingURL=`) are deferred to a later phase; here we only skip
    /// the comment correctly and track the newline flag.
    fn scan_line_comment(&mut self) {
        debug_assert!(
            (self.cursor.peek() == b'/' && self.cursor.peek_at(1) == b'/')
                || (self.cursor.peek() == b'#' && self.cursor.peek_at(1) == b'!')
        );
        // Skip the two-character opening delimiter.
        self.cursor.advance(2);

        loop {
            let c = self.cursor.peek();
            match c {
                0 => {
                    if self.cursor.at_end() {
                        break;
                    } else {
                        self.cursor.advance(1);
                    }
                }
                b'\r' | b'\n' => {
                    self.cursor.advance(1);
                    self.new_line_before_current_token = true;
                    break;
                }
                UTF8_LINE_TERMINATOR_CHAR0 => {
                    if match_unicode_line_terminator_offset1(
                        &self.cursor.raw()[self.cursor.offset() as usize..],
                    ) {
                        self.cursor.advance(3);
                        self.new_line_before_current_token = true;
                        break;
                    } else {
                        self.decode_utf8_skip();
                    }
                }
                _ => {
                    if crate::utf8::is_utf8_start(c) {
                        self.decode_utf8_skip();
                    } else {
                        self.cursor.advance(1);
                    }
                }
            }
        }
    }

    /// Skip a block comment (`/* ... */`), tracking the newline flag.
    /// Port of `skipBlockComment` (JSLexer.cpp:1512-1571). Comment STORAGE is
    /// deferred. A non-terminated block comment reports an error + a "comment
    /// started here" note, matching the C++.
    fn skip_block_comment(&mut self) {
        debug_assert!(self.cursor.peek() == b'/' && self.cursor.peek_at(1) == b'*');
        let block_comment_start = self.cur_loc();
        // Skip the "/*" opening delimiter.
        self.cursor.advance(2);

        loop {
            let c = self.cursor.peek();
            match c {
                0 => {
                    if self.cursor.at_end() {
                        let loc = self.cur_loc();
                        self.error(loc, "non-terminated block comment");
                        self.sm.note(block_comment_start, "comment started here");
                        break;
                    } else {
                        self.cursor.advance(1);
                    }
                }
                b'\r' | b'\n' => {
                    self.cursor.advance(1);
                    self.new_line_before_current_token = true;
                }
                UTF8_LINE_TERMINATOR_CHAR0 => {
                    if match_unicode_line_terminator_offset1(
                        &self.cursor.raw()[self.cursor.offset() as usize..],
                    ) {
                        self.cursor.advance(3);
                        self.new_line_before_current_token = true;
                    } else {
                        self.decode_utf8_skip();
                    }
                }
                b'*' => {
                    self.cursor.advance(1);
                    if self.cursor.peek() == b'/' {
                        self.cursor.advance(1);
                        break;
                    }
                }
                _ => {
                    if crate::utf8::is_utf8_start(c) {
                        self.decode_utf8_skip();
                    } else {
                        self.cursor.advance(1);
                    }
                }
            }
        }
    }

    /// Decode the UTF-8 sequence at the cursor and advance past it, swallowing
    /// any decode errors. Mirrors the member `_decodeUTF8SlowPath(cur)` used
    /// inside the comment scanners (which advances the pointer; errors are
    /// reported via the member but we ignore them inside trivia for 1a).
    fn decode_utf8_skip(&mut self) {
        let raw = self.cursor.raw();
        let mut i = self.cursor.offset() as usize;
        let _ = decode_utf8::<true>(raw, &mut i, |_| {});
        // Advance the cursor by however many bytes were consumed (at least 1).
        let consumed = i - self.cursor.offset() as usize;
        self.cursor.advance(consumed.max(1));
    }

    /// Format the current token like the C++ `js-lexer-dump` line (without the
    /// trailing newline): `"<start> <end> <nl> <KIND>"`. Phase-1a tokens are
    /// fieldless (punctuators / eof), so no per-kind fields are emitted.
    pub fn dump_token(&self, out: &mut String) {
        use std::fmt::Write;
        let start = self.token.start_loc().offset;
        let end = self.token.end_loc().offset;
        let nl = if self.new_line_before_current_token {
            "nl"
        } else {
            "--"
        };
        let _ = write!(out, "{} {} {} {}", start, end, nl, variant_name(self.token.kind()));
    }
}

/// \return true if `ch` is an ASCII decimal digit.
#[inline]
fn is_ascii_digit(ch: u8) -> bool {
    ch.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;
    use atom_table::AtomTable;
    use support::manager::SourceErrorManager;

    /// Build a lexer over `src`, call `consume_unicode_escape` with the cursor
    /// on the leading `\`, and return the decoded code point unless an error was
    /// emitted (in which case `None`).
    fn consume_escape_for_test(src: &str) -> Option<u32> {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        let cp = lex.consume_unicode_escape();
        if lex.sm.error_count() != 0 {
            None
        } else {
            Some(cp)
        }
    }

    #[test]
    fn unicode_escape_4hex_and_braced() {
        assert_eq!(consume_escape_for_test("\\u0041"), Some(0x41)); // 'A'
        assert_eq!(consume_escape_for_test("\\u{1F600}"), Some(0x1F600));
        assert_eq!(consume_escape_for_test("\\u{}"), None); // empty -> error
        assert_eq!(consume_escape_for_test("\\uXY"), None); // bad hex -> error
    }

    fn kinds(src: &str) -> Vec<TokenKind> {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        let mut out = vec![];
        loop {
            let k = lex.advance(GrammarContext::AllowDiv).kind();
            out.push(k);
            if k == TokenKind::eof {
                break;
            }
        }
        out
    }

    #[test]
    fn punctuators_and_comments() {
        use TokenKind::*;
        assert_eq!(
            kinds("{ } ( ) ;"),
            vec![l_brace, r_brace, l_paren, r_paren, semi, eof]
        );
        // Comments and whitespace are skipped; only the `;` punctuators remain.
        assert_eq!(kinds("; /* c */ ;"), vec![semi, semi, eof]);
        assert_eq!(kinds("; // line\n;"), vec![semi, semi, eof]);
    }

    #[test]
    fn newline_flag_tracks_line_terminators() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", ";\n;");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        // First `;` : no newline before it.
        assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::semi);
        assert!(!lex.is_new_line_before_current_token());
        // Second `;` : preceded by a newline.
        assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::semi);
        assert!(lex.is_new_line_before_current_token());
    }
}
