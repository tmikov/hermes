//! JSLexer, a faithful port of `lib/Parser/JSLexer.cpp`.
//!
//! `JSLexer` lexes the full JavaScript token surface: punctuators, trivia
//! (whitespace and line/block comments), identifiers and keywords, numeric
//! literals, string literals, template literals, regular-expression literals,
//! private identifiers, JSX, and the Flow type context. It also exposes the
//! stateful lexer APIs: optional comment/token storage, magic comment
//! (`sourceURL`/`sourceMappingURL`) extraction, `SavePoint` for backtracking,
//! the directive (`"use strict"`) check, and `rescanRBrace` for template
//! continuations. The `impl<'a> JSLexer<'a>` methods are split across the child
//! modules below by concern (escape, identifier, number, string, template,
//! regexp, jsx, dump, state).

// Each child module can see the private fields of `JSLexer` (privacy in Rust is
// "visible to the declaring module and its descendants"), so no field needs to
// be made more public to support the split. Methods called across module
// boundaries are `pub(crate)`.
mod dump;
mod escape;
mod identifier;
mod jsx;
mod lookahead;
mod number;
mod regexp;
mod state;
mod string;
mod template;

pub use state::SavePoint;

use std::rc::Rc;

use atom_table::{AtomBytes, AtomTable};
use support::buffer::SourceBuffer;
use support::diag::Subsystem;
use support::location::{SMLoc, SMRange, SourceId};
use support::manager::SourceErrorManager;

use unicode::{
    is_unicode_id_start, is_unicode_only_id_start, is_unicode_only_space,
};

use crate::cursor::Cursor;
use crate::token::{CommentKind, StoredComment, StoredToken, Token};
use crate::token_kinds::TokenKind;
use crate::utf8::{
    append_unicode_to_storage, convert_utf16_to_utf8_with_replacements,
    convert_utf8_with_surrogates_to_utf16, decode_utf8,
    match_unicode_line_terminator_offset1, UTF8_LINE_TERMINATOR_CHAR0,
};

/// The grammar context affecting how some tokens are lexed (e.g. `/` as a
/// division operator vs. a regular-expression literal). Port of
/// `JSLexer::GrammarContext`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GrammarContext {
    /// A RegExp can follow, so `/` starts a regular-expression literal.
    AllowRegExp,
    /// `/` can follow, so it is scanned as the division operator.
    AllowDiv,
    /// `/` can follow, `-` is part of identifiers, and `>` is scanned as its
    /// own token.
    AllowJSXIdentifier,
    /// A type annotation: `/` can follow, `>>` scans as two separate `>`
    /// tokens, and legacy octal literals are rejected as in strict mode.
    Type,
}

/// The identifier-scanning mode, port of `JSLexer::IdentifierMode`. Affects
/// which extra characters are accepted as identifier parts: JSX accepts `-`,
/// Flow accepts `@`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum IdentifierMode {
    /// Standard JavaScript identifiers only.
    JS,
    /// JavaScript identifiers and '-'.
    JSX,
    /// JavaScript identifiers and identifiers which begin with '@'.
    Flow,
}

/// Type-level marker for the identifier-scanning mode: the Rust analog of the
/// C++ `template <IdentifierMode Mode>` non-type template parameter. Each impl
/// pins `MODE` to a compile-time constant, so the per-character mode checks in
/// the identifier scan loops (`M::MODE == IdentifierMode::JSX`, etc.) fold away
/// in every monomorphization — matching the C++ template specializations rather
/// than testing a runtime `mode` argument inside the inner loop.
pub(crate) trait IdMode {
    const MODE: IdentifierMode;
}
/// Standard JavaScript identifiers only.
pub(crate) struct JsMode;
impl IdMode for JsMode {
    const MODE: IdentifierMode = IdentifierMode::JS;
}
/// JavaScript identifiers and '-'.
pub(crate) struct JsxMode;
impl IdMode for JsxMode {
    const MODE: IdentifierMode = IdentifierMode::JSX;
}
/// JavaScript identifiers and identifiers which begin with '@'.
pub(crate) struct FlowMode;
impl IdMode for FlowMode {
    const MODE: IdentifierMode = IdentifierMode::Flow;
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

    /// Whether the lexer is in strict mode (affects reserved-word recognition
    /// in the identifier scanner and octal handling in the number/escape
    /// scanners). Port of `strictMode_`.
    strict_mode: bool,
    /// Whether to convert surrogate pairs while decoding. Port of
    /// `convertSurrogates_`.
    convert_surrogates: bool,

    /// Scratch storage for assembling identifier/string/regexp values. Port of
    /// `tmpStorage_`.
    tmp_storage: Vec<u8>,

    /// Scratch storage for assembling the Template Raw Value (TRV) of template
    /// literals. Port of `rawStorage_`.
    raw_storage: Vec<u8>,

    /// `//# sourceURL=` value, if seen (port of `sourceURL_`).
    source_url: Option<String>,
    /// `//# sourceMappingURL=` value, if seen (port of `sourceMappingURL_`).
    source_mapping_url: Option<String>,

    /// Whether to store comments encountered while lexing instead of skipping
    /// them. Port of `storeComments_`.
    store_comments: bool,
    /// Stored comments (only populated when `store_comments`). Port of
    /// `commentStorage_`.
    comment_storage: Vec<StoredComment>,
    /// Whether to store every token encountered while lexing. Port of
    /// `storeTokens_`.
    store_tokens: bool,
    /// Stored tokens (only populated when `store_tokens`). Port of
    /// `tokenStorage_`.
    token_storage: Vec<StoredToken>,
}

impl<'a> JSLexer<'a> {
    /// Construct a lexer over the buffer identified by `buf_id` in `sm`.
    /// Port of `JSLexer::JSLexer` + `initializeWithBufferId`. The reserved-word
    /// pre-interning (`initializeReservedIdentifiers`) is performed here so that
    /// `res_word_ident` is a cheap lookup during identifier scanning.
    pub fn new(
        buf_id: SourceId,
        sm: &'a mut SourceErrorManager,
        strtab: &'a AtomTable,
        grammar_context: GrammarContext,
    ) -> JSLexer<'a> {
        JSLexer::new_with_convert_surrogates(
            buf_id,
            sm,
            strtab,
            grammar_context,
            false,
        )
    }

    /// Like `new`, but with control over the `convert_surrogates` option. When
    /// `convert_surrogates` is set, `get_string_literal` re-encodes the internal
    /// WTF-8 string form into valid UTF-8 (combining surrogate pairs and
    /// replacing unpaired surrogates with U+FFFD). Port of the `JSLexer`
    /// constructor's `convertSurrogates` parameter.
    pub fn new_with_convert_surrogates(
        buf_id: SourceId,
        sm: &'a mut SourceErrorManager,
        strtab: &'a AtomTable,
        _grammar_context: GrammarContext,
        convert_surrogates: bool,
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
            convert_surrogates,
            tmp_storage: Vec::new(),
            raw_storage: Vec::new(),
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

    /// Re-encode the internal WTF-8 string `bytes` (which may contain lone
    /// surrogates / surrogate-encoded astral characters) into *valid* UTF-8,
    /// combining surrogate pairs into supplementary-plane characters and
    /// replacing unpaired surrogates with U+FFFD, then intern the result. Port
    /// of `convertSurrogatesInString` (JSLexer.cpp:2486-2495).
    fn convert_surrogates_in_string(&self, bytes: &[u8]) -> AtomBytes {
        let ustr = convert_utf8_with_surrogates_to_utf16(bytes);
        let output = convert_utf16_to_utf8_with_replacements(&ustr);
        self.strtab.atom_bytes(output)
    }

    /// Intern a string-literal value, applying the `convert_surrogates`
    /// re-encoding when the option is set. Port of `getStringLiteral`
    /// (JSLexer.h:689-694).
    pub fn get_string_literal(&self, bytes: &[u8]) -> AtomBytes {
        if self.convert_surrogates {
            self.convert_surrogates_in_string(bytes)
        } else {
            self.strtab.atom_bytes(bytes)
        }
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
    pub(crate) fn res_word_ident(&self, kind: TokenKind) -> AtomBytes {
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

    /// Set whether comments should be stored instead of skipped. Port of
    /// `setStoreComments`.
    pub fn set_store_comments(&mut self, store_comments: bool) {
        self.store_comments = store_comments;
    }

    /// \return whether tokens are being stored. Port of `getStoreTokens`.
    pub fn get_store_tokens(&self) -> bool {
        self.store_tokens
    }

    /// Set whether every token should be stored as it is lexed. Port of
    /// `setStoreTokens`.
    pub fn set_store_tokens(&mut self, store_tokens: bool) {
        self.store_tokens = store_tokens;
    }

    /// Unconditionally store the current token in the token storage. Port of
    /// `storeCurrentToken` (JSLexer.h:548-551).
    pub fn store_current_token(&mut self) {
        debug_assert!(
            self.store_tokens,
            "Tokens shouldn't be stored unless the flag is set"
        );
        self.token_storage.push(StoredToken::new(
            self.token.kind(),
            self.token.source_range(),
        ));
    }

    /// \return any stored comments to this point. Port of `getStoredComments`.
    pub fn get_stored_comments(&self) -> &[StoredComment] {
        &self.comment_storage
    }

    /// \return any stored comments to this point, moving them out of storage in
    /// the lexer and clearing the storage. Port of `moveStoredComments`.
    pub fn move_stored_comments(&mut self) -> Vec<StoredComment> {
        std::mem::take(&mut self.comment_storage)
    }

    /// \return any stored tokens to this point. Port of `getStoredTokens`.
    pub fn get_stored_tokens(&self) -> &[StoredToken] {
        &self.token_storage
    }

    /// \return the source URL from the magic comment, or `None` if there was no
    /// magic comment. Port of `getSourceURL`.
    pub fn get_source_url(&self) -> Option<&str> {
        self.source_url.as_deref()
    }

    /// \return the source mapping URL from the magic comment, or `None` if there
    /// was no magic comment. Port of `getSourceMappingURL`.
    pub fn get_source_mapping_url(&self) -> Option<&str> {
        self.source_mapping_url.as_deref()
    }

    /// \return the end location of the previous token.
    pub fn prev_token_end(&self) -> SMLoc {
        self.prev_token_end
    }

    /// \return the current char pointer location. Port of `getCurLoc`
    /// (JSLexer.h:567-569), which returns `SMLoc::getFromPointer(curCharPtr_)`;
    /// the offset-based equivalent is the cursor's current location.
    pub fn get_cur_loc(&self) -> SMLoc {
        self.cur_loc()
    }

    /// \return the source buffer id we're currently parsing. Port of
    /// `getBufferId` (JSLexer.h:704-706).
    pub fn get_buffer_id(&self) -> SourceId {
        self.buf_id
    }

    /// \return the SourceErrorManager. Port of `getSourceMgr` (JSLexer.h:516).
    pub fn get_source_mgr(&self) -> &SourceErrorManager {
        self.sm
    }

    /// \return a mutable reference to the SourceErrorManager so the parser can
    /// report errors through the lexer. Mirrors the non-const `getSourceMgr()`
    /// overload in `JSLexer.h:516` (C++ returns a non-const reference).
    pub fn get_source_mgr_mut(&mut self) -> &mut SourceErrorManager {
        self.sm
    }

    /// \return the string interner. Port of `getStringTable` (JSLexer.h:523).
    /// (The C++ `getAllocator` has no Rust analog — the port uses the global
    /// allocator and `AtomTable`'s own interning, so there is no bump allocator.)
    pub fn get_string_table(&self) -> &AtomTable {
        self.strtab
    }

    /// \return the logical bytes of the buffer (the source text without the
    /// trailing NUL sentinel). Pointer->offset adaptation of `getBufferStart`/
    /// `getBufferEnd` (JSLexer.h:709-716): C++ returns `bufferStart_`/
    /// `bufferEnd_` pointers; the offset-based equivalent is the buffer byte
    /// slice, with `get_buffer_start` == 0 and `get_buffer_end` == its length.
    pub fn buffer_bytes(&self) -> &[u8] {
        let raw = self.cursor.raw();
        // `raw` includes the trailing NUL sentinel; drop it for the logical
        // bytes (mirroring [bufferStart_, bufferEnd_)).
        &raw[..raw.len() - 1]
    }

    /// \return the start offset of the buffer (always 0). Pointer->offset
    /// adaptation of `getBufferStart` (JSLexer.h:708-711).
    pub fn get_buffer_start(&self) -> u32 {
        0
    }

    /// \return the end offset of the buffer (the logical byte length, excluding
    /// the trailing NUL sentinel). Pointer->offset adaptation of `getBufferEnd`
    /// (JSLexer.h:713-716).
    pub fn get_buffer_end(&self) -> u32 {
        self.buffer_bytes().len() as u32
    }

    /// For certain identifier-like syntactic forms, like Flow's
    /// `renders? number`, we need to check that the `?` comes immediately after
    /// `renders` with no whitespace. Port of `Token::checkFollowingCharacter`
    /// (JSLexer.h:229-234), relocated from `Token` to `JSLexer`: our offset-based
    /// `Token` has no buffer reference, so the check reads the buffer byte at the
    /// current token's end offset.
    ///
    /// \return true iff the character directly after the current token matches
    /// `c`. The next byte could be the EOF NUL or the start of a UTF-8 sequence,
    /// but it is always present (the buffer is NUL-terminated, so the end offset
    /// is always in-bounds).
    pub fn check_following_character(&self, c: u8) -> bool {
        debug_assert!(c < 128, "test character must be ASCII");
        self.cursor.raw()[self.token.end_loc().offset as usize] == c
    }

    /// \return the source text `[start, end)` of the current token. Port of
    /// `Token::inputStr` (JSLexer.h:136-140), relocated from `Token` to
    /// `JSLexer`: our offset-based `Token` has no buffer reference, so the slice
    /// is taken from the cursor's buffer.
    pub fn token_input_str(&self) -> &[u8] {
        self.cursor.slice(
            self.token.start_loc().offset,
            self.token.end_loc().offset,
        )
    }

    /// Intern an identifier. Port of `getIdentifier(StringRef)`
    /// (JSLexer.h:685-687).
    pub fn get_identifier(&self, name: &[u8]) -> AtomBytes {
        self.strtab.atom_bytes(name)
    }

    /// Convert the current token to an identifier-operator token. Port of
    /// `convertCurTokenToIdentOp` (JSLexer.h:827-831).
    /// \pre the current token is an identifier which is an IDENT_OP operator.
    pub fn convert_cur_token_to_ident_op(&mut self, kind: TokenKind) {
        debug_assert_eq!(self.token.kind(), TokenKind::identifier);
        debug_assert_eq!(
            self.strtab.bytes(self.token.get_identifier()),
            crate::token_kinds::token_kind_str(kind).as_bytes()
        );
        self.token.set_ident_op(kind);
    }

    /// A location at the current cursor offset.
    #[inline]
    pub(crate) fn cur_loc(&self) -> SMLoc {
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

    /// Force an EOF at the next token. Port of `forceEOF`.
    #[inline]
    pub fn force_eof(&mut self) {
        self.cursor.seek_end();
    }

    /// Move the lexer to the specified spot. Any future `advance` calls will
    /// start from this position (the current token is not updated until such a
    /// call). Port of `seek` (JSLexer.h:699-701).
    #[inline]
    pub fn seek(&mut self, loc: SMLoc) {
        self.cursor.seek(loc.offset);
    }

    /// Emit an error at `loc` (Lexer subsystem). If the error limit was reached,
    /// force EOF and return false; otherwise return true. Port of
    /// `JSLexer::error(SMLoc, Twine)` (JSLexer.cpp:2497-2503).
    pub(crate) fn error(&mut self, loc: SMLoc, msg: impl Into<String>) -> bool {
        self.sm.error_at(loc, None, msg.into(), Subsystem::Lexer);
        if !self.sm.is_error_limit_reached() {
            return true;
        }
        self.force_eof();
        false
    }

    /// Emit an error over `range` (Lexer subsystem). Port of
    /// `JSLexer::error(SMRange, Twine)` (JSLexer.cpp:2505-2511).
    pub(crate) fn error_range(&mut self, range: SMRange, msg: impl Into<String>) -> bool {
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

    /// The non-ASCII `default:` arm of `advance` (JSLexer.cpp:711-735,
    /// `default_label`). The cursor is on a non-ASCII byte. Decode the UTF-8
    /// character and either scan a Unicode-only identifier, skip a Unicode-only
    /// space, or report an unrecognized-character error.
    ///
    /// The C++ reaches this via `goto default_label` from the `c2`/`e2`/`ef`
    /// lead-byte arms (when the bytes are not the recognized special sequence)
    /// and from the `default:` case. We extract it into a helper so those arms
    /// can call it (the faithful equivalent of the `goto`).
    ///
    /// \return `true` if the caller should `continue` the advance loop (the C++
    ///   `continue` for a Unicode-only space or after an error), or `false` if
    ///   the token is complete and the loop should `break` (the C++ `break`
    ///   after scanning an identifier).
    fn scan_default_non_ascii(&mut self, grammar_context: GrammarContext) -> bool {
        self.set_token_start();
        let ch = self.decode_utf8_advance();

        if is_unicode_only_id_start(ch) {
            self.tmp_storage.clear();
            append_unicode_to_storage(&mut self.tmp_storage, ch);
            self.scan_identifier_parts_in_context(grammar_context);
            false
        } else if is_unicode_only_space(ch) {
            true
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
            true
        }
    }

    /// Advance to the next token and return it. Port of `JSLexer::advance`
    /// (JSLexer.cpp:255-745).
    pub fn advance(&mut self, grammar_context: GrammarContext) -> &Token {
        self.new_line_before_current_token = false;

        loop {
            // The cursor stays within the buffer (raw() includes the trailing NUL).
            debug_assert!((self.cursor.offset() as usize) < self.cursor.raw().len());
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

                // { {|  (the `{|` form is Flow-only Type context)
                b'{' => {
                    self.set_token_start();
                    if grammar_context == GrammarContext::Type
                        && self.cursor.peek_at(1) == b'|'
                    {
                        self.token.set_punctuator(TokenKind::l_bracepipe);
                        self.cursor.advance(2);
                    } else {
                        self.token.set_punctuator(TokenKind::l_brace);
                        self.cursor.advance(1);
                    }
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
                    if grammar_context == GrammarContext::Type
                        && self.cursor.peek_at(1) == b'}'
                    {
                        self.token.set_punctuator(TokenKind::piper_brace);
                        self.cursor.advance(2);
                    } else if self.cursor.peek_at(1) == b'|' {
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

                // % %=  (the `%checks` form is Flow-only Type context)
                b'%' => {
                    self.set_token_start();
                    let off = self.cursor.offset() as usize;
                    let raw = self.cursor.raw();
                    // `off + 7 < raw.len()` == C++ `curCharPtr_ + 7 <= bufferEnd_`
                    // (raw includes the trailing NUL, so raw.len() - 1 is the NUL).
                    if grammar_context == GrammarContext::Type
                        && off + 7 < raw.len()
                        && &raw[off..off + 7] == b"%checks"
                    {
                        // C++ routes this through getStringLiteral (faithful, though
                        // `%checks` is pure ASCII so convertSurrogates is a no-op).
                        let ident = self.get_string_literal(b"%checks");
                        self.token.set_identifier(ident);
                        self.cursor.advance(7);
                    } else if self.cursor.peek_at(1) == b'=' {
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
                        // C++: goto default_label.
                        if self.scan_default_non_ascii(grammar_context) {
                            continue;
                        }
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
                        // C++: goto default_label.
                        if self.scan_default_non_ascii(grammar_context) {
                            continue;
                        }
                    }
                }

                // Byte-order mark U+FEFF is encoded as: ef bb bf
                0xef => {
                    if self.cursor.peek_at(1) == 0xbb && self.cursor.peek_at(2) == 0xbf {
                        self.cursor.advance(3);
                        continue;
                    } else {
                        // C++: goto default_label.
                        if self.scan_default_non_ascii(grammar_context) {
                            continue;
                        }
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
                        if grammar_context == GrammarContext::AllowRegExp {
                            self.scan_regexp();
                        } else if self.cursor.peek_at(1) == b'=' {
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

                // < <= << <<=  (in Type context, always `less`)
                b'<' => {
                    self.set_token_start();
                    if grammar_context == GrammarContext::Type {
                        self.token.set_punctuator(TokenKind::less);
                        self.cursor.advance(1);
                    } else if self.cursor.peek_at(1) == b'=' {
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

                // > >= >> >>> >>= >>>=  (in Type/JSX context, always `greater`)
                b'>' => {
                    self.set_token_start();
                    if grammar_context == GrammarContext::Type
                        || grammar_context == GrammarContext::AllowJSXIdentifier
                    {
                        self.token.set_punctuator(TokenKind::greater);
                        self.cursor.advance(1);
                    } else if self.cursor.peek_at(1) == b'=' {
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

                // . ... or .NNN (a number).
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

                // @ : decorator punctuator, or (in Flow Type context) the start
                // of an `@`-prefixed Flow identifier.
                b'@' => {
                    self.set_token_start();
                    if grammar_context == GrammarContext::Type {
                        let start = self.cursor.offset();
                        self.scan_identifier_fast_path_in_context(start, grammar_context);
                    } else {
                        self.token.set_punctuator(TokenKind::at);
                        self.cursor.advance(1);
                    }
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
                    self.scan_string_in_context(grammar_context);
                }

                // ` : template literal.
                b'`' => {
                    self.set_token_start();
                    self.scan_template_literal();
                }

                // Default: non-ASCII identifier-start / unicode-only space /
                // unrecognized character. Port of JSLexer.cpp:711-735.
                _ => {
                    if self.scan_default_non_ascii(grammar_context) {
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

    /// Consume a line comment starting from the cursor (which is on `//` or
    /// `#!`) and return the comment offsets `(start, end)` EXCLUDING the line
    /// terminator. Update the cursor to point after the line terminator. Port of
    /// `lineCommentHelper` (JSLexer.cpp:1430-1480).
    fn line_comment_helper(&mut self) -> (u32, u32) {
        debug_assert!(
            (self.cursor.peek() == b'/' && self.cursor.peek_at(1) == b'/')
                || (self.cursor.peek() == b'#' && self.cursor.peek_at(1) == b'!')
        );
        let start = self.cursor.offset();
        // The end of the comment, excluding the line terminator.
        let line_comment_end;
        // Skip the two-character opening delimiter.
        self.cursor.advance(2);

        loop {
            let c = self.cursor.peek();
            match c {
                0 => {
                    if self.cursor.at_end() {
                        line_comment_end = self.cursor.offset();
                        break;
                    } else {
                        self.cursor.advance(1);
                    }
                }
                b'\r' | b'\n' => {
                    line_comment_end = self.cursor.offset();
                    self.cursor.advance(1);
                    self.new_line_before_current_token = true;
                    break;
                }
                UTF8_LINE_TERMINATOR_CHAR0 => {
                    if match_unicode_line_terminator_offset1(
                        &self.cursor.raw()[self.cursor.offset() as usize..],
                    ) {
                        line_comment_end = self.cursor.offset();
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

        (start, line_comment_end)
    }

    /// Consume a line comment starting from the cursor (which is on `//` or
    /// `#!`). Optionally store the comment in comment storage. Update the cursor
    /// to point after the line terminator. Process magic comments. Port of
    /// `scanLineComment` (JSLexer.cpp:1482-1510).
    fn scan_line_comment(&mut self) {
        let first = self.cursor.peek();
        let (comment_start, comment_end) = self.line_comment_helper();

        if self.store_comments {
            // `first == '/'` means a `//` line comment; otherwise `#!` hashbang.
            let kind = if first == b'/' {
                CommentKind::Line
            } else {
                CommentKind::Hashbang
            };
            self.comment_storage.push(StoredComment::new(
                kind,
                SMRange {
                    start: SMLoc {
                        source: self.buf_id,
                        offset: comment_start,
                    },
                    end: SMLoc {
                        source: self.buf_id,
                        offset: comment_end,
                    },
                },
            ));
        }

        // Check for magic comments, which excludes #!.
        // Syntax is //# name=value
        let comment = self
            .cursor
            .slice(comment_start, comment_end)
            .to_vec();
        let Some(rest) = comment.strip_prefix(b"//# ") else {
            return;
        };

        if let Some(value) = rest.strip_prefix(b"sourceURL=") {
            // The comment bytes point into the source buffer (ASCII-ish); store
            // them as a String for the lexer's own accessor and the manager.
            let value = String::from_utf8_lossy(value).into_owned();
            self.sm.set_source_url(self.buf_id, &value);
            self.source_url = Some(value);
        } else if let Some(value) = rest.strip_prefix(b"sourceMappingURL=") {
            let value = String::from_utf8_lossy(value).into_owned();
            self.sm.set_source_mapping_url(self.buf_id, &value);
            self.source_mapping_url = Some(value);
        }
    }

    /// Skip a block comment (`/* ... */`), tracking the newline flag.
    /// Optionally store the comment in comment storage. Port of
    /// `skipBlockComment` (JSLexer.cpp:1512-1571). A non-terminated block comment
    /// reports an error + a "comment started here" note, matching the C++.
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

        if self.store_comments {
            self.comment_storage.push(StoredComment::new(
                CommentKind::Block,
                SMRange {
                    start: block_comment_start,
                    end: self.cur_loc(),
                },
            ));
        }
    }

    /// Decode the UTF-8 sequence at the cursor, advance past it, and report any
    /// decode error at the start of the sequence. Port of the member
    /// `decodeUTF8` (JSLexer.h:1145-1151), which uses `decodeUTF8<false>`.
    pub(crate) fn decode_utf8_advance(&mut self) -> u32 {
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
}

/// \return true if `ch` is an ASCII decimal digit.
#[inline]
pub(crate) fn is_ascii_digit(ch: u8) -> bool {
    ch.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;
    use atom_table::AtomTable;
    use support::manager::SourceErrorManager;

    #[test]
    fn convert_surrogates() {
        // With convert_surrogates ON, an astral char in a string literal is
        // re-encoded to VALID UTF-8 (not the WTF-8 surrogate-pair form).
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "'\\u{1F600}' '\\uD800'");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new_with_convert_surrogates(
            id,
            &mut sm,
            &tab,
            GrammarContext::AllowDiv,
            true,
        );
        let t = lex.advance(GrammarContext::AllowDiv);
        assert_eq!(tab.bytes(t.get_string_literal()), b"\xf0\x9f\x98\x80"); // valid 4-byte UTF-8 emoji
        let t = lex.advance(GrammarContext::AllowDiv);
        assert_eq!(tab.bytes(t.get_string_literal()), "\u{FFFD}".as_bytes()); // lone surrogate -> U+FFFD

        // With it OFF (default), the WTF-8 form is preserved (the existing 2a
        // behavior).
        let mut sm2 = SourceErrorManager::new();
        let id2 = sm2.add_buffer("t2", "'\\u{1F600}'");
        let tab2 = AtomTable::new();
        let mut lex2 =
            JSLexer::new(id2, &mut sm2, &tab2, GrammarContext::AllowDiv);
        let t = lex2.advance(GrammarContext::AllowDiv);
        assert_eq!(
            tab2.bytes(t.get_string_literal()),
            b"\xed\xa0\xbd\xed\xb8\x80"
        ); // WTF-8 surrogate pair
    }

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

    /// Like `kinds`, but lexes under an explicit grammar context (used for the
    /// Flow `Type`-context arms).
    fn kinds_ctx(src: &str, ctx: GrammarContext) -> Vec<TokenKind> {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, ctx);
        let mut out = vec![];
        loop {
            let k = lex.advance(ctx).kind();
            out.push(k);
            if k == TokenKind::eof {
                break;
            }
        }
        out
    }

    #[test]
    fn flow_type_context() {
        use TokenKind::*;
        assert_eq!(kinds_ctx("{|", GrammarContext::Type), vec![l_bracepipe, eof]);
        assert_eq!(kinds_ctx("|}", GrammarContext::Type), vec![piper_brace, eof]);
        // plain `{ }` still works in Type context.
        assert_eq!(
            kinds_ctx("{ }", GrammarContext::Type),
            vec![l_brace, r_brace, eof]
        );
        // `<` is `less` (not lessless etc.) in Type context.
        assert_eq!(kinds_ctx("<", GrammarContext::Type), vec![less, eof]);
        // `>>` lexes as two individual `>` in Type context.
        assert_eq!(
            kinds_ctx(">>", GrammarContext::Type),
            vec![greater, greater, eof]
        );
        // `??` is not formed in Type context (`?` is its own token).
        assert_eq!(
            kinds_ctx("??", GrammarContext::Type),
            vec![question, question, eof]
        );
        // `%checks` is an identifier in Type context.
        assert_eq!(kinds_ctx("%checks", GrammarContext::Type), vec![identifier, eof]);
        // `@`-prefixed Flow identifier.
        assert_eq!(kinds_ctx("@foo", GrammarContext::Type), vec![identifier, eof]);
        // Outside Type, these behave normally:
        assert_eq!(
            kinds_ctx("{|", GrammarContext::AllowDiv),
            vec![l_brace, pipe, eof]
        );
        assert_eq!(
            kinds_ctx("@foo", GrammarContext::AllowDiv),
            vec![at, identifier, eof]
        );
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

    /// Lex `src` as a single template literal token and return its
    /// `(kind, Option<cooked> bytes, raw bytes)`.
    fn template(src: &str) -> (TokenKind, Option<Vec<u8>>, Vec<u8>) {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        let tok = lex.advance(GrammarContext::AllowDiv);
        assert!(tok.is_template_literal(), "src={src:?} kind={:?}", tok.kind());
        let kind = tok.kind();
        let cooked = tok.get_template_value().map(|a| tab.bytes(a).to_vec());
        let raw = tab.bytes(tok.get_template_raw_value()).to_vec();
        (kind, cooked, raw)
    }

    #[test]
    fn templates_basic() {
        use TokenKind::*;
        // `abc` -> no_substitution_template, cooked="abc" raw="abc"
        assert_eq!(
            template("`abc`"),
            (no_substitution_template, Some(b"abc".to_vec()), b"abc".to_vec())
        );
        // `a${ -> template_head
        assert_eq!(template("`a${").0, template_head);
        // escapes: cooked has the cooked value, raw has the literal backslash seq
        assert_eq!(
            template("`a\\nb`"),
            (no_substitution_template, Some(vec![b'a', 10, b'b']), b"a\\nb".to_vec())
        );
        // NotEscapeSequence (\9) -> cooked is None, raw keeps it
        assert_eq!(
            template("`\\9`"),
            (no_substitution_template, None, b"\\9".to_vec())
        );
        // CR -> LF normalization in cooked AND raw
        assert_eq!(
            template("`a\rb`"),
            (no_substitution_template, Some(vec![b'a', 10, b'b']), vec![b'a', 10, b'b'])
        );
        // kind sequence: `a${ b } -> template_head, identifier, r_brace (rescan is
        // parser-driven, so the trailing ` starts a new — non-terminated — scan).
        assert_eq!(
            kinds("`a${b}")[..3].to_vec(),
            vec![template_head, identifier, r_brace]
        );
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
    fn token_storage() {
        // With store_tokens on, every advanced token (kind+range) is recorded.
        // finishToken stores the token it just finished, including the final
        // `eof` (advance scans eof, then finishToken records it). This matches
        // the C++ faithfully.
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "a + b");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.set_store_tokens(true);
        assert!(lex.get_store_tokens());
        while lex.advance(GrammarContext::AllowDiv).kind() != TokenKind::eof {}
        let toks: Vec<TokenKind> =
            lex.get_stored_tokens().iter().map(|t| t.kind()).collect();
        assert_eq!(
            toks,
            vec![
                TokenKind::identifier,
                TokenKind::plus,
                TokenKind::identifier,
                TokenKind::eof
            ]
        );
    }

    #[test]
    fn comment_storage() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "a /*c*/ // line\nb");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.set_store_comments(true);
        while lex.advance(GrammarContext::AllowDiv).kind() != TokenKind::eof {}
        // Capture the comment ranges before re-borrowing the buffer for slicing.
        let cs: Vec<(CommentKind, u32, u32)> = lex
            .get_stored_comments()
            .iter()
            .map(|c| {
                let r = c.source_range();
                (c.kind(), r.start.offset, r.end.offset)
            })
            .collect();
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0].0, CommentKind::Block);
        assert_eq!(cs[1].0, CommentKind::Line);
        // The stored range includes the delimiters (getString strips them).
        let buf = sm.source_buffer(id);
        let raw = buf.raw();
        assert_eq!(&raw[cs[0].1 as usize..cs[0].2 as usize], b"/*c*/");
        assert_eq!(&raw[cs[1].1 as usize..cs[1].2 as usize], b"// line");
    }

    #[test]
    fn magic_comments() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer(
            "t",
            "a\n//# sourceURL=http://x/y.js\n//# sourceMappingURL=z.map\nb",
        );
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        while lex.advance(GrammarContext::AllowDiv).kind() != TokenKind::eof {}
        assert_eq!(lex.get_source_url(), Some("http://x/y.js"));
        assert_eq!(lex.get_source_mapping_url(), Some("z.map"));
    }

    #[test]
    fn save_point_restore() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "a . b");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.advance(GrammarContext::AllowDiv); // 'a' (identifier)
        let sp = lex.save_point();
        lex.advance(GrammarContext::AllowDiv); // '.'
        lex.advance(GrammarContext::AllowDiv); // 'b'
        sp.restore(&mut lex);
        // Current token is back to 'a'; next advance gives '.'.
        assert_eq!(lex.token().kind(), TokenKind::identifier);
        assert_eq!(
            lex.advance(GrammarContext::AllowDiv).kind(),
            TokenKind::period
        );
    }

    #[test]
    fn save_point_truncates_storage() {
        // SavePoint restore truncates comment + token storage to the saved size.
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "a /*c*/ . b");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.set_store_tokens(true);
        lex.set_store_comments(true);
        lex.advance(GrammarContext::AllowDiv); // 'a'
        let toks_before = lex.get_stored_tokens().len();
        let comments_before = lex.get_stored_comments().len();
        let sp = lex.save_point();
        lex.advance(GrammarContext::AllowDiv); // '.', skips the /*c*/ comment
        lex.advance(GrammarContext::AllowDiv); // 'b'
        assert!(lex.get_stored_tokens().len() > toks_before);
        assert!(lex.get_stored_comments().len() > comments_before);
        sp.restore(&mut lex);
        assert_eq!(lex.get_stored_tokens().len(), toks_before);
        assert_eq!(lex.get_stored_comments().len(), comments_before);
    }

    #[test]
    fn is_directive() {
        fn directive(src: &str) -> bool {
            let mut sm = SourceErrorManager::new();
            let id = sm.add_buffer("t", src);
            let tab = AtomTable::new();
            let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
            lex.advance(GrammarContext::AllowDiv); // the string literal
            lex.is_current_token_a_directive()
        }
        assert!(directive("\"use strict\";"));
        assert!(directive("\"use strict\"\n"));
        assert!(directive("\"x\" /*c*/ ;"));
        assert!(directive("\"x\"")); // eof
        assert!(directive("\"x\" // line")); // line comment implies newline
        assert!(directive("\"x\" }")); // right brace
        assert!(!directive("\"x\" + y")); // followed by an operator
        assert!(!directive("foo")); // not a string literal
    }

    #[test]
    fn is_directive_does_not_corrupt() {
        // After is_current_token_a_directive, the next advance is unaffected.
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "\"x\" /*c*/ + y");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        assert_eq!(
            lex.advance(GrammarContext::AllowDiv).kind(),
            TokenKind::string_literal
        );
        assert!(!lex.is_current_token_a_directive());
        // The block comment is normally skipped; the next token is '+'.
        assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::plus);
        assert_eq!(
            lex.advance(GrammarContext::AllowDiv).kind(),
            TokenKind::identifier
        );
    }

    #[test]
    fn rescan_rbrace_template() {
        use TokenKind::*;
        // `a${b}c` : template_head, identifier(b), r_brace, then rescan ->
        // template_tail cooked="c".
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "`a${b}c`");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), template_head);
        assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), identifier);
        assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), r_brace);
        let tok = lex.rescan_rbrace_in_template_literal();
        assert_eq!(tok.kind(), template_tail);
        let cooked = tok.get_template_value().map(|a| tab.bytes(a).to_vec());
        assert_eq!(cooked, Some(b"c".to_vec()));
    }

    #[test]
    fn rescan_rbrace_template_middle() {
        use TokenKind::*;
        // `a${b}c${d}e` : the first rescan yields template_middle.
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "`a${b}c${d}e`");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), template_head);
        assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), identifier);
        assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), r_brace);
        assert_eq!(
            lex.rescan_rbrace_in_template_literal().kind(),
            template_middle
        );
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

    /// Regression for the `scan_not_implemented` bug: a `c2`-led Unicode-only
    /// id-start (`ª` U+00AA = `c2 aa`) used to be routed to a "not yet
    /// implemented" stub that errored and forced EOF. It must now fall through
    /// to the default non-ASCII arm and lex as an identifier.
    #[test]
    fn unicode_only_id_start_via_c2_arm() {
        use TokenKind::*;
        assert_eq!(kinds("\u{00aa}"), vec![identifier, eof]);
    }

    #[test]
    fn check_following_character() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "renders?");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        // After scanning `renders`, the next character is `?`.
        assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::identifier);
        assert!(lex.check_following_character(b'?'));
        assert!(!lex.check_following_character(b':'));
    }

    #[test]
    fn token_input_str_returns_source_text() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "  foobar  ");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::identifier);
        assert_eq!(lex.token_input_str(), b"foobar");
    }

    #[test]
    fn convert_cur_token_to_ident_op_for_as() {
        use crate::token_kinds::token_kind_str;
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "as");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        // `as` lexes as a plain identifier.
        assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::identifier);
        // Make sure the IDENT_OP kind we convert to actually has str == "as".
        assert_eq!(token_kind_str(TokenKind::as_operator), "as");
        lex.convert_cur_token_to_ident_op(TokenKind::as_operator);
        assert_eq!(lex.token().kind(), TokenKind::as_operator);
    }

    #[test]
    fn get_identifier_and_buffer_id() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "abc");
        let tab = AtomTable::new();
        let lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        // get_identifier interns into the shared table.
        let a = lex.get_identifier(b"hello");
        assert_eq!(tab.bytes(a), b"hello");
        // get_buffer_id returns the buffer we're lexing.
        assert_eq!(lex.get_buffer_id(), id);
        // buffer bytes exclude the trailing NUL sentinel.
        assert_eq!(lex.buffer_bytes(), b"abc");
        assert_eq!(lex.get_buffer_start(), 0);
        assert_eq!(lex.get_buffer_end(), 3);
    }

    #[test]
    fn source_mgr_mut_reports_errors() {
        // Mirror the exact setup pattern used by other tests in this module:
        // SourceErrorManager::new(), sm.add_buffer, AtomTable::new(), JSLexer::new.
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "x");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        let loc = lex.token().start_loc();
        lex.get_source_mgr_mut()
            .error_at(loc, None, "boom", Subsystem::Parser);
        assert_eq!(lex.get_source_mgr().error_count(), 1);
    }
}
