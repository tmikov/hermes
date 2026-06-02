//! Parser-facing lookahead helpers for the JS lexer.
//!
//! These `impl<'a> JSLexer<'a>` methods live in a child module of `lexer`, so
//! they can access the private fields of `JSLexer` declared in `lexer/mod.rs`.
//!
//! Ported from `lib/Parser/JSLexer.cpp`:
//! - `optimisticSkipWhitespace` (`:117-132`)
//! - `lookahead1` (`:1038-1095`)
//! - `lookahead2` (`:1100-1154`)
//!
//! The C++ `template <bool RequireNoNewLine>` becomes a runtime
//! `require_no_newline: bool` parameter; the parser `Keywords` dependency is
//! replaced by passing the needed pre-interned atom (`ident_using`). The C++
//! `make_scope_exit` restore + `SaveAndSuppressMessages` become explicit
//! save/restore.

use atom_table::AtomBytes;
use support::diag::Subsystem;

use crate::token_kinds::TokenKind;

use super::{GrammarContext, JSLexer};

impl<'a> JSLexer<'a> {
    /// Skip ` `/`\t`/`\v`/`\f` from the cursor (advancing it) and return the
    /// next non-whitespace byte (or `\0` at EOF). Does NOT skip newlines or
    /// comments. Port of `optimisticSkipWhitespace` (JSLexer.cpp:117-132).
    pub(crate) fn optimistic_skip_whitespace(&mut self) -> u8 {
        loop {
            let cur = self.cursor.peek();
            if cur == 0 {
                return 0;
            }
            match cur {
                b' ' | b'\t' | 0x0b | 0x0c => {
                    self.cursor.advance(1);
                    continue;
                }
                _ => return cur,
            }
        }
    }

    /// Look ahead one token, restoring the lexer state afterwards (unless the
    /// next token matches `expected`, in which case the lookahead is consumed).
    /// Port of `lookahead1` (JSLexer.cpp:1038-1095).
    ///
    /// The C++ `template <bool RequireNoNewLine>` becomes `require_no_newline`.
    pub fn lookahead1(
        &mut self,
        require_no_newline: bool,
        expected: Option<TokenKind>,
    ) -> Option<TokenKind> {
        // We support TokenKind::question here because of Flow's render types.
        // `renders?` is not a token itself (as making it a token would be bad
        // for identifier parsing performance). When we are parsing something
        // like (renders?: number) => string and the cursor is under the `?`, we
        // need to perform a lookahead to see if the next token is a colon, in
        // which case this is a function parameter, and if not then parse as a
        // render type.
        debug_assert!(
            self.token.kind() == TokenKind::identifier
                || self.token.is_res_word()
                || self.token.kind() == TokenKind::question,
            "unsupported current token"
        );
        let saved_kind = self.token.kind();
        let saved_ident: Option<AtomBytes> = if saved_kind
            == TokenKind::identifier
            || self.token.is_res_word()
        {
            Some(self.token.get_res_word_or_identifier())
        } else {
            None
        };
        let start = self.token.start_loc();
        let end = self.token.end_loc();
        let cur = self.cur_loc();
        let saved_suppressed = self.sm.suppressed_messages();
        self.sm.set_suppressed_messages(Some(Subsystem::Unspecified));

        // Remove any comments that were stored during the lookahead.
        let saved_comment_storage_size = self.comment_storage.len();

        self.advance(GrammarContext::AllowRegExp);
        let mut kind = Some(self.token.kind());
        if require_no_newline && self.is_new_line_before_current_token() {
            // Disregard anything after LineTerminator.
            kind = None;
        } else if expected == kind {
            // Do not move the cursor back.
            // NOTE: the C++ returns here, leaving messages suppressed via the
            // RAII guard; we restore the suppression state before returning.
            self.sm.set_suppressed_messages(saved_suppressed);
            return kind;
        }

        self.token.set_start(start);
        self.token.set_end(end);
        if saved_kind == TokenKind::identifier {
            self.token.set_identifier(saved_ident.unwrap());
        } else if saved_kind == TokenKind::question {
            self.token.set_punctuator(TokenKind::question);
        } else {
            self.token.set_res_word(saved_kind, saved_ident.unwrap());
        }
        self.seek(cur);

        // Undo the storage for the token we just advanced to.
        if self.store_tokens {
            self.token_storage.pop();
        }
        if self.store_comments {
            self.comment_storage.truncate(saved_comment_storage_size);
        }

        self.sm.set_suppressed_messages(saved_suppressed);
        kind
    }

    /// Look ahead two tokens: if the next token is `expected_ident`, return the
    /// kind of the token after it; otherwise `None`. ALWAYS restores the lexer
    /// state. Port of `lookahead2` (JSLexer.cpp:1100-1154).
    ///
    /// The C++ `template <bool RequireNoNewLine>` becomes `require_no_newline`;
    /// the C++ single `make_scope_exit` that always restores becomes a computed
    /// result followed by an unconditional restore.
    pub fn lookahead2(
        &mut self,
        require_no_newline: bool,
        expected_ident: AtomBytes,
    ) -> Option<TokenKind> {
        debug_assert!(
            self.token.kind() == TokenKind::identifier
                || self.token.is_res_word(),
            "unsupported current token"
        );
        let saved_ident = self.token.get_res_word_or_identifier();
        let saved_kind = self.token.kind();
        let start = self.token.start_loc();
        let end = self.token.end_loc();
        let cur = self.cur_loc();
        let saved_suppressed = self.sm.suppressed_messages();
        self.sm.set_suppressed_messages(Some(Subsystem::Unspecified));

        // Remove any comments/tokens that were stored during the lookahead.
        let saved_comment_storage_size = self.comment_storage.len();
        let saved_token_storage_size =
            if self.store_tokens { self.token_storage.len() } else { 0 };

        // Compute the result; the C++ scope_exit restores unconditionally, so
        // we capture the result and restore at the end of the function.
        let result = self.lookahead2_impl(require_no_newline, expected_ident);

        // Restore (mirror of the C++ `make_scope_exit`).
        if self.store_comments {
            self.comment_storage.truncate(saved_comment_storage_size);
        }
        // Undo the storage for the tokens we advanced to.
        if self.store_tokens {
            self.token_storage.truncate(saved_token_storage_size);
        }
        // Restore the original token.
        self.token.set_start(start);
        self.token.set_end(end);
        if saved_kind == TokenKind::identifier {
            self.token.set_identifier(saved_ident);
        } else {
            self.token.set_res_word(saved_kind, saved_ident);
        }
        self.seek(cur);

        self.sm.set_suppressed_messages(saved_suppressed);
        result
    }

    /// The body of `lookahead2` that advances past two tokens and computes the
    /// result; the surrounding `lookahead2` performs the unconditional restore.
    fn lookahead2_impl(
        &mut self,
        require_no_newline: bool,
        expected_ident: AtomBytes,
    ) -> Option<TokenKind> {
        self.advance(GrammarContext::AllowRegExp);
        if require_no_newline && self.is_new_line_before_current_token() {
            return None;
        }

        // If the next token isn't the expected identifier, bail.
        if self.token.kind() != TokenKind::identifier
            || self.token.get_identifier() != expected_ident
        {
            return None;
        }

        // Advance to the token we're looking ahead to.
        self.advance(GrammarContext::AllowRegExp);
        if require_no_newline && self.is_new_line_before_current_token() {
            return None;
        }

        Some(self.token.kind())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atom_table::AtomTable;
    use support::manager::SourceErrorManager;

    #[test]
    fn lookahead1_basic() {
        // current token must be identifier/resword/question. lookahead1 peeks
        // the next token and restores state unless it matches `expected`.
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "async function");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.advance(GrammarContext::AllowDiv); // 'async' (identifier)
                                               // peek: next is 'function' (rw_function), no newline.
        assert_eq!(lex.lookahead1(true, None), Some(TokenKind::rw_function));
        // state restored: current token still 'async', next advance is 'function'
        assert_eq!(lex.token().kind(), TokenKind::identifier);
        assert_eq!(
            lex.advance(GrammarContext::AllowDiv).kind(),
            TokenKind::rw_function
        );
    }

    #[test]
    fn lookahead1_newline_and_expected() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "async\nx");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.advance(GrammarContext::AllowDiv); // 'async'
                                               // RequireNoNewLine=true and there IS a newline -> None
        assert_eq!(lex.lookahead1(true, None), None);

        // when expectedToken matches, the cursor is NOT moved back (consumes
        // the lookahead):
        let mut sm2 = SourceErrorManager::new();
        let id2 = sm2.add_buffer("t2", "a b");
        let tab2 = AtomTable::new();
        let mut lex2 =
            JSLexer::new(id2, &mut sm2, &tab2, GrammarContext::AllowDiv);
        lex2.advance(GrammarContext::AllowDiv); // 'a'
        assert_eq!(
            lex2.lookahead1(true, Some(TokenKind::identifier)),
            Some(TokenKind::identifier)
        );
        assert_eq!(lex2.token().kind(), TokenKind::identifier); // now 'b' (consumed)
    }

    #[test]
    fn lookahead2_basic() {
        // lookahead2(expected_ident): skip the next token IF it's
        // `expected_ident`, return the kind of the token after it. Always
        // restores state.
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "await using x");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.advance(GrammarContext::AllowDiv); // 'await'
        let using = tab.atom_bytes(b"using");
        // next is 'using' (matches), the one after is 'x' (identifier).
        assert_eq!(lex.lookahead2(true, using), Some(TokenKind::identifier));
        // state restored to 'await'
        assert_eq!(lex.token().kind(), TokenKind::identifier);
        assert_eq!(
            lex.token().get_res_word_or_identifier(),
            tab.atom_bytes(b"await")
        );
    }
}
