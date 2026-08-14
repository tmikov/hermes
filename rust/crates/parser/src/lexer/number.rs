//! Numeric (and BigInt) literal scanner for the JS lexer.
//!
//! These `impl<'a> JSLexer<'a>` methods live in a child module of `lexer`, so
//! they can access the private fields of `JSLexer` declared in `lexer/mod.rs`.

use hermes_support::diag::Subsystem;
use hermes_support::location::{SMLoc, SMRange};

use crate::number;

use super::{is_ascii_digit, GrammarContext, JSLexer, JsMode};

impl<'a> JSLexer<'a> {
    /// Scan a numeric literal (or BigInt). Port of `JSLexer::scanNumber`
    /// (JSLexer.cpp:1573-1856). The cursor is positioned at the first character
    /// of the number (a digit, or `.` for the `.NNN` form). On return the token
    /// is set to `numeric_literal` or `bigint_literal`.
    pub(crate) fn scan_number(&mut self, grammar_context: GrammarContext) {
        // A somewhat ugly state machine for scanning a number

        let mut radix: u32 = 10;
        let mut real = false;
        let mut ok = true;
        // Byte offset of the token start (incl. any radix prefix). Port of
        // `rawStart`.
        let raw_start = self.cursor.offset();
        // Byte offset of the first significant digit. For radix-prefixed forms
        // this is moved past the prefix. Port of `start`.
        let mut start = self.cursor.offset();

        // True when we encounter the numeric literal separator: '_'.
        let mut seen_separator = false;

        // True when we encounter a legacy octal number (starts with '0').
        let mut legacy_octal = false;

        // A label-less reimplementation of the C++ `goto` state machine. The
        // `Phase` enum records which state to enter next; `end` is reached either
        // by falling through the integer loop or via the fraction/exponent
        // states.
        enum Phase {
            IntegerLoop,
            Fraction,
            Exponent,
            End,
        }
        let mut phase: Phase;

        // Detect the radix
        if self.cursor.peek() == b'0' {
            let c1 = self.cursor.peek_at(1);
            if (c1 | 32) == b'x' {
                radix = 16;
                self.cursor.advance(2);
                start += 2;
                phase = Phase::IntegerLoop;
            } else if (c1 | 32) == b'o' {
                radix = 8;
                self.cursor.advance(2);
                start += 2;
                phase = Phase::IntegerLoop;
            } else if (c1 | 32) == b'b' {
                radix = 2;
                self.cursor.advance(2);
                start += 2;
                phase = Phase::IntegerLoop;
            } else if c1 == b'.' {
                self.cursor.advance(2);
                phase = Phase::Fraction;
            } else if (c1 | 32) == b'e' {
                self.cursor.advance(2);
                phase = Phase::Exponent;
            } else {
                radix = 8;
                legacy_octal = true;
                self.cursor.advance(1);
                phase = Phase::IntegerLoop;
            }
        } else {
            phase = Phase::IntegerLoop;
        }

        if let Phase::IntegerLoop = phase {
            while is_ascii_digit(self.cursor.peek())
                || (radix == 16
                    && (self.cursor.peek() | 32) >= b'a'
                    && (self.cursor.peek() | 32) <= b'f')
                || self.cursor.peek() == b'_'
            {
                seen_separator |= self.cursor.peek() == b'_';
                self.cursor.advance(1);
            }

            phase = Phase::End;
            if radix == 10 || legacy_octal {
                // It is not necessarily an integer.
                // We could have interpreted as legacyOctal initially but will
                // have to change to decimal later.
                if self.cursor.peek() == b'.' {
                    self.cursor.advance(1);
                    phase = Phase::Fraction;
                } else if (self.cursor.peek() | 32) == b'e' {
                    self.cursor.advance(1);
                    phase = Phase::Exponent;
                }
            }
        }

        if let Phase::Fraction = phase {
            // We arrive here after we have consumed the decimal dot ".".
            real = true;
            while is_ascii_digit(self.cursor.peek()) || self.cursor.peek() == b'_' {
                seen_separator |= self.cursor.peek() == b'_';
                self.cursor.advance(1);
            }

            if (self.cursor.peek() | 32) == b'e' {
                self.cursor.advance(1);
                phase = Phase::Exponent;
            } else {
                phase = Phase::End;
            }
        }

        if let Phase::Exponent = phase {
            // We arrive here after we have consumed the exponent char 'e' or 'E'.
            real = true;
            if self.cursor.peek() == b'+' || self.cursor.peek() == b'-' {
                self.cursor.advance(1);
            }
            if is_ascii_digit(self.cursor.peek()) {
                loop {
                    seen_separator |= self.cursor.peek() == b'_';
                    self.cursor.advance(1);
                    if !(is_ascii_digit(self.cursor.peek()) || self.cursor.peek() == b'_') {
                        break;
                    }
                }
            } else {
                ok = false;
            }
            phase = Phase::End;
        }

        debug_assert!(matches!(phase, Phase::End));

        // We arrive here after we have consumed all we can from the number. Now,
        // as per the spec, we consume a sequence of identifier characters if they
        // follow directly, which means the number is invalid if it's not BigInt.
        if self.consume_identifier_start() {
            self.consume_identifier_parts::<JsMode>();

            // raw == the full literal source [rawStart, curCharPtr_).
            let cur = self.cursor.offset();
            let raw = self.cursor.raw()[raw_start as usize..cur as usize].to_vec();
            if ok
                && !real
                && (!legacy_octal || raw == b"0n")
                && self.tmp_storage == b"n"
            {
                debug_assert!(
                    cur > start,
                    "Must consume at least the trailing n."
                );
                // digits == [start, curCharPtr_ - 1) (drop the trailing 'n').
                let digits = self.cursor.raw()[start as usize..(cur - 1) as usize].to_vec();
                // Use parseIntWithRadixDigits to validate the bigint literal's
                // digits. The digits themselves can be ignored, since we're only
                // interested in whether the string was parsed correctly.
                if !digits.is_empty()
                    && number::parse_int_with_radix_digits(
                        &digits,
                        radix,
                        /* allow_sep */ true,
                        |_| {},
                    )
                {
                    // This is a BigInt.
                    // ESTree spec:
                    // bigint property is the string representation of the BigInt
                    // value. It must contain only decimal digits and not include
                    // numeric separators (_) or the suffix n.
                    // Filter out the characters we don't want.
                    // Drop the last character from `raw` because that's the 'n',
                    // and skip over all '_'.
                    self.tmp_storage.clear();
                    for &c in &raw[..raw.len() - 1] {
                        if c != b'_' {
                            self.tmp_storage.push(c);
                        }
                    }
                    let value = self.get_string_literal(self.tmp_storage.as_slice());
                    let raw_atom = self.get_string_literal(raw.as_slice());
                    self.token.set_bigint_literal(value, raw_atom);
                    return;
                }

                // This is a BigInt with invalid digits; fail.
            }

            ok = false;
        }

        let cur = self.cursor.offset();
        let start_loc = self.token.start_loc();
        // Every arm of the chain below assigns `val`; we call
        // `set_numeric_literal(val)` exactly once at the single exit point at
        // the end, mirroring the C++ `done:` label. The C++ `goto done` early
        // exits (the error-limit cases that set `val = NaN`) are reproduced
        // with labeled-block breaks (`'done`) that short-circuit the rest of
        // their arm but still fall through to the final single set.
        let val: f64;

        if !ok {
            self.error_range(
                SMRange {
                    start: start_loc,
                    end: self.cur_loc(),
                },
                "invalid numeric literal",
            );
            val = f64::NAN;
        } else if !real
            && radix == 10
            && (cur - start) <= 9
            && !seen_separator
        {
            // If this is a decimal integer of at most 9 digits (log10(2**31-1),
            // it can fit in a 32-bit integer. Use a faster conversion.
            let bytes = self.cursor.raw();
            let mut idx = start as usize;
            let mut ival: i32 = (bytes[idx] - b'0') as i32;
            idx += 1;
            while idx != cur as usize {
                ival = ival * 10 + (bytes[idx] - b'0') as i32;
                idx += 1;
            }
            val = ival as f64;
        } else if real || radix == 10 {
            // Labeled block: the C++ `goto done` error-limit early exits set
            // `val = NaN` and break straight to the final single set.
            val = 'done: {
            if legacy_octal {
                if self.strict_mode || grammar_context == GrammarContext::Type {
                    if !self.error_range(
                        SMRange {
                            start: start_loc,
                            end: self.cur_loc(),
                        },
                        "Decimals with leading zeros are not allowed in strict mode",
                    ) {
                        break 'done f64::NAN;
                    }
                } else {
                    // Check to see if we can actually scan this as radix 10.
                    // Non-integer numbers must be in base 10, otherwise we error.
                    self.update_legacy_octal_radix(start, &mut radix);
                    if radix != 10 {
                        if !self.error_range(
                            SMRange {
                                start: start_loc,
                                end: self.cur_loc(),
                            },
                            "Octal numeric literals must be integers",
                        ) {
                            break 'done f64::NAN;
                        }
                    }
                }
            }

            let mut buf: Vec<u8> = Vec::with_capacity((cur - start) as usize + 1);
            // Own the digit slice so the per-character checks below can borrow
            // `self` mutably for error reporting (the C++ indexes the live
            // pointer; the owned copy is equivalent because the buffer is
            // immutable).
            let bytes = self.cursor.raw()[start as usize..(cur + 1) as usize].to_vec();
            if seen_separator {
                let mut it = 0usize;
                let nbytes = (cur - start) as usize;
                while it != nbytes {
                    let c = bytes[it];
                    if c != b'_' {
                        buf.push(c);
                    } else {
                        // Check to ensure that '_' is surrounded by digits.
                        // This is safe because the source buffer is
                        // zero-terminated and we know that the numeric literal
                        // didn't start with '_'. Note that we could have a 0b_11
                        // literal, but we'd still fail properly because of the
                        // radix==16 check.
                        let prev = bytes[it - 1];
                        let next = bytes[it + 1];
                        if !is_ascii_digit(prev)
                            && !(radix == 16
                                && b'a' <= (prev | 32)
                                && (prev | 32) <= b'f')
                        {
                            self.error_range(
                                SMRange {
                                    start: start_loc,
                                    end: self.cur_loc(),
                                },
                                "numeric separator must come after a digit",
                            );
                        } else if !is_ascii_digit(next)
                            && !(radix == 16
                                && b'a' <= (next | 32)
                                && (next | 32) <= b'f')
                        {
                            self.error_range(
                                SMRange {
                                    start: start_loc,
                                    end: self.cur_loc(),
                                },
                                "numeric separator must come before a digit",
                            );
                        }
                    }
                    it += 1;
                }
            } else {
                buf.extend_from_slice(&bytes[0..(cur - start) as usize]);
            }
            match number::str_to_double(&buf) {
                Some(v) => v,
                None => {
                    self.error_range(
                        SMRange {
                            start: start_loc,
                            end: self.cur_loc(),
                        },
                        "invalid numeric literal",
                    );
                    f64::NAN
                }
            }
            };
        } else {
            // Labeled block: the C++ `goto done` error-limit early exit sets
            // `val = NaN` and breaks straight to the final single set.
            val = 'done: {
            if legacy_octal
                && (self.strict_mode || grammar_context == GrammarContext::Type)
                && (cur - start) > 1
            {
                if !self.error_range(
                    SMRange {
                        start: start_loc,
                        end: self.cur_loc(),
                    },
                    "Octal literals must use '0o' in strict mode",
                ) {
                    break 'done f64::NAN;
                }
            }

            // Handle the zero-radix case. This could only happen with radix 16
            // because otherwise start wouldn't have been changed.
            if cur == start {
                let prefix = self.cursor.raw()[(start - 2) as usize..start as usize].to_vec();
                self.error_range(
                    SMRange {
                        start: start_loc,
                        end: self.cur_loc(),
                    },
                    format!(
                        "No digits after {}",
                        String::from_utf8_lossy(&prefix)
                    ),
                );
                f64::NAN
            } else {
                // Parse the rest of the number:
                if legacy_octal {
                    self.update_legacy_octal_radix(start, &mut radix);
                    // LegacyOctalLikeDecimalIntegerLiteral cannot contain
                    // separators.
                    if seen_separator {
                        self.error_range(
                            SMRange {
                                start: start_loc,
                                end: self.cur_loc(),
                            },
                            "Numeric separator cannot be used in literal after leading 0",
                        );
                    }
                }
                let digits = self.cursor.raw()[start as usize..cur as usize].to_vec();
                match number::parse_int_with_radix(&digits, radix, /* allow_sep */ true) {
                    Some(v) => v,
                    None => {
                        self.error_range(
                            SMRange {
                                start: start_loc,
                                end: self.cur_loc(),
                            },
                            "invalid integer literal",
                        );
                        f64::NAN
                    }
                }
            }
            };
        }

        // Single exit (C++ `done:`): set the numeric literal value exactly once.
        self.token.set_numeric_literal(val);
    }

    /// ES6.0 B.1.1: if we encounter a "legacy" octal number (starting with a
    /// '0') but the integer contains '8' or '9' we interpret it as decimal.
    /// Port of the `updateLegacyOctalRadix` lambda inside `scanNumber`
    /// (JSLexer.cpp:1717-1736). `start` is the byte offset of the first digit;
    /// `radix` is updated to 10 (with a warning) on an 8/9 digit.
    fn update_legacy_octal_radix(&mut self, start: u32, radix: &mut u32) {
        let cur = self.cursor.offset();
        let bytes = self.cursor.raw();
        let mut scan = start as usize;
        while scan != cur as usize {
            let c = bytes[scan];
            if c == b'.' || c == b'e' {
                break;
            }
            if c >= b'8' && c != b'_' {
                let range = SMRange {
                    start: self.token.start_loc(),
                    end: SMLoc {
                        source: self.buf_id,
                        offset: cur,
                    },
                };
                self.sm.warning_range(
                    hermes_support::diag::Warning::Misc,
                    range,
                    "Numeric literal starts with 0 but contains an 8 or 9 digit. \
                     Interpreting as decimal (not octal).",
                    Subsystem::Lexer,
                );
                *radix = 10;
                break;
            }
            scan += 1;
        }
    }
}
