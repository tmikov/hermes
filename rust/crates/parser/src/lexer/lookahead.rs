//! Parser-facing lookahead helpers for the JS lexer.
//!
//! These `impl<'a> JSLexer<'a>` methods live in a child module of `lexer`, so
//! they can access the private fields of `JSLexer` declared in `lexer/mod.rs`.
//!
//! Ported from `lib/Parser/JSLexer.cpp`:
//! - `optimisticSkipWhitespace` (`:117-132`)
//! - `lookahead1` (`:1038-1095`)
//! - `lookahead2` (`:1100-1154`)
//! - `isLetFollowedByDeclStart` (`:134-176`)
//! - `isUsingFollowedByIdentifier` (`:178-204`)
//! - `isAwaitUsingFollowedByIdentifier` (`:206-253`)
//!
//! The C++ `template <bool RequireNoNewLine>` is preserved as the const generic
//! `REQUIRE_NO_NEWLINE` (so each specialization monomorphizes like the C++
//! template); the parser `Keywords` dependency is replaced by passing the needed
//! pre-interned atom (`ident_using`). The C++ `make_scope_exit` restore +
//! `SaveAndSuppressMessages` become explicit save/restore.

use hermes_atom_table::AtomBytes;
use hermes_support::diag::Subsystem;

use hermes_unicode::{is_ascii_identifier_continue, is_ascii_identifier_start};

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
    /// The C++ `template <bool RequireNoNewLine>` is the const generic
    /// `REQUIRE_NO_NEWLINE`.
    pub fn lookahead1<const REQUIRE_NO_NEWLINE: bool>(
        &mut self,
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
        if REQUIRE_NO_NEWLINE && self.is_new_line_before_current_token() {
            // Disregard anything after LineTerminator.
            kind = None;
        } else if expected == kind {
            // Do not move the cursor back.
            // NOTE: the C++ `make_scope_exit` still fires on this early return, so
            // it truncates comment storage here too (and we restore the suppression
            // state the RAII guard would have restored).
            if self.store_comments {
                self.comment_storage.truncate(saved_comment_storage_size);
            }
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
    /// The C++ `template <bool RequireNoNewLine>` is the const generic
    /// `REQUIRE_NO_NEWLINE`; the C++ single `make_scope_exit` that always
    /// restores becomes a computed result followed by an unconditional restore.
    pub fn lookahead2<const REQUIRE_NO_NEWLINE: bool>(
        &mut self,
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
        let result = self.lookahead2_impl::<REQUIRE_NO_NEWLINE>(expected_ident);

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
    fn lookahead2_impl<const REQUIRE_NO_NEWLINE: bool>(
        &mut self,
        expected_ident: AtomBytes,
    ) -> Option<TokenKind> {
        self.advance(GrammarContext::AllowRegExp);
        if REQUIRE_NO_NEWLINE && self.is_new_line_before_current_token() {
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
        if REQUIRE_NO_NEWLINE && self.is_new_line_before_current_token() {
            return None;
        }

        Some(self.token.kind())
    }

    /// \return true if the `let` keyword (the current token) is followed by the
    /// start of a declaration. Port of `isLetFollowedByDeclStart`
    /// (JSLexer.cpp:134-176).
    pub fn is_let_followed_by_decl_start(&mut self) -> bool {
        debug_assert!(
            self.token.kind() == TokenKind::identifier
                && self.strtab.bytes(self.token.get_identifier()) == b"let",
            "current token must be the `let` identifier"
        );

        // Unlike `is_using_*`, this does NOT save/restore the cursor around the
        // whitespace skip (matching the C++): the fast paths only peek, and the
        // slow path delegates to `lookahead1`, which restores the cursor itself.
        let cur_char = self.optimistic_skip_whitespace();

        // Fast path.
        // If the next character is a '{', then this is a let declaration.
        // If the next character is a '[', then this is a let declaration.
        if cur_char == b'{' || cur_char == b'[' {
            return true;
        }

        // Fast path.
        // If the next character starts an ASCII identifier,
        // then this is a declaration.
        // Don't check for UTF-8 here to avoid having to read a codepoint
        // or determine Unicode letter value membership.
        if is_ascii_identifier_start(cur_char as u32) {
            // If the next characters are 'in', this may result in 'in' or
            // 'instanceof'. So we'd actually have to run a lookahead.
            if !(cur_char == b'i' && self.cursor.peek_at(1) == b'n') {
                return true;
            }
        }

        // Slow path.
        // There might be comments, newlines, UTF-8 identifiers, etc.
        // If there's a next token and it's an identifier, '[', '{', then this
        // is a declaration. Otherwise, it's not.
        // Pass RequireNoNewLine=false because
        //   let
        //   x = 3;
        // is supposed to parse as a let declaration of x, no ASI here.
        // https://262.ecma-international.org/14.0/#prod-LexicalBinding
        let next_token_kind = self.lookahead1::<false>(None);
        matches!(
            next_token_kind,
            Some(TokenKind::identifier)
                | Some(TokenKind::l_brace)
                | Some(TokenKind::l_square)
        )
    }

    /// \return true if the `using` keyword (the current token) is followed by an
    /// identifier with no intervening line terminator. Port of
    /// `isUsingFollowedByIdentifier` (JSLexer.cpp:178-204).
    ///
    /// DEVIATION: the C++ takes the `Keywords &kw` and asserts the current token
    /// is `kw.identUsing`; we keep that as a `debug_assert` against the interned
    /// identifier bytes.
    pub fn is_using_followed_by_identifier(&mut self) -> bool {
        debug_assert!(
            self.token.kind() == TokenKind::identifier
                && self.strtab.bytes(self.token.get_identifier()) == b"using",
            "current token must be the `using` identifier"
        );
        // Checking for:
        // using [no LineTerminator here] Identifier
        //      ^

        let saved_ptr = self.cursor.offset();
        let cur_char = self.optimistic_skip_whitespace();
        self.cursor.seek(saved_ptr);

        // Check for newline - if present, this is not a using declaration.
        if cur_char == b'\r' || cur_char == b'\n' {
            return false;
        }

        // Fast path: next char starts an ASCII identifier.
        if is_ascii_identifier_start(cur_char as u32) {
            return true;
        }

        // Slow path: use lookahead with RequireNoNewLine=true.
        let next_token_kind = self.lookahead1::<true>(None);
        next_token_kind == Some(TokenKind::identifier)
    }

    /// \return true if `await` (the current token) is followed by `using` and
    /// then an identifier, with no intervening line terminators. Port of
    /// `isAwaitUsingFollowedByIdentifier` (JSLexer.cpp:206-253).
    ///
    /// DEVIATION: the C++ takes `Keywords &kw`; the `kw.identUsing` atom is
    /// passed in as `ident_using`.
    pub fn is_await_using_followed_by_identifier(
        &mut self,
        ident_using: AtomBytes,
    ) -> bool {
        debug_assert!(
            self.token.kind() == TokenKind::identifier
                && self.strtab.bytes(self.token.get_identifier()) == b"await",
            "current token must be the `await` identifier"
        );
        // Checking for:
        // await [no LineTerminator here] using [no LineTerminator here] Identifier
        //      ^

        let saved_ptr = self.cursor.offset();

        // Skip whitespace after 'await' (no newlines allowed).
        let mut cur_char = self.optimistic_skip_whitespace();

        // Check for newline.
        if cur_char == b'\r' || cur_char == b'\n' {
            self.cursor.seek(saved_ptr);
            return false;
        }

        // Fast path: check if next chars are 'using' followed by whitespace
        // and an ASCII identifier.
        // Note that we can just check character by character because the buffer
        // is null-terminated.
        if cur_char == b'u'
            && self.cursor.peek_at(1) == b's'
            && self.cursor.peek_at(2) == b'i'
            && self.cursor.peek_at(3) == b'n'
            && self.cursor.peek_at(4) == b'g'
            && !is_ascii_identifier_continue(self.cursor.peek_at(5) as u32)
        {
            self.cursor.advance(5);
            cur_char = self.optimistic_skip_whitespace();

            self.cursor.seek(saved_ptr);

            // Check for newline between 'using' and identifier.
            if cur_char == b'\r' || cur_char == b'\n' {
                return false;
            }

            if is_ascii_identifier_start(cur_char as u32) {
                return true;
            }
        }

        // Slow path.
        // There might be comments, newlines, UTF-8 identifiers, etc.
        self.cursor.seek(saved_ptr);
        let opt_next = self.lookahead2::<true>(ident_using);
        opt_next == Some(TokenKind::identifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hermes_atom_table::AtomTable;
    use hermes_support::manager::SourceErrorManager;

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
        assert_eq!(lex.lookahead1::<true>(None), Some(TokenKind::rw_function));
        // state restored: current token still 'async', next advance is 'function'
        assert_eq!(lex.token().kind(), TokenKind::identifier);
        assert_eq!(
            lex.advance(GrammarContext::AllowDiv).kind(),
            TokenKind::rw_function
        );
    }

    #[test]
    fn lookahead1_consume_truncates_comments() {
        // With store_comments on, a comment collected during a CONSUMED lookahead
        // (expected matched) must be rolled back from comment storage, matching the
        // C++ make_scope_exit which fires on the early return too.
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "a /*c*/ b");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.set_store_comments(true);
        lex.advance(GrammarContext::AllowDiv); // 'a'
        assert_eq!(lex.get_stored_comments().len(), 0);
        // Consume the lookahead (next token 'b' is an identifier, which matches).
        assert_eq!(
            lex.lookahead1::<true>(Some(TokenKind::identifier)),
            Some(TokenKind::identifier)
        );
        // The block comment scanned during the consumed lookahead is rolled back.
        assert_eq!(lex.get_stored_comments().len(), 0);
    }

    #[test]
    fn lookahead1_newline_and_expected() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", "async\nx");
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.advance(GrammarContext::AllowDiv); // 'async'
                                               // RequireNoNewLine=true and there IS a newline -> None
        assert_eq!(lex.lookahead1::<true>(None), None);

        // when expectedToken matches, the cursor is NOT moved back (consumes
        // the lookahead):
        let mut sm2 = SourceErrorManager::new();
        let id2 = sm2.add_buffer("t2", "a b");
        let tab2 = AtomTable::new();
        let mut lex2 =
            JSLexer::new(id2, &mut sm2, &tab2, GrammarContext::AllowDiv);
        lex2.advance(GrammarContext::AllowDiv); // 'a'
        assert_eq!(
            lex2.lookahead1::<true>(Some(TokenKind::identifier)),
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
        assert_eq!(lex.lookahead2::<true>(using), Some(TokenKind::identifier));
        // state restored to 'await'
        assert_eq!(lex.token().kind(), TokenKind::identifier);
        assert_eq!(
            lex.token().get_res_word_or_identifier(),
            tab.atom_bytes(b"await")
        );
    }

    #[test]
    fn let_decl_start() {
        fn islet(src: &str) -> bool {
            let mut sm = SourceErrorManager::new();
            let id = sm.add_buffer("t", src);
            let tab = AtomTable::new();
            let mut lex =
                JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
            lex.advance(GrammarContext::AllowDiv); // 'let'
            lex.is_let_followed_by_decl_start()
        }
        assert!(islet("let x"));
        assert!(islet("let {a}"));
        assert!(islet("let [a]"));
        assert!(islet("let\nx")); // no ASI: still a declaration
        assert!(!islet("let in")); // 'let in ...' is not a decl ('in' operator)
        assert!(!islet("let = 3")); // 'let' as identifier
    }

    #[test]
    fn using_decls() {
        fn isusing(src: &str) -> bool {
            let mut sm = SourceErrorManager::new();
            let id = sm.add_buffer("t", src);
            let tab = AtomTable::new();
            let mut lex =
                JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
            lex.advance(GrammarContext::AllowDiv); // 'using'
            lex.is_using_followed_by_identifier()
        }
        assert!(isusing("using x"));
        assert!(!isusing("using\nx")); // newline -> not a using decl
        assert!(!isusing("using = 1")); // 'using' as identifier

        fn isawait(src: &str) -> bool {
            let mut sm = SourceErrorManager::new();
            let id = sm.add_buffer("t", src);
            let tab = AtomTable::new();
            let mut lex =
                JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
            lex.advance(GrammarContext::AllowDiv); // 'await'
            let using = tab.atom_bytes(b"using");
            lex.is_await_using_followed_by_identifier(using)
        }
        assert!(isawait("await using x"));
        assert!(!isawait("await using\nx"));
        assert!(!isawait("await x"));
    }
}
