//! Template literal scanner for the JS lexer.
//!
//! These `impl<'a> JSLexer<'a>` methods live in a child module of `lexer`, so
//! they can access the private fields of `JSLexer` declared in `lexer/mod.rs`.

use crate::token_kinds::TokenKind;
use crate::utf8::{
    append_unicode_to_storage, is_utf8_start,
    match_unicode_line_terminator_offset1, UTF8_LINE_TERMINATOR_CHAR0,
};

use super::JSLexer;

impl<'a> JSLexer<'a> {
    /// Scan a template literal (the cursor is on `` ` `` or `}`). Port of
    /// `JSLexer::scanTemplateLiteral` (JSLexer.cpp:2128-2381). The `}`-start path
    /// is reached only via `rescanRBraceInTemplateLiteral` (a later phase); the
    /// `` ` `` `advance` arm always starts at a backtick.
    pub(crate) fn scan_template_literal(&mut self) {
        debug_assert!(self.cursor.peek() == b'`' || self.cursor.peek() == b'}');

        // Whether the token will result in TemplateHead upon encountering ${.
        // If we end the literal with `, then the result is NoSubstitutionTemplate,
        // so this will be ignored.
        let is_head = self.cursor.peek() == b'`';

        // If the token ended with a ` then it's a tail (or NoSubstitutionTemplate),
        // and if it ended with a ${ then it's not a tail.
        let mut is_tail = false;

        // Advance past the initial `.
        self.cursor.advance(1);

        // Track whether we encounter any NotEscapeSequence instances,
        // which will be used to error out on non-tagged sequences.
        let mut found_not_escape_sequence = false;

        // Store the Template Value (TV) in the tmp_storage.
        self.tmp_storage.clear();

        // Store the Template Raw Value (TRV) in the raw_storage.
        self.raw_storage.clear();

        /// Return the Template Raw Value (TRV) of character `c`.
        /// The only time the TRV is different from c is when c is a <CR>.
        /// In that case, this function will return 0x0a (LINE FEED).
        fn trv(c: u8) -> u8 {
            if c == b'\r' {
                // This case takes \r and \r\n into account.
                // The code below which consumes line separators will skip the
                // following \n if there is a \r\n.
                // For the purposes of finding the TRV it doesn't matter.
                0x0a
            } else {
                c
            }
        }

        loop {
            let c = self.cursor.peek();
            if c == b'`' {
                is_tail = true;
                self.cursor.advance(1);
                break;
            } else if c == b'$' && self.cursor.peek_at(1) == b'{' {
                // End of the TemplateCharacters.
                is_tail = false;
                self.cursor.advance(2);
                break;
            } else if c == b'\\' {
                self.raw_storage.push(c);
                self.cursor.advance(1);
                let e = self.cursor.peek();
                self.raw_storage.push(trv(e));
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
                            self.error(loc, "non-terminated template literal");
                            let start = self.token.start_loc();
                            self.sm.note(start, "template literal started here");
                            break;
                        } else {
                            self.tmp_storage.push(e);
                            self.cursor.advance(1);
                        }
                    }

                    b'0' => {
                        // '\0' is only a valid escape sequence if not followed by
                        // a DecimalDigit.
                        if !(self.cursor.peek_at(1) >= b'0' && self.cursor.peek_at(1) <= b'9') {
                            self.cursor.advance(1);
                            append_unicode_to_storage(&mut self.tmp_storage, 0);
                        } else {
                            // NotEscapeSequence :: 0 DecimalDigit
                            // Octal numbers are not supported in template strings,
                            // so leave the number in the raw storage (done above)
                            // and move on.
                            self.cursor.advance(1);
                            found_not_escape_sequence = true;
                        }
                    }

                    b'1'..=b'9' => {
                        // NotEscapeSequence :: DecimalDigit but not 0
                        // Octal numbers are not supported in template strings,
                        // so leave the number in the raw storage (done above) and
                        // move on.
                        self.cursor.advance(1);
                        found_not_escape_sequence = true;
                    }

                    b'x' => {
                        self.cursor.advance(1);
                        let start = self.cursor.offset();
                        let v = self.consume_hex(2, false);
                        if v.is_none() {
                            found_not_escape_sequence = true;
                        }
                        append_unicode_to_storage(&mut self.tmp_storage, v.unwrap_or(0));
                        let end = self.cursor.offset();
                        self.raw_storage
                            .extend_from_slice(&self.cursor.raw()[start as usize..end as usize]);
                    }

                    b'u' => {
                        // Offset of the first character after the 'u', which is
                        // where we can continue scanning from if we fail to decode
                        // an escape.
                        let start = self.cursor.offset() + 1;
                        // Reset the cursor to the '\' to scan the unicode escape.
                        self.cursor.seek(self.cursor.offset() - 1);
                        debug_assert!(
                            self.cursor.peek() == b'\\',
                            "must have started with \\"
                        );
                        let codepoint = self.consume_unicode_escape_optional();
                        if let Some(cp) = codepoint {
                            append_unicode_to_storage(&mut self.tmp_storage, cp);
                            let end = self.cursor.offset();
                            self.raw_storage.extend_from_slice(
                                &self.cursor.raw()[start as usize..end as usize],
                            );
                        } else {
                            found_not_escape_sequence = true;
                            self.cursor.seek(start);
                        }
                    }

                    // Escaped line terminator. We just need to skip it, because it
                    // was added to the raw storage at the start of the switch
                    // statement.
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
                        let is_line_terminator = match_unicode_line_terminator_offset1(
                            &self.cursor.raw()[self.cursor.offset() as usize..],
                        );
                        let codepoint = self.decode_utf8_advance();
                        // Needs to be added to the raw_storage regardless, but we
                        // first need to pop off the byte that was added prior to
                        // the switch statement.
                        self.raw_storage.pop();
                        append_unicode_to_storage(&mut self.raw_storage, codepoint);
                        if !is_line_terminator {
                            // Only add the codepoint to the tmp_storage if it
                            // wasn't a line terminator.
                            append_unicode_to_storage(&mut self.tmp_storage, codepoint);
                        }
                    }

                    _ => {
                        if is_utf8_start(e) {
                            let codepoint = self.decode_utf8_advance();
                            append_unicode_to_storage(&mut self.tmp_storage, codepoint);
                            // Remove the last byte from raw_storage and then append
                            // the unicode codepoint to it. The already inserted
                            // byte will change if this codepoint is in
                            // Supplementary Planes.
                            self.raw_storage.pop();
                            append_unicode_to_storage(&mut self.raw_storage, codepoint);
                        } else {
                            // The TV of EscapeSequence is the SV of EscapeSequence.
                            self.tmp_storage.push(e);
                            self.cursor.advance(1);
                        }
                    }
                }
            } else if c == 0 && self.cursor.at_end() {
                let loc = self.cur_loc();
                self.error(loc, "non-terminated template literal");
                let start = self.token.start_loc();
                self.sm.note(start, "template literal started here");
                break;
            } else if c == b'\r' {
                // The TV of LineTerminatorSequence is the TRV of
                // LineTerminatorSequence. The only time this differs from the same
                // characters as the bytes in the file is when the sequence begins
                // with a <CR>.
                self.tmp_storage.push(trv(c));
                self.raw_storage.push(trv(c));
                self.cursor.advance(1);
                if self.cursor.peek() == b'\n' {
                    // Skip the <CR> <LF>
                    self.cursor.advance(1);
                }
            } else if is_utf8_start(c) {
                // Decode and re-encode the character and append it to the string
                // storage.
                let codepoint = self.decode_utf8_advance();
                append_unicode_to_storage(&mut self.tmp_storage, codepoint);
                append_unicode_to_storage(&mut self.raw_storage, codepoint);
            } else {
                self.raw_storage.push(c);
                self.tmp_storage.push(c);
                self.cursor.advance(1);
            }
        }

        // If the template literal is tagged and contains invalid escapes, then
        // cooked should be null because there is no way to cook it, per the ESTree
        // 2018 spec. The parser will error when encountering an untagged literal
        // with invalid escapes, so we place None here.
        let cooked = if found_not_escape_sequence {
            None
        } else {
            Some(self.get_string_literal(self.tmp_storage.as_slice()))
        };
        let raw = self.get_string_literal(self.raw_storage.as_slice());
        let kind = if is_head {
            if is_tail {
                // ` characters `
                TokenKind::no_substitution_template
            } else {
                // ` characters ${
                TokenKind::template_head
            }
        } else if is_tail {
            // } characters `
            TokenKind::template_tail
        } else {
            // } characters ${
            TokenKind::template_middle
        };
        self.token.set_template_literal(kind, cooked, raw);
    }
}
