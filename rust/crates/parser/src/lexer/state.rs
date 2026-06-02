//! Self-contained lexer-state surface: the `unsafeSet*` helpers and `SavePoint`
//! (save/restore for backtracking). `isCurrentTokenADirective` and
//! `rescanRBraceInTemplateLiteral` are added in the next task.
//!
//! These `impl<'a> JSLexer<'a>` methods live in a child module of `lexer`, so
//! they can access the private fields of `JSLexer` declared in `lexer/mod.rs`.
//!
//! Ported from `include/hermes/Parser/JSLexer.h` (`SavePoint`, the `unsafeSet*`
//! helpers).

use atom_table::AtomBytes;
use support::location::{SMLoc, SMRange};

use crate::token_kinds::TokenKind;

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
