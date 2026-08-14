//! String literal scanner for the JS lexer.
//!
//! These `impl<'a> JSLexer<'a>` methods live in a child module of `lexer`, so
//! they can access the private fields of `JSLexer` declared in `lexer/mod.rs`.

use crate::utf8::{
    append_unicode_to_storage, is_utf8_start,
    match_unicode_line_terminator_offset1, UTF8_LINE_TERMINATOR_CHAR0,
};

use super::{GrammarContext, JSLexer};

impl<'a> JSLexer<'a> {
    /// Scan a string literal under the grammar context `grammar_context`. Port
    /// of `JSLexer::scanStringInContext` (JSLexer.h:1029-1035): JSX-context
    /// strings (`AllowJSXIdentifier`) decode `&`-HTML-entities, allow raw
    /// newlines, and treat `\` as a literal character.
    pub(crate) fn scan_string_in_context(&mut self, grammar_context: GrammarContext) {
        if grammar_context == GrammarContext::AllowJSXIdentifier {
            self.scan_string::<true>();
        } else {
            self.scan_string::<false>();
        }
    }

    /// Scan a string literal (the cursor is on the opening quote). Port of
    /// `JSLexer::scanString<JSX>` (JSLexer.cpp:1977-2126). When `JSX`, a raw
    /// `\n`/`\r` is pushed to storage (not a non-terminated error), a `&` is
    /// decoded as an HTML entity, and `\` is a literal character (no escape).
    ///
    /// The C++ `template <bool JSX>` is preserved as the const generic `JSX`, so
    /// each specialization folds the `JSX` checks away at compile time.
    pub(crate) fn scan_string<const JSX: bool>(&mut self) {
        debug_assert!(self.cursor.peek() == b'\'' || self.cursor.peek() == b'"');
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
            } else if !JSX && c == b'\\' {
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
                if JSX {
                    // A raw new line is allowed in a JSX string.
                    self.tmp_storage.push(c);
                    self.cursor.advance(1);
                } else {
                    // A raw new line in a (non-JSX) string is not allowed.
                    let loc = self.cur_loc();
                    self.error(loc, "non-terminated string");
                    let start = self.token.start_loc();
                    self.sm.note(start, "string started here");
                    break;
                }
            } else if JSX && c == b'&' {
                if let Some(code_point) = self.consume_html_entity_optional() {
                    append_unicode_to_storage(&mut self.tmp_storage, code_point);
                } else {
                    self.tmp_storage.push(c);
                    self.cursor.advance(1);
                }
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

        let atom = self.get_string_literal(self.tmp_storage.as_slice());
        self.token.set_string_literal(atom, escapes);
    }
}

#[cfg(test)]
mod tests {
    use hermes_atom_table::AtomTable;
    use hermes_support::manager::SourceErrorManager;

    use super::super::{GrammarContext, JSLexer};
    use crate::token_kinds::TokenKind;

    /// Lex the first token of `src` (a string literal) in JSX context and return
    /// its cooked value bytes.
    fn jsx_str_cooked(src: &str) -> Vec<u8> {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowJSXIdentifier);
        let tok = lex.advance(GrammarContext::AllowJSXIdentifier);
        assert_eq!(tok.kind(), TokenKind::string_literal);
        tab.bytes(tok.get_string_literal()).to_vec()
    }

    #[test]
    fn jsx_string() {
        // In JSX context, '&' entities are decoded and raw newlines are allowed.
        assert_eq!(jsx_str_cooked("\"a&amp;b\""), b"a&b".to_vec());
        assert_eq!(jsx_str_cooked("'x\ny'"), b"x\ny".to_vec());
        assert_eq!(jsx_str_cooked("\"&#65;\""), b"A".to_vec());
    }
}
