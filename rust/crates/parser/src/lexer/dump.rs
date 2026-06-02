//! Token dumping for the JS lexer (the `js-lexer-dump` format).
//!
//! These `impl<'a> JSLexer<'a>` methods live in a child module of `lexer`, so
//! they can access the private fields of `JSLexer` declared in `lexer/mod.rs`.

use crate::token_kinds::{variant_name, TokenKind};

use super::JSLexer;

impl<'a> JSLexer<'a> {
    /// Format the current token like the C++ `js-lexer-dump` line (without the
    /// trailing newline): `"<start> <end> <nl> <KIND>[ <field> ...]"`. Phase-1b-i
    /// emits the `ident=` field for identifiers / private identifiers / reserved
    /// words; the other literal fields land in later phases.
    pub fn dump_token(&self, out: &mut String) {
        use std::fmt::Write;
        let start = self.token.start_loc().offset;
        let end = self.token.end_loc().offset;
        let nl = if self.new_line_before_current_token {
            "nl"
        } else {
            "--"
        };
        let kind = self.token.kind();
        let _ = write!(out, "{} {} {} {}", start, end, nl, variant_name(kind));
        self.emit_fields(out, kind);
    }

    /// Emit the per-kind dump fields. Port of `js-lexer-dump.cpp:emitFields`
    /// (the identifier/reserved-word cases; other cases land in later phases).
    fn emit_fields(&self, out: &mut String, kind: TokenKind) {
        match kind {
            TokenKind::identifier => {
                out.push_str(" ident=");
                quote_bytes(out, self.strtab.bytes(self.token.get_identifier()));
            }
            TokenKind::private_identifier => {
                out.push_str(" ident=");
                quote_bytes(out, self.strtab.bytes(self.token.get_private_identifier()));
            }
            TokenKind::string_literal => {
                use std::fmt::Write;
                let _ = write!(
                    out,
                    " escapes={}",
                    if self.token.get_string_literal_contains_escapes() {
                        1
                    } else {
                        0
                    }
                );
                out.push_str(" value=");
                quote_bytes(out, self.strtab.bytes(self.token.get_string_literal()));
            }
            TokenKind::numeric_literal => {
                use std::fmt::Write;
                // Match the harness `snprintf(" bits=0x%016llx", DoubleToBits)`:
                // 16-digit, zero-padded, lowercase hex of the f64 bit pattern.
                let bits = self.token.get_numeric_literal().to_bits();
                let _ = write!(out, " bits=0x{:016x}", bits);
            }
            TokenKind::bigint_literal => {
                out.push_str(" value=");
                quote_bytes(out, self.strtab.bytes(self.token.get_bigint_literal()));
                out.push_str(" raw=");
                quote_bytes(
                    out,
                    self.strtab.bytes(self.token.get_bigint_literal_raw_value()),
                );
            }
            TokenKind::regexp_literal => {
                let re = self.token.get_regexp_literal();
                out.push_str(" body=");
                quote_bytes(out, self.strtab.bytes(re.body()));
                out.push_str(" flags=");
                quote_bytes(out, self.strtab.bytes(re.flags()));
            }
            TokenKind::no_substitution_template
            | TokenKind::template_head
            | TokenKind::template_middle
            | TokenKind::template_tail => {
                out.push_str(" cooked=");
                match self.token.get_template_value() {
                    Some(cooked) => quote_bytes(out, self.strtab.bytes(cooked)),
                    None => out.push_str("null"),
                }
                out.push_str(" raw=");
                quote_bytes(out, self.strtab.bytes(self.token.get_template_raw_value()));
            }
            TokenKind::jsx_text => {
                out.push_str(" value=");
                quote_bytes(out, self.strtab.bytes(self.token.get_jsx_text_value()));
                out.push_str(" raw=");
                quote_bytes(out, self.strtab.bytes(self.token.get_jsx_text_raw()));
            }
            _ => {
                // Reserved words: emit the identifier string.
                if kind.is_res_word() {
                    out.push_str(" ident=");
                    quote_bytes(out, self.strtab.bytes(self.token.get_res_word_identifier()));
                }
                // Punctuators and eof: no extra fields.
            }
        }
    }
}

/// Emit `bytes` quoted per the `js-lexer-dump` `Q()` spec into `out`. Port of
/// `quoteBytes` (js-lexer-dump.cpp:91-115): wrap in double quotes; `"`->`\"`,
/// `\`->`\\`, `\n`->`\n`, `\t`->`\t`, `\r`->`\r`; printable ASCII
/// `0x20..=0x7e` literal; every other byte as lowercase `\xHH`.
fn quote_bytes(out: &mut String, bytes: &[u8]) {
    out.push('"');
    for &c in bytes {
        match c {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            b'\r' => out.push_str("\\r"),
            0x20..=0x7e => out.push(c as char),
            _ => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                out.push_str("\\x");
                out.push(HEX[(c >> 4) as usize & 0xf] as char);
                out.push(HEX[c as usize & 0xf] as char);
            }
        }
    }
    out.push('"');
}
