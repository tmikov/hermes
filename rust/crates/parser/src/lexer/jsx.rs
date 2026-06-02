//! JSX scanners for the JS lexer: HTML-entity decoding (`consume_html_entity_\
//! optional`) and `advance_in_jsx_child` (JSX text + `{`/`<` delimiters).
//!
//! These `impl<'a> JSLexer<'a>` methods live in a child module of `lexer`, so
//! they can access the private fields of `JSLexer` declared in `lexer/mod.rs`.

use unicode::UNICODE_MAX_VALUE;

use crate::html_entities;

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
}

#[cfg(test)]
mod tests {
    use atom_table::AtomTable;
    use support::manager::SourceErrorManager;

    use super::super::{GrammarContext, JSLexer};

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
}
