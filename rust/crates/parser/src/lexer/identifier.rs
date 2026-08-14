//! Identifier and reserved-word scanners for the JS lexer.
//!
//! These `impl<'a> JSLexer<'a>` methods live in a child module of `lexer`, so
//! they can access the private fields of `JSLexer` declared in `lexer/mod.rs`.

use hermes_support::diag::Subsystem;
use hermes_support::location::SMRange;

use hermes_unicode::{
    is_ascii_identifier_start, is_unicode_id_continue, is_unicode_id_start,
};

use crate::token_kinds::{match_reserved_word, TokenKind};
use crate::utf8::{append_unicode_to_storage, is_utf8_start};

use super::{
    FlowMode, GrammarContext, IdMode, IdentifierMode, JSLexer, JsMode, JsxMode,
};

impl<'a> JSLexer<'a> {
    /// Try to consume the start of an identifier into `tmp_storage`. Port of
    /// `consumeIdentifierStart` (JSLexer.cpp:1228-1267). Returns true if an
    /// identifier start was consumed. Used by `scan_number` (the trailing
    /// identifier / BigInt check) and `scan_private_identifier`.
    pub(crate) fn consume_identifier_start(&mut self) -> bool {
        let c = self.cursor.peek();
        if c == b'_' || c == b'$' || ((c | 32) >= b'a' && (c | 32) <= b'z') {
            self.tmp_storage.clear();
            self.tmp_storage.push(c);
            self.cursor.advance(1);
            return true;
        }

        if c == b'\\' {
            let start_loc = self.cur_loc();
            self.tmp_storage.clear();
            let cp = self.consume_unicode_escape();
            if !is_unicode_id_start(cp) {
                self.error_range(
                    SMRange {
                        start: start_loc,
                        end: self.cur_loc(),
                    },
                    format!("Unicode escape \\u{:x}is not a valid identifier start", cp),
                );
            } else {
                append_unicode_to_storage(&mut self.tmp_storage, cp);
            }
            return true;
        }

        if !is_utf8_start(c) {
            return false;
        }

        let (cp, next) = self.cursor.peek_utf8();
        if is_unicode_id_start(cp) {
            self.tmp_storage.clear();
            append_unicode_to_storage(&mut self.tmp_storage, cp);
            self.cursor.seek(next);
            return true;
        }

        false
    }

    /// Try to consume one non-escaped identifier part into `tmp_storage`. Port
    /// of `consumeOneIdentifierPartNoEscape<Mode>` (JSLexer.cpp:1269-1290).
    #[inline]
    pub(crate) fn consume_one_identifier_part_no_escape<M: IdMode>(
        &mut self,
    ) -> bool {
        let ch = self.cursor.peek();
        if ch == b'_'
            || ch == b'$'
            || ((ch | 32) >= b'a' && (ch | 32) <= b'z')
            || ch.is_ascii_digit()
            || (M::MODE == IdentifierMode::JSX && ch == b'-')
            || (M::MODE == IdentifierMode::Flow && ch == b'@')
        {
            self.tmp_storage.push(ch);
            self.cursor.advance(1);
            return true;
        } else if is_utf8_start(ch) {
            // If we have encountered a Unicode character, we try to decode it. If
            // it can be a part of the identifier, we consume it, otherwise we
            // leave it alone.
            let (cp, next) = self.cursor.peek_utf8();
            if is_unicode_id_continue(cp) {
                append_unicode_to_storage(&mut self.tmp_storage, cp);
                self.cursor.seek(next);
                return true;
            }
        }
        false
    }

    /// Consume identifier parts into `tmp_storage`. Port of
    /// `consumeIdentifierParts<Mode>` (JSLexer.cpp:1292-1309).
    pub(crate) fn consume_identifier_parts<M: IdMode>(&mut self) {
        loop {
            // Try consuming a non-escaped identifier part. Failing that, check
            // for an escape.
            if self.consume_one_identifier_part_no_escape::<M>() {
                continue;
            } else if self.cursor.peek() == b'\\' {
                // Decode the escape.
                let start_loc = self.cur_loc();
                let cp = self.consume_unicode_escape();
                if !is_unicode_id_continue(cp) {
                    self.error_range(
                        SMRange {
                            start: start_loc,
                            end: self.cur_loc(),
                        },
                        format!(
                            "Unicode escape \\u{:x} is not a valid identifier codepoint",
                            cp
                        ),
                    );
                } else {
                    append_unicode_to_storage(&mut self.tmp_storage, cp);
                }
            } else {
                break;
            }
        }
    }

    /// Recognise a reserved word from `bytes`, applying the non-strict-mode
    /// future-reserved-word filter. Port of `scanReservedWord`
    /// (JSLexer.cpp:1865-1887).
    fn scan_reserved_word(&self, bytes: &[u8]) -> TokenKind {
        let mut rw = match_reserved_word(bytes);

        // Check for "Future reserved words" which should not be recognised in
        // non-strict mode.
        if !self.strict_mode && rw != TokenKind::identifier {
            match rw {
                TokenKind::rw_implements
                | TokenKind::rw_interface
                | TokenKind::rw_package
                | TokenKind::rw_private
                | TokenKind::rw_protected
                | TokenKind::rw_public
                | TokenKind::rw_static
                | TokenKind::rw_yield => {
                    rw = TokenKind::identifier;
                }
                _ => {}
            }
        }
        rw
    }

    /// Dispatch `scan_identifier_fast_path` with the right `IdentifierMode` for
    /// the grammar context. Port of `scanIdentifierFastPathInContext`
    /// (JSLexer.h:992-1006). Only JS mode is exercised in 1b-i.
    pub(crate) fn scan_identifier_fast_path_in_context(
        &mut self,
        start: u32,
        grammar_context: GrammarContext,
    ) {
        if grammar_context == GrammarContext::AllowJSXIdentifier {
            self.scan_identifier_fast_path::<JsxMode>(start);
        } else if grammar_context == GrammarContext::Type {
            self.scan_identifier_fast_path::<FlowMode>(start);
        } else {
            self.scan_identifier_fast_path::<JsMode>(start);
        }
    }

    /// Dispatch `scan_identifier_parts` with the right `IdentifierMode` for the
    /// grammar context. Port of `scanIdentifierPartsInContext` (JSLexer.h).
    pub(crate) fn scan_identifier_parts_in_context(
        &mut self,
        grammar_context: GrammarContext,
    ) {
        if grammar_context == GrammarContext::AllowJSXIdentifier {
            self.scan_identifier_parts::<JsxMode>();
        } else if grammar_context == GrammarContext::Type {
            self.scan_identifier_parts::<FlowMode>();
        } else {
            self.scan_identifier_parts::<JsMode>();
        }
    }

    /// Scan an identifier assuming no Unicode escapes / UTF-8 (the common case),
    /// falling back to the slow path on the first escape or UTF-8 byte. Port of
    /// `scanIdentifierFastPath<Mode>` (JSLexer.cpp:1889-1933). `start` is the
    /// byte offset of the first identifier character (the cursor is there).
    pub(crate) fn scan_identifier_fast_path<M: IdMode>(&mut self, start: u32) {
        // Quickly consume the ASCII identifier part.
        let mut end = start;
        let raw = self.cursor.raw();
        let ch = loop {
            end += 1;
            let ch = raw[end as usize];
            if !(ch == b'_'
                || ch == b'$'
                || ((ch | 32) >= b'a' && (ch | 32) <= b'z')
                || ch.is_ascii_digit()
                || (M::MODE == IdentifierMode::JSX && ch == b'-')
                || (M::MODE == IdentifierMode::Flow && ch == b'@'))
            {
                break ch;
            }
        };

        // Check whether a slow part of the identifier follows.
        if ch == b'\\' {
            // An escape. Pass the baton to the slow path.
            self.tmp_storage.clear();
            self.tmp_storage
                .extend_from_slice(&self.cursor.raw()[start as usize..end as usize]);
            self.cursor.seek(end);
            self.scan_identifier_parts::<M>();
            return;
        } else if is_utf8_start(ch) {
            // If we have encountered a Unicode character, we try to decode it. If
            // it can be a part of the identifier, we consume it, otherwise we
            // leave it alone.
            self.cursor.seek(end);
            let (cp, next) = self.cursor.peek_utf8();
            if is_unicode_id_continue(cp) {
                self.tmp_storage.clear();
                self.tmp_storage
                    .extend_from_slice(&self.cursor.raw()[start as usize..end as usize]);
                append_unicode_to_storage(&mut self.tmp_storage, cp);
                self.cursor.seek(next);
                self.scan_identifier_parts::<M>();
                return;
            }
            // Not an id-continue: the identifier ends at `end`; cursor already
            // seeked there.
        } else {
            self.cursor.seek(end);
        }

        let slice = &self.cursor.raw()[start as usize..end as usize];
        let rw = self.scan_reserved_word(slice);
        if rw != TokenKind::identifier {
            let ident = self.res_word_ident(rw);
            self.token.set_res_word(rw, ident);
        } else {
            let ident = self.strtab.atom_bytes(slice);
            self.token.set_identifier(ident);
        }
    }

    /// Scan the remaining identifier parts via the slow path (`tmp_storage`
    /// already holds the prefix). Port of `scanIdentifierParts<Mode>`
    /// (JSLexer.cpp:1935-1949). A reserved word reached through a unicode escape
    /// ALSO emits a warning.
    pub(crate) fn scan_identifier_parts<M: IdMode>(&mut self) {
        self.consume_identifier_parts::<M>();
        let rw = self.scan_reserved_word(&self.tmp_storage);
        if rw != TokenKind::identifier {
            let ident = self.res_word_ident(rw);
            self.token.set_res_word(rw, ident);
            let range = SMRange {
                start: self.token.start_loc(),
                end: self.cur_loc(),
            };
            self.sm.warning_range(
                hermes_support::diag::Warning::Misc,
                range,
                "scanning identifier with unicode escape as reserved word",
                Subsystem::Lexer,
            );
        } else {
            let ident = self.strtab.atom_bytes(self.tmp_storage.as_slice());
            self.token.set_identifier(ident);
        }
    }

    /// Scan a private identifier (the cursor is on `#`). Port of
    /// `scanPrivateIdentifier` (JSLexer.cpp:1951-1975). Returns false (and emits
    /// an "empty private identifier" error) if `#` is not followed by an
    /// identifier.
    pub(crate) fn scan_private_identifier(&mut self) -> bool {
        debug_assert!(self.cursor.peek() == b'#');

        // Skip the '#'.
        let start = self.cur_loc();
        self.cursor.advance(1);

        // Scan the actual identifier.
        if is_ascii_identifier_start(self.cursor.peek() as u32) {
            let here = self.cursor.offset();
            self.scan_identifier_fast_path::<JsMode>(here);
        } else if self.consume_identifier_start() {
            // The cursor has been updated by consume_identifier_start.
            self.scan_identifier_parts::<JsMode>();
        } else {
            self.error(start, "empty private identifier");
            return false;
        }

        // Parsed a resword or identifier.
        // Convert the TokenKind to private_identifier after the fact.
        // This avoids adding another Mode to IdentifierMode.
        let ident = self.token.get_res_word_or_identifier();
        self.token.set_private_identifier(ident);

        true
    }
}
