//! Regular-expression literal scanning for the JS lexer.
//!
//! Port of `JSLexer::scanRegExp` (JSLexer.cpp:2384-2484). These
//! `impl<'a> JSLexer<'a>` methods live in a child module of `lexer`, so they can
//! access the private fields of `JSLexer` declared in `lexer/mod.rs`.

use hermes_atom_table::AtomBytes;

use crate::token::RegExpLiteral;
use crate::utf8::{
    append_unicode_to_storage, is_utf8_start,
    match_unicode_line_terminator_offset1, UTF8_LINE_TERMINATOR_CHAR0,
};

use super::{JSLexer, JsMode};

impl<'a> JSLexer<'a> {
    /// Scan a regular-expression literal `/body/flags`, with the cursor on the
    /// leading `/`. The body and flags are passed uninterpreted to the regular
    /// expression constructor (ES6 5.1 7.8.5), so escape sequences are not
    /// interpreted here. Port of `JSLexer::scanRegExp` (JSLexer.cpp:2384-2484).
    pub(crate) fn scan_regexp(&mut self) {
        let start_loc = self.cur_loc();
        debug_assert!(self.cursor.peek() == b'/');
        self.cursor.advance(1);

        self.tmp_storage.clear();
        let mut in_class = false;

        // The body loop. `goto unterminated` / `goto exit_loop` in the C++ are
        // modelled with an `unterminated` flag + `break`s; the body interning
        // after the loop happens regardless of which exit was taken.
        loop {
            let mut unterminated = false;
            match self.cursor.peek() {
                b'/' => {
                    if !in_class {
                        self.cursor.advance(1);
                        break; // goto exitLoop
                    }
                }

                b'[' => {
                    in_class = true; // It may be true already, but so what.
                }

                b']' => {
                    in_class = false; // It may be false already, but so what.
                }

                b'\\' => {
                    // an escape
                    self.tmp_storage.push(b'\\');
                    self.cursor.advance(1);
                    match self.cursor.peek() {
                        b'\0' => {
                            if self.cursor.at_end() {
                                unterminated = true;
                            }
                        }
                        UTF8_LINE_TERMINATOR_CHAR0 => {
                            if match_unicode_line_terminator_offset1(
                                &self.cursor.raw()[self.cursor.offset() as usize..],
                            ) {
                                unterminated = true;
                            }
                        }
                        b'\n' | b'\r' => {
                            unterminated = true;
                        }
                        _ => {}
                    }
                }

                b'\0' => {
                    if self.cursor.at_end() {
                        unterminated = true;
                    }
                }
                UTF8_LINE_TERMINATOR_CHAR0 => {
                    if match_unicode_line_terminator_offset1(
                        &self.cursor.raw()[self.cursor.offset() as usize..],
                    ) {
                        unterminated = true;
                    }
                }

                b'\n' | b'\r' => {
                    unterminated = true;
                }

                _ => {}
            }

            if unterminated {
                let loc = self.cur_loc();
                self.error(loc, "non-terminated regular expression literal");
                self.sm.note(start_loc, "regular expression started here");
                break; // goto exitLoop
            }

            if is_utf8_start(self.cursor.peek()) {
                let cp = self.decode_utf8_advance();
                append_unicode_to_storage(&mut self.tmp_storage, cp);
            } else {
                self.tmp_storage.push(self.cursor.peek());
                self.cursor.advance(1);
            }
        }
        // exitLoop:
        let body: AtomBytes = self.get_string_literal(self.tmp_storage.as_slice());

        // Scan the flags. We must not interpret escape sequences.
        // E6 5.1 7.8.5: "The Strings of characters comprising the
        // RegularExpressionBody and the RegularExpressionFlags are passed
        // uninterpreted to the regular expression constructor"
        self.tmp_storage.clear();
        let mut escaping_backslash = false;
        loop {
            if self.consume_one_identifier_part_no_escape::<JsMode>() {
                escaping_backslash = false;
                continue;
            } else if self.cursor.peek() == b'\\' {
                self.tmp_storage.push(b'\\');
                self.cursor.advance(1);

                // ES6 11.8.5.1: It is a Syntax Error if IdentifierPart contains a
                // Unicode escape sequence.
                escaping_backslash = !escaping_backslash;
                if escaping_backslash && self.cursor.peek() == b'u' {
                    let loc = self.cur_loc();
                    self.error(
                        loc,
                        "Unicode escape sequences are not allowed in regular expression flags",
                    );
                }
            } else {
                break;
            }
        }

        let flags: AtomBytes = self.get_string_literal(self.tmp_storage.as_slice());

        self.token
            .set_regexp_literal(RegExpLiteral::new(body, flags));
    }
}

#[cfg(test)]
mod tests {
    use crate::lexer::{GrammarContext, JSLexer};
    use crate::token_kinds::TokenKind;
    use hermes_atom_table::AtomTable;
    use hermes_support::manager::SourceErrorManager;

    /// Lex `src` under `ctx` and return the kind sequence (incl. the final eof).
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

    /// Lex `src` as a single regexp literal under `AllowRegExp` and return its
    /// `(body, flags)` interned bytes.
    fn regexp(src: &str) -> (Vec<u8>, Vec<u8>) {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowRegExp);
        let tok = lex.advance(GrammarContext::AllowRegExp);
        assert_eq!(tok.kind(), TokenKind::regexp_literal, "src={src:?}");
        let re = tok.get_regexp_literal();
        (tab.bytes(re.body()).to_vec(), tab.bytes(re.flags()).to_vec())
    }

    #[test]
    fn regexp_basic() {
        use TokenKind::*;
        assert_eq!(
            kinds_ctx("/abc/g", GrammarContext::AllowRegExp),
            vec![regexp_literal, eof]
        );
        assert_eq!(regexp("/abc/gi"), (b"abc".to_vec(), b"gi".to_vec()));
        assert_eq!(regexp("/[/]/"), (b"[/]".to_vec(), b"".to_vec())); // '/' inside a class is body
        assert_eq!(regexp("/a\\/b/"), (b"a\\/b".to_vec(), b"".to_vec())); // escaped '/' is body
        assert_eq!(regexp("/x/y"), (b"x".to_vec(), b"y".to_vec()));
        // under AllowDiv, '/' is a division operator, NOT a regexp:
        assert_eq!(
            kinds_ctx("a / b", GrammarContext::AllowDiv),
            vec![identifier, slash, identifier, eof]
        );
    }
}
