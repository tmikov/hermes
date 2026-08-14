//! Unicode/octal/hex escape consumers for the JS lexer.
//!
//! These `impl<'a> JSLexer<'a>` methods live in a child module of `lexer`, so
//! they can access the private fields of `JSLexer` declared in `lexer/mod.rs`.

use hermes_support::location::{SMLoc, SMRange};

use hermes_unicode::{UNICODE_MAX_VALUE, UNICODE_REPLACEMENT_CHARACTER};

use super::JSLexer;

impl<'a> JSLexer<'a> {
    /// Consume `required_len` hex digits, accumulating them into a code point.
    /// Port of `consumeHex` (JSLexer.cpp:1329-1353). On failure, if
    /// `error_on_fail`, report "invalid hex number" at the offending position;
    /// returns `None`.
    pub(crate) fn consume_hex(
        &mut self,
        required_len: u32,
        error_on_fail: bool,
    ) -> Option<u32> {
        let mut cp: u32 = 0;
        for _ in 0..required_len {
            let mut ch = self.cursor.peek() as u32;
            if (b'0' as u32..=b'9' as u32).contains(&ch) {
                ch -= b'0' as u32;
            } else {
                // Now that we know it is not a digit, it is safe to lowercase.
                ch |= 32;
                if (b'a' as u32..=b'f' as u32).contains(&ch) {
                    ch -= b'a' as u32 - 10;
                } else {
                    if error_on_fail {
                        let loc = self.cur_loc();
                        self.error(loc, "invalid hex number");
                    }
                    return None;
                }
            }
            cp = (cp << 4) + ch;
            self.cursor.advance(1);
        }
        Some(cp)
    }

    /// Consume up to `max_len` octal digits (the cursor is on the first octal
    /// digit), accumulating them into a byte. Port of `consumeOctal`
    /// (JSLexer.cpp:1311-1327), including the strict-mode error.
    pub(crate) fn consume_octal(&mut self, mut max_len: u32) -> u8 {
        debug_assert!(self.cursor.peek() >= b'0' && self.cursor.peek() <= b'7');

        if self.strict_mode {
            let loc = SMLoc {
                source: self.buf_id,
                offset: self.cursor.offset() - 1,
            };
            if !self.error(loc, "octals not allowed in strict mode") {
                return 0;
            }
        }

        let mut res: u8 = self.cursor.peek() - b'0';
        self.cursor.advance(1);
        max_len -= 1;
        while max_len != 0 && self.cursor.peek() >= b'0' && self.cursor.peek() <= b'7' {
            res = (res << 3) + (self.cursor.peek() - b'0');
            self.cursor.advance(1);
            max_len -= 1;
        }

        res
    }

    /// Consume a braced code point escape `{HHHH}` (the cursor is on `{`).
    /// Port of `consumeBracedCodePoint` (JSLexer.cpp:1355-1428). Reproduces the
    /// empty / invalid-character / too-large / non-terminated error paths and
    /// the `failed` flag + `error_on_fail` gating.
    pub(crate) fn consume_braced_code_point(&mut self, error_on_fail: bool) -> Option<u32> {
        debug_assert!(self.cursor.peek() == b'{', "braced codepoint must begin with {{");
        self.cursor.advance(1);
        let start = self.cur_loc();
        let start_offset = self.cursor.offset();

        // Set to true if we failed to get a code point that is in bounds or saw
        // an invalid character.
        let mut failed = false;

        // Loop until we hit the } or eof, max out the value, or see an invalid
        // char.
        let mut cp: u32 = 0;
        while self.cursor.peek() != b'}' {
            let raw = self.cursor.peek();
            let ch_val: u32;
            if (b'0'..=b'9').contains(&raw) {
                ch_val = (raw - b'0') as u32;
            } else if (b'a'..=b'f').contains(&raw) {
                ch_val = (raw - (b'a' - 10)) as u32;
            } else if (b'A'..=b'F').contains(&raw) {
                ch_val = (raw - (b'A' - 10)) as u32;
            } else {
                // The only way this can be the end of the buffer is if this is a
                // \0. Check if this is the end of the buffer, else continue so
                // that we may report more errors after this braced code point.
                if self.cursor.at_end() {
                    if !failed && error_on_fail {
                        self.error(start, "non-terminated unicode codepoint escape");
                    }
                    return None;
                }
                // Invalid character, set the failed flag and continue.
                if !failed && error_on_fail {
                    let loc = self.cur_loc();
                    if !self.error(loc, "invalid character in unicode codepoint escape") {
                        return None;
                    }
                }
                failed = true;
                self.cursor.advance(1);
                continue;
            }
            cp = (cp << 4) + ch_val;
            if cp > UNICODE_MAX_VALUE {
                // Number grew too big, set the failed flag and continue.
                if !failed && error_on_fail {
                    if !self.error(start, "unicode codepoint escape is too large") {
                        return None;
                    }
                }
                failed = true;
            }
            self.cursor.advance(1);
        }

        debug_assert!(
            !self.cursor.at_end(),
            "bufferEnd_ should cause early return"
        );

        // An empty escape sequence is invalid.
        if self.cursor.offset() == start_offset {
            if !failed && error_on_fail {
                if !self.error(start, "empty unicode codepoint escape") {
                    return None;
                }
            }
            failed = true;
        }

        // Consume the final } and return.
        self.cursor.advance(1);
        if failed {
            None
        } else {
            Some(cp)
        }
    }

    /// Consume a `\u`/`\u{}` escape (the cursor is on `\`). Port of
    /// `consumeUnicodeEscape` (JSLexer.cpp:1159-1190). On error reports a
    /// diagnostic and returns `UNICODE_REPLACEMENT_CHARACTER`.
    pub(crate) fn consume_unicode_escape(&mut self) -> u32 {
        debug_assert!(self.cursor.peek() == b'\\');
        let backslash_offset = self.cursor.offset();
        self.cursor.advance(1);

        if self.cursor.peek() != b'u' {
            let range = SMRange {
                start: SMLoc {
                    source: self.buf_id,
                    offset: backslash_offset,
                },
                end: SMLoc {
                    source: self.buf_id,
                    offset: backslash_offset + 2,
                },
            };
            self.error_range(range, "invalid Unicode escape");
            return UNICODE_REPLACEMENT_CHARACTER;
        }
        self.cursor.advance(1);

        if self.cursor.peek() == b'{' {
            return match self.consume_braced_code_point(true) {
                // consumeBracedCodePoint has reported an error.
                None => UNICODE_REPLACEMENT_CHARACTER,
                Some(cp) => cp,
            };
        }

        match self.consume_hex(4, true) {
            None => UNICODE_REPLACEMENT_CHARACTER,
            // We don't need to check for valid UTF-16. JavaScript allows invalid
            // surrogate pairs, so we just encode every UTF-16 code into a UTF-8
            // sequence, even though theoretically it is not a valid UTF-8.
            Some(cp) => cp,
        }
    }

    /// Optionally consume a `\u`/`\u{}` escape: on ANY failure, reset the cursor
    /// to the start and return `None`. Port of `consumeUnicodeEscapeOptional`
    /// (JSLexer.cpp:1192-1226). Used by `scan_template_literal`.
    pub(crate) fn consume_unicode_escape_optional(&mut self) -> Option<u32> {
        let start = self.cursor.offset();
        debug_assert!(self.cursor.peek() == b'\\');
        self.cursor.advance(1);

        if self.cursor.peek() != b'u' {
            self.cursor.seek(start);
            return None;
        }
        self.cursor.advance(1);

        if self.cursor.peek() == b'{' {
            // Avoid reporting an error because we are consuming the escape
            // optionally.
            match self.consume_braced_code_point(false) {
                None => {
                    self.cursor.seek(start);
                    None
                }
                Some(cp) => Some(cp),
            }
        } else {
            match self.consume_hex(4, false) {
                None => {
                    self.cursor.seek(start);
                    None
                }
                Some(cp) => Some(cp),
            }
        }
    }
}
