//! Token and friends, ported from include/hermes/Parser/JSLexer.h (Token,
//! RegExpLiteral, StoredComment, StoredToken).
//!
//! Unlike the C++, which holds `UniqueString *` pointers and `SMLoc` pointers
//! into the buffer, the Rust port is offset-based: locations are `SMRange`
//! (buffer id + byte offsets) and interned values are `AtomBytes` handles into
//! the `AtomTable`. A `Token` carries its kind, source range, and (depending on
//! the kind) a numeric value, an interned identifier/string-literal/raw value,
//! or a `RegExpLiteral` — the complete `Token` surface, matching the C++.
//!
//! The full set of value getters/setters
//! (numeric/identifier/string/template/regexp/bigint/jsx) is part of the
//! faithful `Token` surface; the public accessors are part of the lexer's API
//! (consumed by the parser), and the `pub(crate)` setters are all used by the
//! scanners or exercised by tests.

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
    /// A literal with the given interned body and flags.
    pub fn new(body: AtomBytes, flags: AtomBytes) -> RegExpLiteral {
        RegExpLiteral { body, flags }
    }
    /// \return the pattern between the delimiting slashes.
    pub fn body(&self) -> AtomBytes {
        self.body
    }
    /// \return the flags following the closing slash (possibly empty).
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

    /// \return the kind of this token.
    pub fn kind(&self) -> TokenKind {
        self.kind
    }
    /// \return true if this token is a reserved word.
    pub fn is_res_word(&self) -> bool {
        self.kind.is_res_word()
    }
    /// \return true if this token is one of the four template-literal kinds.
    pub fn is_template_literal(&self) -> bool {
        matches!(
            self.kind,
            TokenKind::no_substitution_template
                | TokenKind::template_head
                | TokenKind::template_middle
                | TokenKind::template_tail
        )
    }

    /// \return the location of the first byte of the token.
    pub fn start_loc(&self) -> SMLoc {
        self.range.start
    }
    /// \return the location one past the last byte of the token.
    pub fn end_loc(&self) -> SMLoc {
        self.range.end
    }
    /// \return the half-open source range covered by the token.
    pub fn source_range(&self) -> SMRange {
        self.range
    }

    /// \return the value of a `numeric_literal` token.
    pub fn get_numeric_literal(&self) -> f64 {
        debug_assert_eq!(self.kind, TokenKind::numeric_literal);
        self.numeric
    }

    /// \return the interned name of an `identifier` token.
    pub fn get_identifier(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::identifier);
        self.ident.unwrap()
    }
    /// \return the interned name of a `private_identifier` token, without
    /// the leading `#` (which is still part of the token's source range).
    pub fn get_private_identifier(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::private_identifier);
        self.ident.unwrap()
    }
    /// \return the interned spelling of a reserved-word token.
    pub fn get_res_word_identifier(&self) -> AtomBytes {
        debug_assert!(self.is_res_word());
        self.ident.unwrap()
    }
    /// \return the interned spelling of an identifier or reserved word,
    /// for the many places where the grammar accepts either.
    pub fn get_res_word_or_identifier(&self) -> AtomBytes {
        debug_assert!(self.kind == TokenKind::identifier || self.is_res_word());
        self.ident.unwrap()
    }

    /// \return the cooked value of a `string_literal` token, with escapes
    /// and line continuations already processed.
    pub fn get_string_literal(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::string_literal);
        self.string_literal.unwrap()
    }
    /// \return the raw (undecoded) text of a JSX string literal. Only set by
    /// `Token::set_jsx_string_literal`; the normal string path leaves it unset.
    pub fn get_string_literal_raw_value(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::string_literal);
        self.raw_string.unwrap()
    }
    /// \return whether a `string_literal` contained any escape or line
    /// continuation. Used to decide whether a string can be a directive.
    pub fn get_string_literal_contains_escapes(&self) -> bool {
        debug_assert_eq!(self.kind, TokenKind::string_literal);
        self.string_literal_contains_escapes
    }

    /// \return whether the template literal token contains a NotEscapeSequence.
    pub fn get_template_literal_contains_not_escapes(&self) -> bool {
        debug_assert!(self.is_template_literal());
        self.string_literal.is_none()
    }
    /// \return the cooked value of a template-literal token, or `None` if it
    /// contains a NotEscapeSequence (legal only in a tagged template).
    pub fn get_template_value(&self) -> Option<AtomBytes> {
        debug_assert!(self.is_template_literal());
        self.string_literal
    }
    /// \return the Template Raw Value (TRV) of a template-literal token.
    pub fn get_template_raw_value(&self) -> AtomBytes {
        debug_assert!(self.is_template_literal());
        self.raw_string.unwrap()
    }

    /// \return the digits of a `bigint_literal` token, without the trailing
    /// `n`; the value is kept as text and never converted to a number.
    pub fn get_bigint_literal(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::bigint_literal);
        self.string_literal.unwrap()
    }
    /// \return the raw source text of a `bigint_literal` token, including
    /// any radix prefix and the trailing `n`.
    pub fn get_bigint_literal_raw_value(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::bigint_literal);
        self.raw_string.unwrap()
    }

    /// \return the body and flags of a `regexp_literal` token.
    pub fn get_regexp_literal(&self) -> RegExpLiteral {
        debug_assert_eq!(self.kind, TokenKind::regexp_literal);
        self.regexp.unwrap()
    }

    /// \return the value of a `jsx_text` token, with HTML entities decoded.
    pub fn get_jsx_text_value(&self) -> AtomBytes {
        debug_assert_eq!(self.kind, TokenKind::jsx_text);
        self.string_literal.unwrap()
    }
    /// \return the raw source text of a `jsx_text` token, with HTML entities
    /// left as written.
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
    /// Port of C++ `Token::setJSXStringLiteral`. The lexer's JSX string path
    /// uses `set_string_literal` (matching the C++ `scanString`), so this is not
    /// called by the lexer itself; it is kept for faithful `Token` surface
    /// completeness and exercised by a unit test.
    #[allow(dead_code)]
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
    /// A comment of `kind` spanning `range`, delimiters included.
    pub fn new(kind: CommentKind, range: SMRange) -> StoredComment {
        StoredComment { kind, range }
    }
    /// \return whether this is a line, block, or hashbang comment.
    pub fn kind(&self) -> CommentKind {
        self.kind
    }
    /// \return the source range of the comment, delimiters included.
    pub fn source_range(&self) -> SMRange {
        self.range
    }

    /// \return the comment with delimiters (//, /*, */, #!) stripped. Port of
    /// `StoredComment::getString` (JSLexer.h:339-347).
    ///
    /// Unlike the C++, which dereferences pointers into the source buffer, our
    /// offset-based comment can't deref a pointer, so the caller passes the
    /// source `buffer` bytes and we slice into it.
    pub fn get_string<'a>(&self, buffer: &'a [u8]) -> &'a [u8] {
        // Ignore opening delimiter.
        let start = self.range.start.offset as usize + 2;
        // Conditionally ignore closing delimiter.
        let end = if self.kind == CommentKind::Block {
            self.range.end.offset as usize - 2
        } else {
            self.range.end.offset as usize
        };
        debug_assert!(end >= start, "invalid comment range");
        &buffer[start..end]
    }

    /// \return the comment with delimiters (//, /*, */, #!) included. Port of
    /// `StoredComment::getFullString` (JSLexer.h:349-355).
    ///
    /// Unlike the C++, which dereferences pointers into the source buffer, our
    /// offset-based comment can't deref a pointer, so the caller passes the
    /// source `buffer` bytes and we slice into it.
    pub fn get_full_string<'a>(&self, buffer: &'a [u8]) -> &'a [u8] {
        &buffer[self.range.start.offset as usize..self.range.end.offset as usize]
    }
}

/// Stored token when lexing. Port of `StoredToken`.
#[derive(Copy, Clone, Debug)]
pub struct StoredToken {
    kind: TokenKind,
    range: SMRange,
}

impl StoredToken {
    /// A stored token of `kind` spanning `range`.
    pub fn new(kind: TokenKind, range: SMRange) -> StoredToken {
        StoredToken { kind, range }
    }
    /// \return the kind of the stored token.
    pub fn kind(&self) -> TokenKind {
        self.kind
    }
    /// \return the source range of the stored token.
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

    #[test]
    fn jsx_string_literal_value_and_raw() {
        // set_jsx_string_literal sets value + raw + escapes=false. Exercises
        // set_jsx_string_literal and get_string_literal_raw_value (a faithful
        // Token surface the parser consumes; otherwise unexercised in-crate).
        let tab = atom_table::AtomTable::new();
        let value = tab.atom_bytes(b"a<b");
        let raw = tab.atom_bytes(b"a&lt;b");
        let mut t = Token::new(SourceId::from_index(0));
        t.set_jsx_string_literal(value, raw);
        assert_eq!(t.kind(), TokenKind::string_literal);
        assert_eq!(t.get_string_literal(), value);
        assert_eq!(t.get_string_literal_raw_value(), raw);
        assert!(!t.get_string_literal_contains_escapes());
    }

    #[test]
    fn template_literal_contains_not_escapes() {
        // get_template_literal_contains_not_escapes() == (cooked is None).
        let tab = atom_table::AtomTable::new();
        let raw = tab.atom_bytes(b"\\9");
        let mut t = Token::new(SourceId::from_index(0));
        t.set_template_literal(TokenKind::no_substitution_template, None, raw);
        assert!(t.get_template_literal_contains_not_escapes());
        let cooked = tab.atom_bytes(b"ok");
        t.set_template_literal(TokenKind::template_head, Some(cooked), raw);
        assert!(!t.get_template_literal_contains_not_escapes());
        assert_eq!(t.get_template_value(), Some(cooked));
    }

    #[test]
    fn stored_comment_get_string() {
        let id = SourceId::from_index(0);
        let loc = |off| SMLoc {
            source: id,
            offset: off,
        };
        // Buffer: a line comment then a block comment.
        //          0         1         2
        //          0123456789012345678901234
        let buffer = b"// hello /* world */ rest";
        // Line comment "// hello" spans [0, 8).
        let line = StoredComment::new(
            CommentKind::Line,
            SMRange {
                start: loc(0),
                end: loc(8),
            },
        );
        assert_eq!(line.get_string(buffer), b" hello");
        assert_eq!(line.get_full_string(buffer), b"// hello");
        // Block comment "/* world */" spans [9, 20).
        let block = StoredComment::new(
            CommentKind::Block,
            SMRange {
                start: loc(9),
                end: loc(20),
            },
        );
        assert_eq!(block.get_string(buffer), b" world ");
        assert_eq!(block.get_full_string(buffer), b"/* world */");
    }
}
