//! JSLexer, ported from lib/Parser/JSLexer.cpp.
//!
//! Phase 1a implements the lexer skeleton and `advance()` for punctuators,
//! whitespace, line/block comments, and EOF. Identifiers, numbers, strings,
//! templates, regexp, private identifiers and the `\` identifier-escape arm are
//! stubbed: they report a "not yet implemented (phase 1b)" error and force EOF,
//! so the lexer is well-defined for those inputs but they are not part of the
//! phase-1a corpus.

use std::rc::Rc;

use atom_table::{AtomBytes, AtomTable};
use support::buffer::SourceBuffer;
use support::diag::Subsystem;
use support::location::{SMLoc, SMRange, SourceId};
use support::manager::SourceErrorManager;

use unicode::{
    is_unicode_id_continue, is_unicode_id_start, is_unicode_only_id_start,
    is_unicode_only_space, UNICODE_MAX_VALUE, UNICODE_REPLACEMENT_CHARACTER,
};

use crate::cursor::Cursor;
use crate::number;
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

/// The identifier-scanning mode, port of `JSLexer::IdentifierMode`. Affects
/// which extra characters are accepted as identifier parts: JSX accepts `-`,
/// Flow accepts `@`. Only `JS` is exercised in phase 1b-i; the JSX/Flow arms are
/// carried for forward-compatibility but never reached (the differential drives
/// `--context=div`).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum IdentifierMode {
    JS,
    JSX,
    Flow,
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
    strtab: &'a AtomTable,

    /// Pre-interned reserved-word identifiers, indexed by
    /// `ord(kind) - ord(_first_resword)`. Port of `resWordIdent_`.
    res_word_idents: Vec<AtomBytes>,

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
        let mut lexer = JSLexer {
            sm,
            buf_id,
            cursor,
            strtab,
            res_word_idents: Vec::new(),
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
        };
        lexer.initialize_reserved_identifiers();
        lexer
    }

    /// Pre-intern all reserved words so that `res_word_ident` is a cheap lookup.
    /// Port of `initializeReservedIdentifiers` (JSLexer.cpp:111-115).
    fn initialize_reserved_identifiers(&mut self) {
        use crate::token_kinds::{ord, token_kind_str_by_ord, TokenKind};
        let first = ord(TokenKind::_first_resword);
        let last = ord(TokenKind::_last_resword);
        // Index by `ord(kind) - ord(_first_resword)`. We allocate one slot per
        // ordinal in the inclusive marker range so `res_word_ident` can index
        // directly; the two marker slots are never read.
        let count = (last - first + 1) as usize;
        self.res_word_idents = Vec::with_capacity(count);
        for v in first..=last {
            // The marker slots (`_first_resword`/`_last_resword`) get an empty
            // placeholder; every real reserved word interns its name.
            let name = if v > first && v < last {
                token_kind_str_by_ord(v)
            } else {
                ""
            };
            self.res_word_idents
                .push(self.strtab.atom_bytes(name.as_bytes()));
        }
    }

    /// \return the pre-interned identifier for reserved word `kind`. Port of
    /// `resWordIdent` (JSLexer.h:445-449).
    fn res_word_ident(&self, kind: TokenKind) -> AtomBytes {
        use crate::token_kinds::{ord, TokenKind as TK};
        debug_assert!(kind.is_res_word());
        self.res_word_idents[(ord(kind) - ord(TK::_first_resword)) as usize]
    }

    /// \return the current token.
    pub fn token(&self) -> &Token {
        &self.token
    }

    /// \return whether the lexer is in strict mode. Port of `isStrictMode`.
    pub fn is_strict_mode(&self) -> bool {
        self.strict_mode
    }

    /// Set strict mode (affects future-reserved-word recognition). Port of
    /// `setStrictMode`.
    pub fn set_strict_mode(&mut self, strict_mode: bool) {
        self.strict_mode = strict_mode;
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

    /// Consume up to `max_len` octal digits (the cursor is on the first octal
    /// digit), accumulating them into a byte. Port of `consumeOctal`
    /// (JSLexer.cpp:1311-1327), including the strict-mode error.
    fn consume_octal(&mut self, mut max_len: u32) -> u8 {
        debug_assert!(self.cursor.peek() >= b'0' && self.cursor.peek() <= b'7');

        if self.strict_mode {
            let loc = SMLoc {
                source: self.buf_id,
                offset: self.cursor.offset() - 1,
            };
            if !self.error(loc, "octals not allowed in strict mode") {
                return 0;
            }
        }

        let mut res: u8 = self.cursor.peek() - b'0';
        self.cursor.advance(1);
        max_len -= 1;
        while max_len != 0 && self.cursor.peek() >= b'0' && self.cursor.peek() <= b'7' {
            res = (res << 3) + (self.cursor.peek() - b'0');
            self.cursor.advance(1);
            max_len -= 1;
        }

        res
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

    // ---- Identifier scanners ------------------------------------------------

    /// Try to consume the start of an identifier into `tmp_storage`. Port of
    /// `consumeIdentifierStart` (JSLexer.cpp:1228-1267). Returns true if an
    /// identifier start was consumed (used by the private-identifier scanner,
    /// which is deferred — hence currently unexercised).
    #[allow(dead_code)]
    fn consume_identifier_start(&mut self) -> bool {
        let c = self.cursor.peek();
        if c == b'_' || c == b'$' || ((c | 32) >= b'a' && (c | 32) <= b'z') {
            self.tmp_storage.clear();
            self.tmp_storage.push(c);
            self.cursor.advance(1);
            return true;
        }

        if c == b'\\' {
            let start_loc = self.cur_loc();
            self.tmp_storage.clear();
            let cp = self.consume_unicode_escape();
            if !is_unicode_id_start(cp) {
                self.error_range(
                    SMRange {
                        start: start_loc,
                        end: self.cur_loc(),
                    },
                    format!("Unicode escape \\u{:x}is not a valid identifier start", cp),
                );
            } else {
                append_unicode_to_storage(&mut self.tmp_storage, cp);
            }
            return true;
        }

        if !is_utf8_start(c) {
            return false;
        }

        let (cp, next) = self.cursor.peek_utf8();
        if is_unicode_id_start(cp) {
            self.tmp_storage.clear();
            append_unicode_to_storage(&mut self.tmp_storage, cp);
            self.cursor.seek(next);
            return true;
        }

        false
    }

    /// Try to consume one non-escaped identifier part into `tmp_storage`. Port
    /// of `consumeOneIdentifierPartNoEscape<Mode>` (JSLexer.cpp:1269-1290).
    #[inline]
    fn consume_one_identifier_part_no_escape(&mut self, mode: IdentifierMode) -> bool {
        let ch = self.cursor.peek();
        if ch == b'_'
            || ch == b'$'
            || ((ch | 32) >= b'a' && (ch | 32) <= b'z')
            || ch.is_ascii_digit()
            || (mode == IdentifierMode::JSX && ch == b'-')
            || (mode == IdentifierMode::Flow && ch == b'@')
        {
            self.tmp_storage.push(ch);
            self.cursor.advance(1);
            return true;
        } else if is_utf8_start(ch) {
            // If we have encountered a Unicode character, we try to decode it. If
            // it can be a part of the identifier, we consume it, otherwise we
            // leave it alone.
            let (cp, next) = self.cursor.peek_utf8();
            if is_unicode_id_continue(cp) {
                append_unicode_to_storage(&mut self.tmp_storage, cp);
                self.cursor.seek(next);
                return true;
            }
        }
        false
    }

    /// Consume identifier parts into `tmp_storage`. Port of
    /// `consumeIdentifierParts<Mode>` (JSLexer.cpp:1292-1309).
    fn consume_identifier_parts(&mut self, mode: IdentifierMode) {
        loop {
            // Try consuming a non-escaped identifier part. Failing that, check
            // for an escape.
            if self.consume_one_identifier_part_no_escape(mode) {
                continue;
            } else if self.cursor.peek() == b'\\' {
                // Decode the escape.
                let start_loc = self.cur_loc();
                let cp = self.consume_unicode_escape();
                if !is_unicode_id_continue(cp) {
                    self.error_range(
                        SMRange {
                            start: start_loc,
                            end: self.cur_loc(),
                        },
                        format!(
                            "Unicode escape \\u{:x} is not a valid identifier codepoint",
                            cp
                        ),
                    );
                } else {
                    append_unicode_to_storage(&mut self.tmp_storage, cp);
                }
            } else {
                break;
            }
        }
    }

    /// Recognise a reserved word from `bytes`, applying the non-strict-mode
    /// future-reserved-word filter. Port of `scanReservedWord`
    /// (JSLexer.cpp:1865-1887).
    fn scan_reserved_word(&self, bytes: &[u8]) -> TokenKind {
        let mut rw = match_reserved_word(bytes);

        // Check for "Future reserved words" which should not be recognised in
        // non-strict mode.
        if !self.strict_mode && rw != TokenKind::identifier {
            match rw {
                TokenKind::rw_implements
                | TokenKind::rw_interface
                | TokenKind::rw_package
                | TokenKind::rw_private
                | TokenKind::rw_protected
                | TokenKind::rw_public
                | TokenKind::rw_static
                | TokenKind::rw_yield => {
                    rw = TokenKind::identifier;
                }
                _ => {}
            }
        }
        rw
    }

    /// Dispatch `scan_identifier_fast_path` with the right `IdentifierMode` for
    /// the grammar context. Port of `scanIdentifierFastPathInContext`
    /// (JSLexer.h:992-1006). Only JS mode is exercised in 1b-i.
    fn scan_identifier_fast_path_in_context(
        &mut self,
        start: u32,
        grammar_context: GrammarContext,
    ) {
        let mode = if grammar_context == GrammarContext::AllowJSXIdentifier {
            IdentifierMode::JSX
        } else if grammar_context == GrammarContext::Type {
            IdentifierMode::Flow
        } else {
            IdentifierMode::JS
        };
        self.scan_identifier_fast_path(start, mode);
    }

    /// Dispatch `scan_identifier_parts` with the right `IdentifierMode` for the
    /// grammar context. Port of `scanIdentifierPartsInContext` (JSLexer.h).
    fn scan_identifier_parts_in_context(&mut self, grammar_context: GrammarContext) {
        let mode = if grammar_context == GrammarContext::AllowJSXIdentifier {
            IdentifierMode::JSX
        } else if grammar_context == GrammarContext::Type {
            IdentifierMode::Flow
        } else {
            IdentifierMode::JS
        };
        self.scan_identifier_parts(mode);
    }

    /// Scan an identifier assuming no Unicode escapes / UTF-8 (the common case),
    /// falling back to the slow path on the first escape or UTF-8 byte. Port of
    /// `scanIdentifierFastPath<Mode>` (JSLexer.cpp:1889-1933). `start` is the
    /// byte offset of the first identifier character (the cursor is there).
    fn scan_identifier_fast_path(&mut self, start: u32, mode: IdentifierMode) {
        // Quickly consume the ASCII identifier part.
        let mut end = start;
        let raw = self.cursor.raw();
        let ch = loop {
            end += 1;
            let ch = raw[end as usize];
            if !(ch == b'_'
                || ch == b'$'
                || ((ch | 32) >= b'a' && (ch | 32) <= b'z')
                || ch.is_ascii_digit()
                || (mode == IdentifierMode::JSX && ch == b'-')
                || (mode == IdentifierMode::Flow && ch == b'@'))
            {
                break ch;
            }
        };

        // Check whether a slow part of the identifier follows.
        if ch == b'\\' {
            // An escape. Pass the baton to the slow path.
            self.tmp_storage.clear();
            self.tmp_storage
                .extend_from_slice(&self.cursor.raw()[start as usize..end as usize]);
            self.cursor.seek(end);
            self.scan_identifier_parts(mode);
            return;
        } else if is_utf8_start(ch) {
            // If we have encountered a Unicode character, we try to decode it. If
            // it can be a part of the identifier, we consume it, otherwise we
            // leave it alone.
            self.cursor.seek(end);
            let (cp, next) = self.cursor.peek_utf8();
            if is_unicode_id_continue(cp) {
                self.tmp_storage.clear();
                self.tmp_storage
                    .extend_from_slice(&self.cursor.raw()[start as usize..end as usize]);
                append_unicode_to_storage(&mut self.tmp_storage, cp);
                self.cursor.seek(next);
                self.scan_identifier_parts(mode);
                return;
            }
            // Not an id-continue: the identifier ends at `end`; cursor already
            // seeked there.
        } else {
            self.cursor.seek(end);
        }

        let slice = &self.cursor.raw()[start as usize..end as usize];
        let rw = self.scan_reserved_word(slice);
        if rw != TokenKind::identifier {
            let ident = self.res_word_ident(rw);
            self.token.set_res_word(rw, ident);
        } else {
            let ident = self.strtab.atom_bytes(slice);
            self.token.set_identifier(ident);
        }
    }

    /// Scan the remaining identifier parts via the slow path (`tmp_storage`
    /// already holds the prefix). Port of `scanIdentifierParts<Mode>`
    /// (JSLexer.cpp:1935-1949). A reserved word reached through a unicode escape
    /// ALSO emits a warning.
    fn scan_identifier_parts(&mut self, mode: IdentifierMode) {
        self.consume_identifier_parts(mode);
        let rw = self.scan_reserved_word(&self.tmp_storage);
        if rw != TokenKind::identifier {
            let ident = self.res_word_ident(rw);
            self.token.set_res_word(rw, ident);
            let range = SMRange {
                start: self.token.start_loc(),
                end: self.cur_loc(),
            };
            self.sm.warning_range(
                support::diag::Warning::Misc,
                range,
                "scanning identifier with unicode escape as reserved word",
                Subsystem::Lexer,
            );
        } else {
            let ident = self.strtab.atom_bytes(self.tmp_storage.as_slice());
            self.token.set_identifier(ident);
        }
    }

    // ---- Number scanner -----------------------------------------------------

    /// Scan a numeric literal (or BigInt). Port of `JSLexer::scanNumber`
    /// (JSLexer.cpp:1573-1856). The cursor is positioned at the first character
    /// of the number (a digit, or `.` for the `.NNN` form). On return the token
    /// is set to `numeric_literal` or `bigint_literal`.
    fn scan_number(&mut self, grammar_context: GrammarContext) {
        // A somewhat ugly state machine for scanning a number

        let mut radix: u32 = 10;
        let mut real = false;
        let mut ok = true;
        // Byte offset of the token start (incl. any radix prefix). Port of
        // `rawStart`.
        let raw_start = self.cursor.offset();
        // Byte offset of the first significant digit. For radix-prefixed forms
        // this is moved past the prefix. Port of `start`.
        let mut start = self.cursor.offset();

        // True when we encounter the numeric literal separator: '_'.
        let mut seen_separator = false;

        // True when we encounter a legacy octal number (starts with '0').
        let mut legacy_octal = false;

        // A label-less reimplementation of the C++ `goto` state machine. The
        // `Phase` enum records which state to enter next; `end` is reached either
        // by falling through the integer loop or via the fraction/exponent
        // states.
        enum Phase {
            IntegerLoop,
            Fraction,
            Exponent,
            End,
        }
        let mut phase: Phase;

        // Detect the radix
        if self.cursor.peek() == b'0' {
            let c1 = self.cursor.peek_at(1);
            if (c1 | 32) == b'x' {
                radix = 16;
                self.cursor.advance(2);
                start += 2;
                phase = Phase::IntegerLoop;
            } else if (c1 | 32) == b'o' {
                radix = 8;
                self.cursor.advance(2);
                start += 2;
                phase = Phase::IntegerLoop;
            } else if (c1 | 32) == b'b' {
                radix = 2;
                self.cursor.advance(2);
                start += 2;
                phase = Phase::IntegerLoop;
            } else if c1 == b'.' {
                self.cursor.advance(2);
                phase = Phase::Fraction;
            } else if (c1 | 32) == b'e' {
                self.cursor.advance(2);
                phase = Phase::Exponent;
            } else {
                radix = 8;
                legacy_octal = true;
                self.cursor.advance(1);
                phase = Phase::IntegerLoop;
            }
        } else {
            phase = Phase::IntegerLoop;
        }

        if let Phase::IntegerLoop = phase {
            while is_ascii_digit(self.cursor.peek())
                || (radix == 16
                    && (self.cursor.peek() | 32) >= b'a'
                    && (self.cursor.peek() | 32) <= b'f')
                || self.cursor.peek() == b'_'
            {
                seen_separator |= self.cursor.peek() == b'_';
                self.cursor.advance(1);
            }

            phase = Phase::End;
            if radix == 10 || legacy_octal {
                // It is not necessarily an integer.
                // We could have interpreted as legacyOctal initially but will
                // have to change to decimal later.
                if self.cursor.peek() == b'.' {
                    self.cursor.advance(1);
                    phase = Phase::Fraction;
                } else if (self.cursor.peek() | 32) == b'e' {
                    self.cursor.advance(1);
                    phase = Phase::Exponent;
                }
            }
        }

        if let Phase::Fraction = phase {
            // We arrive here after we have consumed the decimal dot ".".
            real = true;
            while is_ascii_digit(self.cursor.peek()) || self.cursor.peek() == b'_' {
                seen_separator |= self.cursor.peek() == b'_';
                self.cursor.advance(1);
            }

            if (self.cursor.peek() | 32) == b'e' {
                self.cursor.advance(1);
                phase = Phase::Exponent;
            } else {
                phase = Phase::End;
            }
        }

        if let Phase::Exponent = phase {
            // We arrive here after we have consumed the exponent char 'e' or 'E'.
            real = true;
            if self.cursor.peek() == b'+' || self.cursor.peek() == b'-' {
                self.cursor.advance(1);
            }
            if is_ascii_digit(self.cursor.peek()) {
                loop {
                    seen_separator |= self.cursor.peek() == b'_';
                    self.cursor.advance(1);
                    if !(is_ascii_digit(self.cursor.peek()) || self.cursor.peek() == b'_') {
                        break;
                    }
                }
            } else {
                ok = false;
            }
            phase = Phase::End;
        }

        debug_assert!(matches!(phase, Phase::End));

        // We arrive here after we have consumed all we can from the number. Now,
        // as per the spec, we consume a sequence of identifier characters if they
        // follow directly, which means the number is invalid if it's not BigInt.
        if self.consume_identifier_start() {
            self.consume_identifier_parts(IdentifierMode::JS);

            // raw == the full literal source [rawStart, curCharPtr_).
            let cur = self.cursor.offset();
            let raw = self.cursor.raw()[raw_start as usize..cur as usize].to_vec();
            if ok
                && !real
                && (!legacy_octal || raw == b"0n")
                && self.tmp_storage == b"n"
            {
                debug_assert!(
                    cur > start,
                    "Must consume at least the trailing n."
                );
                // digits == [start, curCharPtr_ - 1) (drop the trailing 'n').
                let digits = self.cursor.raw()[start as usize..(cur - 1) as usize].to_vec();
                // Use parseIntWithRadixDigits to validate the bigint literal's
                // digits. The digits themselves can be ignored, since we're only
                // interested in whether the string was parsed correctly.
                if !digits.is_empty()
                    && number::parse_int_with_radix_digits(
                        &digits,
                        radix,
                        /* allow_sep */ true,
                        |_| {},
                    )
                {
                    // This is a BigInt.
                    // ESTree spec:
                    // bigint property is the string representation of the BigInt
                    // value. It must contain only decimal digits and not include
                    // numeric separators (_) or the suffix n.
                    // Filter out the characters we don't want.
                    // Drop the last character from `raw` because that's the 'n',
                    // and skip over all '_'.
                    self.tmp_storage.clear();
                    for &c in &raw[..raw.len() - 1] {
                        if c != b'_' {
                            self.tmp_storage.push(c);
                        }
                    }
                    let value = self.strtab.atom_bytes(self.tmp_storage.as_slice());
                    let raw_atom = self.strtab.atom_bytes(raw.as_slice());
                    self.token.set_bigint_literal(value, raw_atom);
                    return;
                }

                // This is a BigInt with invalid digits; fail.
            }

            ok = false;
        }

        let cur = self.cursor.offset();
        let start_loc = self.token.start_loc();
        // Every arm of the chain below assigns `val`; we call
        // `set_numeric_literal(val)` exactly once at the single exit point at
        // the end, mirroring the C++ `done:` label. The C++ `goto done` early
        // exits (the error-limit cases that set `val = NaN`) are reproduced
        // with labeled-block breaks (`'done`) that short-circuit the rest of
        // their arm but still fall through to the final single set.
        let val: f64;

        if !ok {
            self.error_range(
                SMRange {
                    start: start_loc,
                    end: self.cur_loc(),
                },
                "invalid numeric literal",
            );
            val = f64::NAN;
        } else if !real
            && radix == 10
            && (cur - start) <= 9
            && !seen_separator
        {
            // If this is a decimal integer of at most 9 digits (log10(2**31-1),
            // it can fit in a 32-bit integer. Use a faster conversion.
            let bytes = self.cursor.raw();
            let mut idx = start as usize;
            let mut ival: i32 = (bytes[idx] - b'0') as i32;
            idx += 1;
            while idx != cur as usize {
                ival = ival * 10 + (bytes[idx] - b'0') as i32;
                idx += 1;
            }
            val = ival as f64;
        } else if real || radix == 10 {
            // Labeled block: the C++ `goto done` error-limit early exits set
            // `val = NaN` and break straight to the final single set.
            val = 'done: {
            if legacy_octal {
                if self.strict_mode || grammar_context == GrammarContext::Type {
                    if !self.error_range(
                        SMRange {
                            start: start_loc,
                            end: self.cur_loc(),
                        },
                        "Decimals with leading zeros are not allowed in strict mode",
                    ) {
                        break 'done f64::NAN;
                    }
                } else {
                    // Check to see if we can actually scan this as radix 10.
                    // Non-integer numbers must be in base 10, otherwise we error.
                    self.update_legacy_octal_radix(start, &mut radix);
                    if radix != 10 {
                        if !self.error_range(
                            SMRange {
                                start: start_loc,
                                end: self.cur_loc(),
                            },
                            "Octal numeric literals must be integers",
                        ) {
                            break 'done f64::NAN;
                        }
                    }
                }
            }

            let mut buf: Vec<u8> = Vec::with_capacity((cur - start) as usize + 1);
            // Own the digit slice so the per-character checks below can borrow
            // `self` mutably for error reporting (the C++ indexes the live
            // pointer; the owned copy is equivalent because the buffer is
            // immutable).
            let bytes = self.cursor.raw()[start as usize..(cur + 1) as usize].to_vec();
            if seen_separator {
                let mut it = 0usize;
                let nbytes = (cur - start) as usize;
                while it != nbytes {
                    let c = bytes[it];
                    if c != b'_' {
                        buf.push(c);
                    } else {
                        // Check to ensure that '_' is surrounded by digits.
                        // This is safe because the source buffer is
                        // zero-terminated and we know that the numeric literal
                        // didn't start with '_'. Note that we could have a 0b_11
                        // literal, but we'd still fail properly because of the
                        // radix==16 check.
                        let prev = bytes[it - 1];
                        let next = bytes[it + 1];
                        if !is_ascii_digit(prev)
                            && !(radix == 16
                                && b'a' <= (prev | 32)
                                && (prev | 32) <= b'f')
                        {
                            self.error_range(
                                SMRange {
                                    start: start_loc,
                                    end: self.cur_loc(),
                                },
                                "numeric separator must come after a digit",
                            );
                        } else if !is_ascii_digit(next)
                            && !(radix == 16
                                && b'a' <= (next | 32)
                                && (next | 32) <= b'f')
                        {
                            self.error_range(
                                SMRange {
                                    start: start_loc,
                                    end: self.cur_loc(),
                                },
                                "numeric separator must come before a digit",
                            );
                        }
                    }
                    it += 1;
                }
            } else {
                buf.extend_from_slice(&bytes[0..(cur - start) as usize]);
            }
            match number::str_to_double(&buf) {
                Some(v) => v,
                None => {
                    self.error_range(
                        SMRange {
                            start: start_loc,
                            end: self.cur_loc(),
                        },
                        "invalid numeric literal",
                    );
                    f64::NAN
                }
            }
            };
        } else {
            // Labeled block: the C++ `goto done` error-limit early exit sets
            // `val = NaN` and breaks straight to the final single set.
            val = 'done: {
            if legacy_octal
                && (self.strict_mode || grammar_context == GrammarContext::Type)
                && (cur - start) > 1
            {
                if !self.error_range(
                    SMRange {
                        start: start_loc,
                        end: self.cur_loc(),
                    },
                    "Octal literals must use '0o' in strict mode",
                ) {
                    break 'done f64::NAN;
                }
            }

            // Handle the zero-radix case. This could only happen with radix 16
            // because otherwise start wouldn't have been changed.
            if cur == start {
                let prefix = self.cursor.raw()[(start - 2) as usize..start as usize].to_vec();
                self.error_range(
                    SMRange {
                        start: start_loc,
                        end: self.cur_loc(),
                    },
                    format!(
                        "No digits after {}",
                        String::from_utf8_lossy(&prefix)
                    ),
                );
                f64::NAN
            } else {
                // Parse the rest of the number:
                if legacy_octal {
                    self.update_legacy_octal_radix(start, &mut radix);
                    // LegacyOctalLikeDecimalIntegerLiteral cannot contain
                    // separators.
                    if seen_separator {
                        self.error_range(
                            SMRange {
                                start: start_loc,
                                end: self.cur_loc(),
                            },
                            "Numeric separator cannot be used in literal after leading 0",
                        );
                    }
                }
                let digits = self.cursor.raw()[start as usize..cur as usize].to_vec();
                match number::parse_int_with_radix(&digits, radix, /* allow_sep */ true) {
                    Some(v) => v,
                    None => {
                        self.error_range(
                            SMRange {
                                start: start_loc,
                                end: self.cur_loc(),
                            },
                            "invalid integer literal",
                        );
                        f64::NAN
                    }
                }
            }
            };
        }

        // Single exit (C++ `done:`): set the numeric literal value exactly once.
        self.token.set_numeric_literal(val);
    }

    /// ES6.0 B.1.1: if we encounter a "legacy" octal number (starting with a
    /// '0') but the integer contains '8' or '9' we interpret it as decimal.
    /// Port of the `updateLegacyOctalRadix` lambda inside `scanNumber`
    /// (JSLexer.cpp:1717-1736). `start` is the byte offset of the first digit;
    /// `radix` is updated to 10 (with a warning) on an 8/9 digit.
    fn update_legacy_octal_radix(&mut self, start: u32, radix: &mut u32) {
        let cur = self.cursor.offset();
        let bytes = self.cursor.raw();
        let mut scan = start as usize;
        while scan != cur as usize {
            let c = bytes[scan];
            if c == b'.' || c == b'e' {
                break;
            }
            if c >= b'8' && c != b'_' {
                let range = SMRange {
                    start: self.token.start_loc(),
                    end: SMLoc {
                        source: self.buf_id,
                        offset: cur,
                    },
                };
                self.sm.warning_range(
                    support::diag::Warning::Misc,
                    range,
                    "Numeric literal starts with 0 but contains an 8 or 9 digit. \
                     Interpreting as decimal (not octal).",
                    Subsystem::Lexer,
                );
                *radix = 10;
                break;
            }
            scan += 1;
        }
    }

    // ---- String literal scanner ---------------------------------------------

    /// Scan a string literal (the cursor is on the opening quote). Port of
    /// `JSLexer::scanString<false>` (JSLexer.cpp:1977-2126), i.e. the non-JSX
    /// path; the JSX `&`-HTML-entity arm and the JSX newline-in-string arm are
    /// phase 3 and omitted here.
    fn scan_string(&mut self) {
        debug_assert!(self.cursor.peek() == b'\'' || self.cursor.peek() == b'"');
        // NOTE: `convert_surrogates` is off by default and the differential never
        // enables it. The `convertSurrogatesInString` re-encoding path is
        // DEFERRED (needs UTF-16 conversion utilities), so we intern
        // `tmp_storage` directly.
        debug_assert!(!self.convert_surrogates);
        let quote_ch = self.cursor.peek();
        self.cursor.advance(1);

        // Track whether we encounter any escapes or new line continuations. We
        // need that information in order to detect directives.
        let mut escapes = false;

        self.tmp_storage.clear();

        loop {
            let c = self.cursor.peek();
            if c == quote_ch {
                self.cursor.advance(1);
                break;
            } else if c == b'\\' {
                escapes = true;
                self.cursor.advance(1);
                let e = self.cursor.peek();
                match e {
                    b'\'' | b'"' | b'\\' => {
                        self.tmp_storage.push(e);
                        self.cursor.advance(1);
                    }

                    b'b' => {
                        self.cursor.advance(1);
                        self.tmp_storage.push(8);
                    }
                    b'f' => {
                        self.cursor.advance(1);
                        self.tmp_storage.push(12);
                    }
                    b'n' => {
                        self.cursor.advance(1);
                        self.tmp_storage.push(10);
                    }
                    b'r' => {
                        self.cursor.advance(1);
                        self.tmp_storage.push(13);
                    }
                    b't' => {
                        self.cursor.advance(1);
                        self.tmp_storage.push(9);
                    }
                    b'v' => {
                        self.cursor.advance(1);
                        self.tmp_storage.push(11);
                    }

                    0 => {
                        // EOF?
                        if self.cursor.at_end() {
                            // eof?
                            let loc = self.cur_loc();
                            self.error(loc, "non-terminated string");
                            let start = self.token.start_loc();
                            self.sm.note(start, "string started here");
                            break;
                        } else {
                            self.tmp_storage.push(e);
                            self.cursor.advance(1);
                        }
                    }

                    b'0' => {
                        // '\0' is not an octal so handle it separately.
                        if !(self.cursor.peek_at(1) >= b'0' && self.cursor.peek_at(1) <= b'7') {
                            self.cursor.advance(1);
                            append_unicode_to_storage(&mut self.tmp_storage, 0);
                        } else {
                            let v = self.consume_octal(3) as u32;
                            append_unicode_to_storage(&mut self.tmp_storage, v);
                        }
                    }
                    b'1' | b'2' | b'3' => {
                        let v = self.consume_octal(3) as u32;
                        append_unicode_to_storage(&mut self.tmp_storage, v);
                    }
                    b'4' | b'5' | b'6' | b'7' => {
                        let v = self.consume_octal(2) as u32;
                        append_unicode_to_storage(&mut self.tmp_storage, v);
                    }

                    b'x' => {
                        self.cursor.advance(1);
                        let v = self.consume_hex(2, true);
                        append_unicode_to_storage(&mut self.tmp_storage, v.unwrap_or(0));
                    }

                    b'u' => {
                        // Back up one so the cursor is on the '\\'.
                        self.cursor.seek(self.cursor.offset() - 1);
                        let cp = self.consume_unicode_escape();
                        append_unicode_to_storage(&mut self.tmp_storage, cp);
                    }

                    // Escaped line terminator. We just need to skip it.
                    b'\n' => {
                        self.cursor.advance(1);
                    }
                    b'\r' => {
                        self.cursor.advance(1);
                        if self.cursor.peek() == b'\n' {
                            // skip CR LF
                            self.cursor.advance(1);
                        }
                    }
                    UTF8_LINE_TERMINATOR_CHAR0 => {
                        if match_unicode_line_terminator_offset1(
                            &self.cursor.raw()[self.cursor.offset() as usize..],
                        ) {
                            self.cursor.advance(3);
                        } else {
                            let cp = self.decode_utf8_advance();
                            append_unicode_to_storage(&mut self.tmp_storage, cp);
                        }
                    }

                    _ => {
                        if is_utf8_start(e) {
                            let cp = self.decode_utf8_advance();
                            append_unicode_to_storage(&mut self.tmp_storage, cp);
                        } else {
                            self.tmp_storage.push(e);
                            self.cursor.advance(1);
                        }
                    }
                }
            } else if c == b'\n' || c == b'\r' {
                // A raw new line in a (non-JSX) string is not allowed.
                let loc = self.cur_loc();
                self.error(loc, "non-terminated string");
                let start = self.token.start_loc();
                self.sm.note(start, "string started here");
                break;
            } else if c == 0 && self.cursor.at_end() {
                let loc = self.cur_loc();
                self.error(loc, "non-terminated string");
                let start = self.token.start_loc();
                self.sm.note(start, "string started here");
                break;
            } else if is_utf8_start(c) {
                // Decode and re-encode the character and append it to the string
                // storage.
                let cp = self.decode_utf8_advance();
                append_unicode_to_storage(&mut self.tmp_storage, cp);
            } else {
                self.tmp_storage.push(c);
                self.cursor.advance(1);
            }
        }

        let atom = self.strtab.atom_bytes(self.tmp_storage.as_slice());
        self.token.set_string_literal(atom, escapes);
    }

    // ---- Private identifier scanner -----------------------------------------

    /// Scan a private identifier (the cursor is on `#`). Port of
    /// `scanPrivateIdentifier` (JSLexer.cpp:1951-1975). Returns false (and emits
    /// an "empty private identifier" error) if `#` is not followed by an
    /// identifier.
    fn scan_private_identifier(&mut self) -> bool {
        debug_assert!(self.cursor.peek() == b'#');

        // Skip the '#'.
        let start = self.cur_loc();
        self.cursor.advance(1);

        // Scan the actual identifier.
        if is_ascii_identifier_start(self.cursor.peek()) {
            let here = self.cursor.offset();
            self.scan_identifier_fast_path(here, IdentifierMode::JS);
        } else if self.consume_identifier_start() {
            // The cursor has been updated by consume_identifier_start.
            self.scan_identifier_parts(IdentifierMode::JS);
        } else {
            self.error(start, "empty private identifier");
            return false;
        }

        // Parsed a resword or identifier.
        // Convert the TokenKind to private_identifier after the fact.
        // This avoids adding another Mode to IdentifierMode.
        let ident = self.token.get_res_word_or_identifier();
        self.token.set_private_identifier(ident);

        true
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

                // # : hashbang (only at buffer start) or private identifier.
                b'#' => {
                    if self.cursor.offset() == 0 && self.cursor.peek_at(1) == b'!' {
                        // #! (hashbang) at the very start of the buffer.
                        self.scan_line_comment();
                        continue;
                    }
                    self.set_token_start();
                    if !self.scan_private_identifier() {
                        continue;
                    }
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
                        self.scan_number(grammar_context);
                    } else if self.cursor.peek_at(1) == b'.' && self.cursor.peek_at(2) == b'.' {
                        self.token.set_punctuator(TokenKind::dotdotdot);
                        self.cursor.advance(3);
                    } else {
                        self.token.set_punctuator(TokenKind::period);
                        self.cursor.advance(1);
                    }
                }

                // 0-9 : numbers.
                b'0'..=b'9' => {
                    self.set_token_start();
                    self.scan_number(grammar_context);
                }

                // Identifier fast path.
                b'_' | b'$' | b'a'..=b'z' | b'A'..=b'Z' => {
                    self.set_token_start();
                    let start = self.cursor.offset();
                    self.scan_identifier_fast_path_in_context(start, grammar_context);
                }

                // @ : decorator punctuator (non-Type context).
                b'@' => {
                    self.set_token_start();
                    self.token.set_punctuator(TokenKind::at);
                    self.cursor.advance(1);
                }

                // \ : identifier with a leading unicode escape.
                // Port of JSLexer.cpp:683-698.
                b'\\' => {
                    self.set_token_start();
                    self.tmp_storage.clear();
                    let cp = self.consume_unicode_escape();
                    if !is_unicode_id_start(cp) {
                        self.error_range(
                            SMRange {
                                start: self.token.start_loc(),
                                end: self.cur_loc(),
                            },
                            format!(
                                "Unicode escape \\u{:x} is not a valid identifier start",
                                cp
                            ),
                        );
                        continue;
                    } else {
                        append_unicode_to_storage(&mut self.tmp_storage, cp);
                    }
                    self.scan_identifier_parts_in_context(grammar_context);
                }

                // ' " : string literals.
                b'\'' | b'"' => {
                    self.set_token_start();
                    self.scan_string();
                }

                // ` : template literal (deferred to phase 1b).
                b'`' => {
                    self.set_token_start();
                    self.scan_not_implemented();
                }

                // Default: non-ASCII identifier-start / unicode-only space /
                // unrecognized character. Port of JSLexer.cpp:711-735.
                _ => {
                    self.set_token_start();
                    let ch = self.decode_utf8_advance();

                    if is_unicode_only_id_start(ch) {
                        self.tmp_storage.clear();
                        append_unicode_to_storage(&mut self.tmp_storage, ch);
                        self.scan_identifier_parts_in_context(grammar_context);
                    } else if is_unicode_only_space(ch) {
                        continue;
                    } else {
                        let range = SMRange {
                            start: self.token.start_loc(),
                            end: self.cur_loc(),
                        };
                        if ch > 31 && ch < 127 {
                            self.error_range(
                                range,
                                format!("unrecognized character '{}'", ch as u8 as char),
                            );
                        } else {
                            self.error_range(
                                range,
                                format!("unrecognized Unicode character \\u{:x}", ch),
                            );
                        }
                        continue;
                    }
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

    /// Decode the UTF-8 sequence at the cursor, advance past it, and report any
    /// decode error at the start of the sequence. Port of the member
    /// `decodeUTF8` (JSLexer.h:1145-1151), which uses `decodeUTF8<false>`.
    fn decode_utf8_advance(&mut self) -> u32 {
        let save_start = self.cur_loc();
        let raw = self.cursor.raw();
        let mut i = self.cursor.offset() as usize;
        let mut err_msg: Option<String> = None;
        let cp = decode_utf8::<false>(raw, &mut i, |m| {
            if err_msg.is_none() {
                err_msg = Some(m.to_string());
            }
        });
        let consumed = (i - self.cursor.offset() as usize).max(1);
        self.cursor.advance(consumed);
        if let Some(msg) = err_msg {
            self.error(save_start, msg);
        }
        cp
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
    /// trailing newline): `"<start> <end> <nl> <KIND>[ <field> ...]"`. Phase-1b-i
    /// emits the `ident=` field for identifiers / private identifiers / reserved
    /// words; the other literal fields land in later phases.
    pub fn dump_token(&self, out: &mut String) {
        use std::fmt::Write;
        let start = self.token.start_loc().offset;
        let end = self.token.end_loc().offset;
        let nl = if self.new_line_before_current_token {
            "nl"
        } else {
            "--"
        };
        let kind = self.token.kind();
        let _ = write!(out, "{} {} {} {}", start, end, nl, variant_name(kind));
        self.emit_fields(out, kind);
    }

    /// Emit the per-kind dump fields. Port of `js-lexer-dump.cpp:emitFields`
    /// (the identifier/reserved-word cases; other cases land in later phases).
    fn emit_fields(&self, out: &mut String, kind: TokenKind) {
        match kind {
            TokenKind::identifier => {
                out.push_str(" ident=");
                quote_bytes(out, self.strtab.bytes(self.token.get_identifier()));
            }
            TokenKind::private_identifier => {
                out.push_str(" ident=");
                quote_bytes(out, self.strtab.bytes(self.token.get_private_identifier()));
            }
            TokenKind::string_literal => {
                use std::fmt::Write;
                let _ = write!(
                    out,
                    " escapes={}",
                    if self.token.get_string_literal_contains_escapes() {
                        1
                    } else {
                        0
                    }
                );
                out.push_str(" value=");
                quote_bytes(out, self.strtab.bytes(self.token.get_string_literal()));
            }
            TokenKind::numeric_literal => {
                use std::fmt::Write;
                // Match the harness `snprintf(" bits=0x%016llx", DoubleToBits)`:
                // 16-digit, zero-padded, lowercase hex of the f64 bit pattern.
                let bits = self.token.get_numeric_literal().to_bits();
                let _ = write!(out, " bits=0x{:016x}", bits);
            }
            TokenKind::bigint_literal => {
                out.push_str(" value=");
                quote_bytes(out, self.strtab.bytes(self.token.get_bigint_literal()));
                out.push_str(" raw=");
                quote_bytes(
                    out,
                    self.strtab.bytes(self.token.get_bigint_literal_raw_value()),
                );
            }
            _ => {
                // Reserved words: emit the identifier string.
                if kind.is_res_word() {
                    out.push_str(" ident=");
                    quote_bytes(out, self.strtab.bytes(self.token.get_res_word_identifier()));
                }
                // Punctuators and eof: no extra fields.
            }
        }
    }
}

/// Emit `bytes` quoted per the `js-lexer-dump` `Q()` spec into `out`. Port of
/// `quoteBytes` (js-lexer-dump.cpp:91-115): wrap in double quotes; `"`->`\"`,
/// `\`->`\\`, `\n`->`\n`, `\t`->`\t`, `\r`->`\r`; printable ASCII
/// `0x20..=0x7e` literal; every other byte as lowercase `\xHH`.
fn quote_bytes(out: &mut String, bytes: &[u8]) {
    out.push('"');
    for &c in bytes {
        match c {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            b'\r' => out.push_str("\\r"),
            0x20..=0x7e => out.push(c as char),
            _ => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                out.push_str("\\x");
                out.push(HEX[(c >> 4) as usize & 0xf] as char);
                out.push(HEX[c as usize & 0xf] as char);
            }
        }
    }
    out.push('"');
}

/// \return true if `ch` is an ASCII decimal digit.
#[inline]
fn is_ascii_digit(ch: u8) -> bool {
    ch.is_ascii_digit()
}

/// \return true if `ch` has the ID_Start property and is ASCII. Port of
/// `isASCIIIdentifierStart` (CharacterProperties.h:100-102).
#[inline]
fn is_ascii_identifier_start(ch: u8) -> bool {
    ch == b'_' || ch == b'$' || ((ch | 32) >= b'a' && (ch | 32) <= b'z')
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

    /// Like `kinds`, but the lexer is switched to non-strict mode.
    fn kinds_nonstrict(src: &str) -> Vec<TokenKind> {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.set_strict_mode(false);
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

    /// Lex `src` as a single identifier and return its interned bytes.
    fn ident_bytes(src: &str) -> Vec<u8> {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        let tok = lex.advance(GrammarContext::AllowDiv);
        assert_eq!(tok.kind(), TokenKind::identifier);
        let ab = tok.get_identifier();
        tab.bytes(ab).to_vec()
    }

    #[test]
    fn identifiers_and_reswords() {
        use TokenKind::*;
        assert_eq!(kinds("foo _bar $x9"), vec![identifier, identifier, identifier, eof]);
        assert_eq!(
            kinds("function for yield"),
            vec![rw_function, rw_for, rw_yield, eof]
        ); // strict mode default
           // non-strict: yield is an identifier
        assert_eq!(kinds_nonstrict("yield"), vec![identifier, eof]);
        // non-strict downgrade for the other future reserved words
        assert_eq!(
            kinds_nonstrict("implements interface package private protected public static"),
            vec![
                identifier, identifier, identifier, identifier, identifier, identifier,
                identifier, eof
            ]
        );
        // unicode identifier
        assert_eq!(kinds("\u{00e9}tude"), vec![identifier, eof]); // étude
                                                                  // escaped identifier start
        assert_eq!(kinds("\\u0041bc"), vec![identifier, eof]); // 'Abc'
                                                               // ident value round-trips through the interner
        assert_eq!(ident_bytes("caf\u{00e9}"), b"caf\xc3\xa9");
        // escaped identifier interns the decoded bytes
        assert_eq!(ident_bytes("\\u0041bc"), b"Abc");
    }

    /// Lex `src` as a single numeric literal and return its f64 bits.
    fn num_bits(src: &str) -> u64 {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        let tok = lex.advance(GrammarContext::AllowDiv);
        assert_eq!(tok.kind(), TokenKind::numeric_literal, "src={src:?}");
        tok.get_numeric_literal().to_bits()
    }

    /// Lex `src` as a single bigint literal and return (value, raw) bytes.
    fn bigint_bytes(src: &str) -> (Vec<u8>, Vec<u8>) {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        let tok = lex.advance(GrammarContext::AllowDiv);
        assert_eq!(tok.kind(), TokenKind::bigint_literal, "src={src:?}");
        let v = tab.bytes(tok.get_bigint_literal()).to_vec();
        let r = tab.bytes(tok.get_bigint_literal_raw_value()).to_vec();
        (v, r)
    }

    #[test]
    fn numbers_basic() {
        use TokenKind::*;
        assert_eq!(num_bits("5"), 5.0f64.to_bits());
        assert_eq!(num_bits("0.1"), 0.1f64.to_bits());
        assert_eq!(num_bits("0xff"), 255.0f64.to_bits());
        assert_eq!(num_bits("0o17"), 15.0f64.to_bits());
        assert_eq!(num_bits("0b1010"), 10.0f64.to_bits());
        assert_eq!(num_bits("1e10"), 1e10f64.to_bits());
        assert_eq!(num_bits("1_000"), 1000.0f64.to_bits());
        assert_eq!(num_bits(".5"), 0.5f64.to_bits());
        assert_eq!(num_bits("3.14e2"), 314.0f64.to_bits());
        assert_eq!(num_bits("0XAB"), (0xab as f64).to_bits());
        assert_eq!(num_bits("0o7"), 7.0f64.to_bits());
        assert_eq!(num_bits("0b11"), 3.0f64.to_bits());
        assert_eq!(num_bits("2E-3"), 2e-3f64.to_bits());
        // kind check
        assert_eq!(
            kinds("5 0xff 1.5"),
            vec![numeric_literal, numeric_literal, numeric_literal, eof]
        );
    }

    #[test]
    fn bigint_basic() {
        assert_eq!(bigint_bytes("10n"), (b"10".to_vec(), b"10n".to_vec()));
        assert_eq!(bigint_bytes("0xffn"), (b"0xff".to_vec(), b"0xffn".to_vec()));
        assert_eq!(bigint_bytes("255n"), (b"255".to_vec(), b"255n".to_vec()));
        assert_eq!(bigint_bytes("0n"), (b"0".to_vec(), b"0n".to_vec()));
        // separators stripped from value, kept in raw
        assert_eq!(
            bigint_bytes("1_000n"),
            (b"1000".to_vec(), b"1_000n".to_vec())
        );
    }

    /// Lex `src` as a single string literal and return its (cooked bytes,
    /// contains_escapes) pair.
    fn str_cooked(src: &str) -> (Vec<u8>, bool) {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        let tok = lex.advance(GrammarContext::AllowDiv);
        assert_eq!(tok.kind(), TokenKind::string_literal, "src={src:?}");
        let cooked = tab.bytes(tok.get_string_literal()).to_vec();
        let escapes = tok.get_string_literal_contains_escapes();
        (cooked, escapes)
    }

    #[test]
    fn strings_basic() {
        use TokenKind::*;
        assert_eq!(kinds("'a' \"b\""), vec![string_literal, string_literal, eof]);
        assert_eq!(str_cooked("'hello'"), (b"hello".to_vec(), false));
        assert_eq!(str_cooked("\"a\\tb\""), (b"a\tb".to_vec(), true)); // \t -> tab, escapes=true
        assert_eq!(str_cooked("'\\n\\r\\\\'"), (vec![10, 13, b'\\'], true));
        assert_eq!(str_cooked("'\\x41'"), (b"A".to_vec(), true)); // \x41 -> 'A'
        assert_eq!(str_cooked("'\\u00e9'"), (b"\xc3\xa9".to_vec(), true)); // é (WTF-8)
        assert_eq!(str_cooked("'a\\\nb'"), (b"ab".to_vec(), true)); // escaped newline continuation
        assert_eq!(str_cooked("'caf\u{00e9}'"), (b"caf\xc3\xa9".to_vec(), false)); // raw unicode, no escape
    }

    /// Lex `src` as a single private identifier and return its interned bytes.
    fn private_ident_bytes(src: &str) -> Vec<u8> {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        let tok = lex.advance(GrammarContext::AllowDiv);
        assert_eq!(tok.kind(), TokenKind::private_identifier);
        tab.bytes(tok.get_private_identifier()).to_vec()
    }

    #[test]
    fn private_identifiers() {
        use TokenKind::*;
        assert_eq!(
            kinds("#foo #bar"),
            vec![private_identifier, private_identifier, eof]
        );
        assert_eq!(private_ident_bytes("#x"), b"x"); // the interned name excludes '#'
                                                     // '#' followed by no identifier -> "empty private identifier"
                                                     // error; scan_private_identifier returns false -> no token.
        assert_eq!(kinds("#"), vec![eof]);
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
