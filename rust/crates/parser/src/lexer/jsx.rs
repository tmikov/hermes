//! JSX scanners for the JS lexer: HTML-entity decoding (`consume_html_entity_\
//! optional`) and `advance_in_jsx_child` (JSX text + `{`/`<` delimiters).
//!
//! These `impl<'a> JSLexer<'a>` methods live in a child module of `lexer`, so
//! they can access the private fields of `JSLexer` declared in `lexer/mod.rs`.

use hermes_unicode::UNICODE_MAX_VALUE;

use crate::html_entities;
use crate::token::Token;
use crate::token_kinds::TokenKind;
use crate::utf8::{append_unicode_to_storage, is_utf8_start};

use super::{is_ascii_digit, JSLexer};

impl<'a> JSLexer<'a> {
    /// Try to consume an HTML entity at the cursor (which must be on `&`).
    /// Port of `JSLexer::consumeHTMLEntityOptional` (JSLexer.cpp:811-907).
    ///
    /// Recognizes `&#xHEX;` (hex), `&#NUMBER;` (decimal) and `&NAME;` (named).
    /// On any failure the cursor is reset to the `&` and `None` is returned.
    pub(crate) fn consume_html_entity_optional(&mut self) -> Option<u32> {
        debug_assert!(self.cursor.peek() == b'&');
        let start = self.cursor.offset();

        if self.cursor.peek_at(1) == b'#' {
            if self.cursor.peek_at(2) == b'x' {
                // HTML entity with form &#xHEX;
                self.cursor.advance(3);
                let number_start = self.cursor.offset();

                let mut code_point: u32 = 0;
                let mut ch = self.cursor.peek();

                // Calculate code point from non-empty sequence of hex digits
                // followed by a semicolon.
                loop {
                    if ch == b';' && self.cursor.offset() != number_start {
                        self.cursor.advance(1);
                        return Some(code_point);
                    } else if is_ascii_digit(ch) {
                        ch -= b'0';
                    } else {
                        ch |= 32;
                        if (b'a'..=b'f').contains(&ch) {
                            ch -= b'a' - 10;
                        } else {
                            break;
                        }
                    }

                    // Check that this number is representable as a code point.
                    code_point = (code_point << 4) + ch as u32;
                    if code_point > UNICODE_MAX_VALUE {
                        break;
                    }

                    self.cursor.advance(1);
                    ch = self.cursor.peek();
                }
            } else {
                // HTML entity with form &#NUMBER;
                self.cursor.advance(2);
                let number_start = self.cursor.offset();

                let mut code_point: u32 = 0;
                let mut ch = self.cursor.peek();

                // Calculate code point from non-empty sequence of decimal digits
                // followed by a semicolon.
                loop {
                    if ch == b';' && self.cursor.offset() != number_start {
                        self.cursor.advance(1);
                        return Some(code_point);
                    } else if is_ascii_digit(ch) {
                        // Check that this number is representable as a code point.
                        code_point = code_point * 10 + (ch - b'0') as u32;
                        if code_point > UNICODE_MAX_VALUE {
                            break;
                        }
                    } else {
                        break;
                    }

                    self.cursor.advance(1);
                    ch = self.cursor.peek();
                }
            }
        } else {
            // HTML entity with form &NAME;
            self.cursor.advance(1);

            // Gather HTML entity name and lookup name in table. HTML entity
            // names are composed of a sequence of up to 8 alphanumeric
            // characters followed by a semicolon. To minimize backtracking due
            // to an `&` without a following semicolon we only need to look at
            // most 9 characters ahead (8 for the name, 1 for the semicolon).
            for i in 0..9 {
                let ch = self.cursor.peek();
                if ch == b';' {
                    let name = self.cursor.slice(self.cursor.offset() - i, self.cursor.offset());
                    match html_entities::lookup(name) {
                        None => break,
                        Some(value) => {
                            self.cursor.advance(1);
                            return Some(value);
                        }
                    }
                } else if ((ch | 32) >= b'a' && (ch | 32) <= b'z') || is_ascii_digit(ch) {
                    self.cursor.advance(1);
                } else {
                    break;
                }
            }
        }

        self.cursor.seek(start);
        None
    }

    /// Advance to the next token while scanning a JSX child. Port of
    /// `JSLexer::advanceInJSXChild` (JSLexer.cpp:749-809). Emits `l_brace` /
    /// `less` for `{` / `<`, `eof` at end of input, and otherwise accumulates a
    /// single `jsx_text` token (with HTML entities decoded into the value and
    /// kept verbatim in the raw) up to the next `{` / `<` / EOF.
    pub fn advance_in_jsx_child(&mut self) -> &Token {
        self.token.set_start(self.cur_loc());
        // Structural `for(;;){ switch …; break; }` mirroring the C++ (and `advance()`):
        // the outer loop never actually iterates here (unlike `advance()`, the JSX-child
        // variant has no outer `continue`), but the shape is kept faithful to the C++.
        #[allow(clippy::never_loop)]
        loop {
            debug_assert!(
                (self.cursor.offset() as usize) <= self.cursor.raw().len(),
                "lexing past end of input"
            );
            match self.cursor.peek() {
                b'{' => {
                    self.punc_l1_1(TokenKind::l_brace);
                }
                b'<' => {
                    self.punc_l1_1(TokenKind::less);
                }

                0 if self.cursor.at_end() => {
                    self.token.set_eof();
                }

                // Fall-through to start scanning text.
                _ => {
                    let start = self.cur_loc();
                    self.token.set_start(start);

                    // Build up cooked value using XHTML entities.
                    self.tmp_storage.clear();
                    self.raw_storage.clear();
                    loop {
                        let c = self.cursor.peek();

                        if is_utf8_start(c) {
                            let codepoint = self.decode_utf8_advance();
                            append_unicode_to_storage(&mut self.tmp_storage, codepoint);
                            append_unicode_to_storage(&mut self.raw_storage, codepoint);
                            continue;
                        } else if c == b'&' {
                            let html_start = self.cursor.offset();
                            if let Some(code_point) = self.consume_html_entity_optional() {
                                append_unicode_to_storage(&mut self.tmp_storage, code_point);
                                let consumed = self.cursor.slice(html_start, self.cursor.offset());
                                self.raw_storage.extend_from_slice(consumed);
                                continue;
                            }
                        } else if (c == 0 && self.cursor.at_end()) || c == b'{' || c == b'<' {
                            let value = self.get_string_literal(self.tmp_storage.as_slice());
                            let raw = self.get_string_literal(self.raw_storage.as_slice());
                            self.token.set_jsx_text(value, raw);
                            break;
                        }
                        self.tmp_storage.push(c);
                        self.raw_storage.push(c);
                        self.cursor.advance(1);
                    }
                }
            }

            // Always terminate the loop unless "continue" was used.
            break;
        }
        self.finish_token();
        &self.token
    }
}

#[cfg(test)]
mod tests {
    use hermes_atom_table::AtomTable;
    use hermes_support::manager::SourceErrorManager;

    use super::super::{GrammarContext, JSLexer};
    use crate::token_kinds::TokenKind;

    /// Build a lexer over `src` with the cursor on the leading `&` and run
    /// `consume_html_entity_optional`, returning its result.
    fn entity(src: &str) -> Option<u32> {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowJSXIdentifier);
        lex.consume_html_entity_optional()
    }

    #[test]
    fn html_entities() {
        assert_eq!(entity("&amp;"), Some(0x26)); // named
        assert_eq!(entity("&#65;"), Some(65)); // decimal
        assert_eq!(entity("&#x41;"), Some(0x41)); // hex
        assert_eq!(entity("&nope;"), None); // unknown name -> None, cursor reset
        assert_eq!(entity("&amp"), None); // no semicolon -> None
    }

    /// Run the `advance_in_jsx_child` loop to EOF and collect token kinds.
    fn advance_jsx(src: &str) -> Vec<TokenKind> {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowJSXIdentifier);
        let mut kinds = Vec::new();
        loop {
            let k = lex.advance_in_jsx_child().kind();
            kinds.push(k);
            if k == TokenKind::eof {
                break;
            }
        }
        kinds
    }

    /// Lex the first `jsx_text` token of `src` and return `(value, raw)`.
    fn jsx_text_value(src: &str) -> (Vec<u8>, Vec<u8>) {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowJSXIdentifier);
        let tok = lex.advance_in_jsx_child();
        assert_eq!(tok.kind(), TokenKind::jsx_text);
        let value = tab.bytes(tok.get_jsx_text_value()).to_vec();
        let raw = tab.bytes(tok.get_jsx_text_raw()).to_vec();
        (value, raw)
    }

    #[test]
    fn jsx_child() {
        use TokenKind::*;
        // advance_in_jsx_child emits l_brace/less and accumulates everything
        // else as one jsx_text until {/</EOF.
        assert_eq!(advance_jsx("hello{x"), vec![jsx_text, l_brace, jsx_text, eof]);
        assert_eq!(advance_jsx("a<b"), vec![jsx_text, less, jsx_text, eof]);
        // jsx text value decodes entities; raw keeps them.
        assert_eq!(
            jsx_text_value("a&amp;b{"),
            (b"a&b".to_vec(), b"a&amp;b".to_vec())
        );
    }
}
