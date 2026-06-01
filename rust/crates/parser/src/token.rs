//! Token and friends, ported from include/hermes/Parser/JSLexer.h (Token,
//! RegExpLiteral, StoredComment, StoredToken).
//!
//! Unlike the C++, which holds `UniqueString *` pointers and `SMLoc` pointers
//! into the buffer, the Rust port is offset-based: locations are `SMRange`
//! (buffer id + byte offsets) and interned values are `AtomBytes` handles into
//! the `AtomTable`. Phase 1a only sets kind + range + punctuator/eof; the value
//! fields and accessors are carried with faithful shapes for later phases.
//!
//! Many value getters/setters (numeric/identifier/string/template/regexp/bigint
//! /jsx) are part of the faithful `Token` surface but are not yet exercised in
//! phase 1a, which only lexes punctuators/trivia/eof. They are wired up by the
//! lexer in phases 1b+, so we allow `dead_code` for the whole module rather than
//! drop the surface.
#![allow(dead_code)]

use atom_table::AtomBytes;
use support::location::{SMLoc, SMRange, SourceId};

use crate::token_kinds::TokenKind;

/// Port of `JSLexer.h`'s `RegExpLiteral`: an interned body and flags.
#[derive(Copy, Clone, Debug)]
pub struct RegExpLiteral {
    body: AtomBytes,
    flags: AtomBytes,
}

impl RegExpLiteral {
    pub fn new(body: AtomBytes, flags: AtomBytes) -> RegExpLiteral {
        RegExpLiteral { body, flags }
    }
    pub fn body(&self) -> AtomBytes {
        self.body
    }
    pub fn flags(&self) -> AtomBytes {
        self.flags
    }
}

/// Encapsulates the information contained in the current token.
/// We only ever create one of these, but it is cleaner to keep the data
/// in a separate class. Port of `Token`.
#[derive(Clone, Debug)]
pub struct Token {
    kind: TokenKind,
    range: SMRange,
    numeric: f64,
    ident: Option<AtomBytes>,

    /// Representation of the string literal for tokens that are strings.
    /// If the current token is part of a template literal, this is `None`
    /// when it contains a NotEscapeSequence.
    string_literal: Option<AtomBytes>,

    regexp: Option<RegExpLiteral>,

    /// Representation of one of these depending on the TokenKind:
    /// - The Template Raw Value (TRV) associated with the token if it
    ///   represents a part or whole of a template literal.
    /// - The raw string of a JSXText.
    raw_string: Option<AtomBytes>,

    /// If the current token is a string literal, this flag indicates whether it
    /// contains any escapes or new line continuations. We need this in order to
    /// detect directives.
    string_literal_contains_escapes: bool,
}

impl Token {
    /// A fresh `none` token with an empty range in `source`.
    pub fn new(source: SourceId) -> Token {
        let loc = SMLoc { source, offset: 0 };
        Token {
            kind: TokenKind::none,
            range: SMRange {
                start: loc,
                end: loc,
            },
            numeric: 0.0,
            ident: None,
            string_literal: None,
            regexp: None,
            raw_string: None,
            string_literal_contains_escapes: false,
        }
    }

    pub fn kind(&self) -> TokenKind {
        self.kind
    }
    pub fn is_res_word(&self) -> bool {
        self.kind.is_res_word()
    }
    pub fn is_template_literal(&self) -> bool {
        matches!(
            self.kind,
            TokenKind::no_substitution_template
                | TokenKind::template_head
                | TokenKind::template_middle
                | TokenKind::template_tail
        )
    }

    pub fn start_loc(&self) -> SMLoc {
        self.range.start
    }
    pub fn end_loc(&self) -> SMLoc {
        self.range.end
    }
    pub fn source_range(&self) -> SMRange {
        self.range
    }

    pub fn get_numeric_literal(&self) -> f64 {
        debug_assert_eq!(self.kind, TokenKind::numeric_literal);
        self.numeric
    }

    pub fn get_identifier(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::identifier);
        self.ident.unwrap()
    }
    pub fn get_private_identifier(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::private_identifier);
        self.ident.unwrap()
    }
    pub fn get_res_word_identifier(&self) -> AtomBytes {
        debug_assert!(self.is_res_word());
        self.ident.unwrap()
    }
    pub fn get_res_word_or_identifier(&self) -> AtomBytes {
        debug_assert!(self.kind == TokenKind::identifier || self.is_res_word());
        self.ident.unwrap()
    }

    pub fn get_string_literal(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::string_literal);
        self.string_literal.unwrap()
    }
    pub fn get_string_literal_raw_value(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::string_literal);
        self.raw_string.unwrap()
    }
    pub fn get_string_literal_contains_escapes(&self) -> bool {
        debug_assert_eq!(self.kind, TokenKind::string_literal);
        self.string_literal_contains_escapes
    }

    /// \return whether the template literal token contains a NotEscapeSequence.
    pub fn get_template_literal_contains_not_escapes(&self) -> bool {
        debug_assert!(self.is_template_literal());
        self.string_literal.is_none()
    }
    pub fn get_template_value(&self) -> Option<AtomBytes> {
        debug_assert!(self.is_template_literal());
        self.string_literal
    }
    pub fn get_template_raw_value(&self) -> AtomBytes {
        debug_assert!(self.is_template_literal());
        self.raw_string.unwrap()
    }

    pub fn get_bigint_literal(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::bigint_literal);
        self.string_literal.unwrap()
    }
    pub fn get_bigint_literal_raw_value(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::bigint_literal);
        self.raw_string.unwrap()
    }

    pub fn get_regexp_literal(&self) -> RegExpLiteral {
        debug_assert_eq!(self.kind, TokenKind::regexp_literal);
        self.regexp.unwrap()
    }

    pub fn get_jsx_text_value(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::jsx_text);
        self.string_literal.unwrap()
    }
    pub fn get_jsx_text_raw(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::jsx_text);
        self.raw_string.unwrap()
    }

    // ---- Setters (crate-visible: only the lexer mutates the token) ----------

    pub(crate) fn set_start(&mut self, start: SMLoc) {
        self.range.start = start;
    }
    pub(crate) fn set_end(&mut self, end: SMLoc) {
        self.range.end = end;
    }
    pub(crate) fn set_range(&mut self, range: SMRange) {
        self.range = range;
    }

    pub(crate) fn set_punctuator(&mut self, kind: TokenKind) {
        self.kind = kind;
    }
    /// Set the TokenKind to a given IDENT_OP token.
    pub(crate) fn set_ident_op(&mut self, kind: TokenKind) {
        self.kind = kind;
    }
    pub(crate) fn set_eof(&mut self) {
        self.kind = TokenKind::eof;
    }

    pub(crate) fn set_bigint_literal(&mut self, bigint: AtomBytes, raw: AtomBytes) {
        self.kind = TokenKind::bigint_literal;
        self.string_literal = Some(bigint);
        self.raw_string = Some(raw);
    }
    pub(crate) fn set_numeric_literal(&mut self, literal: f64) {
        self.kind = TokenKind::numeric_literal;
        self.numeric = literal;
    }
    pub(crate) fn set_identifier(&mut self, ident: AtomBytes) {
        self.kind = TokenKind::identifier;
        self.ident = Some(ident);
    }
    pub(crate) fn set_private_identifier(&mut self, ident: AtomBytes) {
        self.kind = TokenKind::private_identifier;
        self.ident = Some(ident);
    }
    pub(crate) fn set_string_literal(&mut self, literal: AtomBytes, contains_escapes: bool) {
        self.kind = TokenKind::string_literal;
        self.string_literal = Some(literal);
        self.string_literal_contains_escapes = contains_escapes;
    }
    pub(crate) fn set_jsx_string_literal(&mut self, literal: AtomBytes, raw: AtomBytes) {
        self.kind = TokenKind::string_literal;
        self.string_literal = Some(literal);
        self.raw_string = Some(raw);
        self.string_literal_contains_escapes = false;
    }
    pub(crate) fn set_regexp_literal(&mut self, literal: RegExpLiteral) {
        self.kind = TokenKind::regexp_literal;
        self.regexp = Some(literal);
    }
    pub(crate) fn set_res_word(&mut self, kind: TokenKind, ident: AtomBytes) {
        debug_assert!(kind.is_res_word());
        self.kind = kind;
        self.ident = Some(ident);
    }
    pub(crate) fn set_template_literal(
        &mut self,
        kind: TokenKind,
        cooked: Option<AtomBytes>,
        raw: AtomBytes,
    ) {
        debug_assert!(matches!(
            kind,
            TokenKind::no_substitution_template
                | TokenKind::template_head
                | TokenKind::template_middle
                | TokenKind::template_tail
        ));
        self.kind = kind;
        self.string_literal = cooked;
        self.raw_string = Some(raw);
    }
    pub(crate) fn set_jsx_text(&mut self, value: AtomBytes, raw: AtomBytes) {
        self.kind = TokenKind::jsx_text;
        self.string_literal = Some(value);
        self.raw_string = Some(raw);
    }
}

/// The kind of a stored comment. Port of `StoredComment::Kind`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum CommentKind {
    /// Comment that begins with "//".
    Line,
    /// Comment that is delimited by "/*" and "*/".
    Block,
    /// Comment that begins with "#!" and starts at the first byte of the file.
    Hashbang,
}

/// Represents a comment stored while lexing the file. Port of `StoredComment`.
#[derive(Copy, Clone, Debug)]
pub struct StoredComment {
    kind: CommentKind,
    range: SMRange,
}

impl StoredComment {
    pub fn new(kind: CommentKind, range: SMRange) -> StoredComment {
        StoredComment { kind, range }
    }
    pub fn kind(&self) -> CommentKind {
        self.kind
    }
    pub fn source_range(&self) -> SMRange {
        self.range
    }
}

/// Stored token when lexing. Port of `StoredToken`.
#[derive(Copy, Clone, Debug)]
pub struct StoredToken {
    kind: TokenKind,
    range: SMRange,
}

impl StoredToken {
    pub fn new(kind: TokenKind, range: SMRange) -> StoredToken {
        StoredToken { kind, range }
    }
    pub fn kind(&self) -> TokenKind {
        self.kind
    }
    pub fn source_range(&self) -> SMRange {
        self.range
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_kinds::TokenKind;
    use support::location::{SMLoc, SMRange, SourceId};

    #[test]
    fn punctuator_token() {
        let id = SourceId::from_index(0);
        let mut t = Token::new(id);
        t.set_punctuator(TokenKind::l_brace);
        t.set_range(SMRange {
            start: SMLoc {
                source: id,
                offset: 0,
            },
            end: SMLoc {
                source: id,
                offset: 1,
            },
        });
        assert_eq!(t.kind(), TokenKind::l_brace);
        assert_eq!(t.start_loc().offset, 0);
        assert_eq!(t.end_loc().offset, 1);
    }
}
