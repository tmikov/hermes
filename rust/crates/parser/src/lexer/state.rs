//! Self-contained lexer-state surface: the `unsafeSet*` helpers, `SavePoint`
//! (save/restore for backtracking), `isCurrentTokenADirective`, and
//! `rescanRBraceInTemplateLiteral`.
//!
//! These `impl<'a> JSLexer<'a>` methods live in a child module of `lexer`, so
//! they can access the private fields of `JSLexer` declared in `lexer/mod.rs`.
//!
//! Ported from `include/hermes/Parser/JSLexer.h` (`SavePoint`, the `unsafeSet*`
//! helpers) and `lib/Parser/JSLexer.cpp:911-1035`.

use hermes_atom_table::AtomBytes;
use hermes_support::diag::Subsystem;
use hermes_support::location::{SMLoc, SMRange};

use crate::token_kinds::TokenKind;
use crate::utf8::{
    is_utf8_start, match_unicode_line_terminator_offset1,
    UTF8_LINE_TERMINATOR_CHAR0,
};
use hermes_unicode::is_unicode_only_space;

use super::JSLexer;

impl<'a> JSLexer<'a> {
    /// Set the end location of the previous token. Port of
    /// `setPrevTokenEndLoc`.
    pub fn set_prev_token_end_loc(&mut self, loc: SMLoc) {
        self.prev_token_end = loc;
    }

    /// Set the current token kind to `kind` without any checks and seek to
    /// `loc`. Should only be used for save-point use-cases. Port of
    /// `unsafeSetPunctuator` (JSLexer.h:1049-1054). (The C++ name says "unsafe"
    /// but the Rust port involves no `unsafe` keyword — all `unsafe` lives in
    /// `cursor.rs`.)
    pub(crate) fn unsafe_set_punctuator(
        &mut self,
        kind: TokenKind,
        loc: SMLoc,
        range: SMRange,
    ) {
        debug_assert!(kind.is_punctuator(), "must set a punctuator");
        self.token.set_punctuator(kind);
        self.token.set_range(range);
        self.seek(loc);
    }

    /// Set the current token to an identifier without any checks and seek to
    /// `loc`. Should only be used for save-point use-cases. Port of
    /// `unsafeSetIdentifier` (JSLexer.h:1059-1063).
    pub(crate) fn unsafe_set_identifier(
        &mut self,
        ident: AtomBytes,
        loc: SMLoc,
        range: SMRange,
    ) {
        self.token.set_identifier(ident);
        self.token.set_range(range);
        self.seek(loc);
    }

    /// Set the current token to a reserved word without any checks and seek to
    /// `loc`. Should only be used for save-point use-cases. Port of
    /// `unsafeSetReservedWord` (JSLexer.h:1068-1072).
    pub(crate) fn unsafe_set_reserved_word(
        &mut self,
        kind: TokenKind,
        loc: SMLoc,
        range: SMRange,
    ) {
        let ident = self.res_word_ident(kind);
        self.token.set_res_word(kind, ident);
        self.token.set_range(range);
        self.seek(loc);
    }

    /// Store state of the lexer and allow rescanning from that point. Port of
    /// `JSLexer::SavePoint::SavePoint` (JSLexer.h:778-794). Can only save state
    /// when the current token is a punctuator, `identifier`, or `rw_extends`.
    ///
    /// DEVIATION: the C++ `SavePoint` is an RAII-style object holding a
    /// `JSLexer *`. Rust cannot hold a `&mut JSLexer` across an `advance` call
    /// (which also needs `&mut JSLexer`), so we model it as a plain value
    /// snapshot whose `restore(&mut JSLexer)` re-applies the saved state.
    pub fn save_point(&self) -> SavePoint {
        let kind = self.token.kind();
        debug_assert!(
            kind.is_punctuator()
                || kind == TokenKind::identifier
                || kind == TokenKind::rw_extends,
            "SavePoint can only be used for punctuators, identifier or `extends` keyword"
        );
        SavePoint {
            kind,
            // Saved identifier, None if kind != identifier.
            ident: if kind == TokenKind::identifier {
                Some(self.token.get_identifier())
            } else {
                None
            },
            loc: self.cur_loc(),
            range: self.token.source_range(),
            prev_token_end: self.prev_token_end,
            comment_storage_size: self.get_stored_comments().len(),
            token_storage_size: self.get_stored_tokens().len(),
        }
    }

    /// Check whether the current token is a directive, in other words is it a
    /// string literal without escapes or new line continuations, followed by
    /// either new line, semicolon or right brace. This doesn't move the input
    /// pointer, so the optional semicolon, brace or the new line will be
    /// consumed normally by the next `advance` call. Port of
    /// `isCurrentTokenADirective` (JSLexer.cpp:911-1021).
    ///
    /// \return true if the token can be interpreted as a directive.
    pub fn is_current_token_a_directive(&mut self) -> bool {
        if self.token.kind() != TokenKind::string_literal {
            return false;
        }

        // A directive is a string literal (the current token, directly behind
        // the cursor), followed by a semicolon, new line, or eof that we will
        // now try to find. There can also be comments. So, we loop, consuming
        // whitespace until we encounter:
        // - EOF. Don't consume it and succeed.
        // - Semicolon. Don't consume it and succeed.
        // - Right brace. Don't consume it and succeed.
        // - A new line. Don't consume it and succeed.
        // - A line comment. It implies a new line. Don't consume it and succeed.
        // - A block comment. Consume it and continue.
        // - Anything else. We consume nothing and fail.
        //
        // DEVIATION: the C++ scans with a local `ptr` for the simple cases and
        // calls `skipBlockComment(ptr)` (which returns the new ptr) only for
        // block comments. Our `skip_block_comment` mutates the cursor + newline
        // flag, so we scan from a local offset, only moving the real cursor for
        // the block-comment case, and restore the cursor offset (and the newline
        // flag, which the block-comment scan may set) before returning so the
        // caller's next `advance` starts where it left off.
        let saved_offset = self.cursor.offset();
        let saved_newline = self.new_line_before_current_token;
        // Clone the buffer Rc so the byte view does not borrow `self`; the
        // block-comment arm needs `&mut self`, and the buffer bytes are stable.
        let buffer = self.cursor.buffer().clone();
        let raw = buffer.raw();
        let mut ptr = saved_offset as usize;

        let result = loop {
            debug_assert!(
                ptr < raw.len(),
                "lexing past end of input"
            );

            match raw[ptr] {
                0 => {
                    // EOF? (the trailing NUL is at index raw.len() - 1)
                    if ptr == raw.len() - 1 {
                        break true;
                    }
                    // We encountered a stray 0 character.
                    break false;
                }

                b';' | b'}' => break true,

                b'\r' | b'\n' => break true,

                // Line separator   UTF8 encoded is      : e2 80 a8
                // Paragraph separator   UTF8 encoded is : e2 80 a9
                UTF8_LINE_TERMINATOR_CHAR0 => {
                    if match_unicode_line_terminator_offset1(&raw[ptr..]) {
                        break true;
                    }
                    break false;
                }

                // \v \f : skip whitespace.
                0x0b | 0x0c => {
                    ptr += 1;
                    continue;
                }

                // \t and space: spaces frequently come in groups, so use a
                // tight inner loop to skip.
                b'\t' | b' ' => {
                    loop {
                        ptr += 1;
                        if raw[ptr] != b'\t' && raw[ptr] != b' ' {
                            break;
                        }
                    }
                    continue;
                }

                // No-break space   is UTF8 encoded as: c2 a0
                0xc2 => {
                    if raw[ptr + 1] == 0xa0 {
                        ptr += 2;
                        continue;
                    } else {
                        // Fall through to the default (unicode-space) handling.
                        if let Some(next) = directive_unicode_space(raw, ptr) {
                            ptr = next;
                            continue;
                        }
                        break false;
                    }
                }

                // Byte-order mark ﻿ is encoded as: ef bb bf
                0xef => {
                    if raw[ptr + 1] == 0xbb && raw[ptr + 2] == 0xbf {
                        ptr += 3;
                        continue;
                    } else {
                        if let Some(next) = directive_unicode_space(raw, ptr) {
                            ptr = next;
                            continue;
                        }
                        break false;
                    }
                }

                b'/' => {
                    if raw[ptr + 1] == b'/' {
                        // Line comment? It implies a new line, so we are good.
                        break true;
                    } else if raw[ptr + 1] == b'*' {
                        // Block comment. Consume it (with messages suppressed
                        // and comment storage saved/restored) and continue.
                        let saved_comment_len = self.comment_storage.len();
                        let saved_suppressed = self.sm.suppressed_messages();
                        self.sm
                            .set_suppressed_messages(Some(Subsystem::Unspecified));
                        // Drive `skip_block_comment` from `ptr`; it mutates the
                        // cursor (and may set the newline flag), so seek there
                        // first and read the new offset back into `ptr`.
                        self.cursor.seek(ptr as u32);
                        self.skip_block_comment();
                        ptr = self.cursor.offset() as usize;
                        self.sm.set_suppressed_messages(saved_suppressed);
                        if self.store_comments {
                            self.comment_storage.truncate(saved_comment_len);
                        }
                        // Re-borrow `raw` after the mutable calls above by
                        // looping; the buffer bytes are stable.
                        continue;
                    } else {
                        break false;
                    }
                }

                // Handle all other characters: if it is a unicode space, skip
                // it. Otherwise we have failed.
                _ => {
                    if let Some(next) = directive_unicode_space(raw, ptr) {
                        ptr = next;
                        continue;
                    }
                    break false;
                }
            }
        };

        // Restore the cursor and the newline flag so the caller is unaffected.
        self.cursor.seek(saved_offset);
        self.new_line_before_current_token = saved_newline;
        result
    }

    /// Rescan the `}` token as a TemplateMiddle or TemplateTail. Should be
    /// called in the middle of parsing a template literal. Port of
    /// `rescanRBraceInTemplateLiteral` (JSLexer.cpp:1023-1035).
    pub fn rescan_rbrace_in_template_literal(&mut self) -> &crate::token::Token {
        debug_assert!(
            self.token.kind() == TokenKind::r_brace,
            "need }} to rescan"
        );
        // Back the cursor up one, to the `}`.
        let back = self.cursor.offset() - 1;
        self.cursor.seek(back);
        // Undo the storage for the '}'.
        if self.store_tokens {
            self.token_storage.pop();
        }
        debug_assert!(
            self.cursor.peek() == b'}',
            "non-}} was scanned as r_brace"
        );
        // Set the token start to the `}` and scan the `}`-start template path.
        let start = self.cur_loc();
        self.token.set_start(start);
        self.scan_template_literal();
        self.finish_token();
        &self.token
    }
}

/// Store state of the lexer and allow rescanning from that point. Port of the
/// C++ `JSLexer::SavePoint` (JSLexer.h:751-821).
///
/// DEVIATION: the C++ `SavePoint` holds a `JSLexer *` and exposes `restore()`.
/// In Rust a save point cannot hold a `&mut JSLexer` across an intervening
/// `advance` (which also borrows the lexer mutably), so it is a plain value
/// snapshot and `restore` takes `&mut JSLexer`.
pub struct SavePoint {
    /// Saved token kind: a punctuator, `identifier`, or `rw_extends`.
    kind: TokenKind,

    /// Saved identifier, None if `kind != identifier`.
    ident: Option<AtomBytes>,

    /// Saved cursor location (port of `loc_`, i.e. `curCharPtr_`).
    loc: SMLoc,

    /// Saved token range from the lexer.
    range: SMRange,

    /// Saved previous token end location from the lexer.
    prev_token_end: SMLoc,

    /// Saved size of comment storage within the lexer. If we restore this save
    /// point, comments past this index should be removed from the lexer.
    comment_storage_size: usize,

    /// Stored token storage size. If we backtrack, we must also delete the
    /// previously stored tokens.
    token_storage_size: usize,
}

impl SavePoint {
    /// Restore the state of `lexer` to the originally saved state. Port of
    /// `JSLexer::SavePoint::restore` (JSLexer.h:797-820).
    pub fn restore(self, lexer: &mut JSLexer) {
        if self.kind == TokenKind::identifier {
            lexer.unsafe_set_identifier(self.ident.unwrap(), self.loc, self.range);
        } else if self.kind == TokenKind::rw_extends {
            lexer.unsafe_set_reserved_word(self.kind, self.loc, self.range);
        } else {
            lexer.unsafe_set_punctuator(self.kind, self.loc, self.range);
        }

        lexer.prev_token_end = self.prev_token_end;

        // Deliberately mirror C++: tokens are gated on `getStoreTokens()`
        // while comments are gated on the `storeComments_` field directly.
        if lexer.store_comments
            && self.comment_storage_size < lexer.comment_storage.len()
        {
            lexer.comment_storage.truncate(self.comment_storage_size);
        }

        if lexer.get_store_tokens() {
            lexer.token_storage.truncate(self.token_storage_size);
        }
    }
}

/// If the byte sequence at `raw[ptr..]` begins a unicode-only space, return the
/// offset just past it; otherwise `None`. Mirrors the `default` arm of
/// `isCurrentTokenADirective` (JSLexer.cpp:1008-1018): only multi-byte UTF-8
/// starts are considered (ASCII spaces are handled by their own arms).
fn directive_unicode_space(raw: &[u8], ptr: usize) -> Option<usize> {
    if is_utf8_start(raw[ptr]) {
        let mut i = ptr;
        let cp = crate::utf8::decode_utf8::<false>(raw, &mut i, |_| {});
        if is_unicode_only_space(cp) {
            return Some(i);
        }
    }
    None
}
