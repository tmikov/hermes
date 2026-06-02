//! Numeric-literal conversion primitives for the JS lexer, ported from
//! include/hermes/Support/Conversions.h. The decimal/real path uses Rust std's
//! correctly-rounded `str::parse::<f64>()` (the same fast_float algorithm the
//! C++ lexer uses) — no FFI, no third-party crate.
//!
//! Public API:
//! - [`parse_int_with_radix_digits`] — digit-by-digit radix parser (callback style).
//! - [`parse_int_with_radix`] — full integer-radix parse with power-of-2 rounding path.
//! - [`str_to_double`] — decimal/real path: pure-Rust, bit-identical to `fastStrToDouble`.

/// Takes a letter (a-z or A-Z) and makes it lowercase.
/// Port of `charLetterToLower` (Conversions.h:160).
#[inline]
fn char_letter_to_lower(c: u8) -> u8 {
    c | 32
}

/// Takes a non-empty string (without the leading "0x" if hex) and parses it
/// as radix `radix`, calling `digit` with the value of each digit,
/// going from left to right.
/// `allow_sep`: when true, allow '_' as a separator and ignore it when parsing.
/// Returns true if the string was successfully parsed, false otherwise.
/// Port of `parseIntWithRadixDigits` (Conversions.h:166).
pub fn parse_int_with_radix_digits(
    bytes: &[u8],
    radix: u32,
    allow_sep: bool,
    mut digit: impl FnMut(u8),
) -> bool {
    debug_assert!((2..=36).contains(&radix), "Invalid radix passed to parseIntWithRadix");
    debug_assert!(!bytes.is_empty(), "Empty string");
    // Use i32 for arithmetic so `radix - 10` does not underflow for radix < 10.
    let radix = radix as i32;
    for (i, &c) in bytes.iter().enumerate() {
        let c_low = char_letter_to_lower(c);
        if c >= b'0' && c <= b'9' && (c as i32) < b'0' as i32 + radix {
            digit(c - b'0');
        } else if c_low >= b'a' && (c_low as i32) < b'a' as i32 + radix - 10 {
            digit(c_low - b'a' + 0xa);
        } else if allow_sep && c == b'_' {
            // Ensure the '_' is in a valid location.
            // It can only be between two existing digits.
            if i == 0 || i == bytes.len() - 1 {
                return false;
            }
            // Note that the previous character must not be '_' if the current
            // character is '_', because we would have returned false.
            // So just check if the next character is '_'.
            if bytes[i + 1] == b'_' {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

/// Takes a non-empty string (without the leading "0x" if hex) and parses it
/// as radix `radix`.
/// `allow_sep`: when true, allow '_' as a separator and ignore it when parsing.
/// Returns the f64 that results on success, or None on error.
/// Port of `parseIntWithRadix` (Conversions.h:204), including the >2^53
/// power-of-two bit-by-bit rounding path (lines 222–328).
pub fn parse_int_with_radix(bytes: &[u8], radix: u32, allow_sep: bool) -> Option<f64> {
    let mut result: f64 = 0.0;
    let success = parse_int_with_radix_digits(bytes, radix, allow_sep, |d| {
        result *= radix as f64;
        result += d as f64;
    });
    if !success {
        return None;
    }

    // The largest value that fits in the 53-bit mantissa (2**53).
    const MAX_MANTISSA: f64 = 9007199254740992.0;
    if result >= MAX_MANTISSA && radix.is_power_of_two() {
        // If the result is too high, manually reconstruct the double if
        // the radix is 2, 4, 8, 16, 32.
        // Go through the digits bit by bit, and manually round when necessary.
        result = 0.0;

        // Keep track of how far along parsing is using this enum.
        #[derive(PartialEq)]
        enum Mode {
            LeadingZero,    // Haven't seen a set bit yet.
            Mantissa,       // Lower bits that allow exact representation.
            ExpLowBit,      // Lowest bit of the exponent (determine rounding).
            ExpLeadingZero, // Zeros in the exponent.
            Exponent,       // Seen a set bit in the exponent.
        }

        let mut remaining_mantissa: usize = 53;
        let mut exp_factor: f64 = 0.0;
        let mut cur_digit: usize = 0;

        let mut last_mantissa_bit = false;
        let mut lowest_exponent_bit = false;

        let mut cur_mode = Mode::LeadingZero;
        // Plain iterator (matches the C++ `auto itr = str.begin()`); we only ever
        // advance with `next()`.
        let mut itr = bytes.iter();
        let mut bit_mask: u32 = 0;
        loop {
            if bit_mask == 0 {
                // Only need to do this check every log2(radix) iterations.
                match itr.next() {
                    None => break,
                    Some(&c) => {
                        let c = c as char;
                        if allow_sep && c == '_' {
                            // Skip separators; we already validated them.
                            continue;
                        }
                        let c_low = char_letter_to_lower(c as u8);
                        if c >= '0' && c <= '9' {
                            cur_digit = (c as u8 - b'0') as usize;
                        } else {
                            // Must be valid, else we would have returned None on first pass.
                            debug_assert!(
                                c_low >= b'a' && (c_low as i32) < b'a' as i32 + radix as i32 - 10
                            );
                            cur_digit = (c_low - b'a' + 0xa) as usize;
                        }
                        // Reset bitmask to look at the first bit.
                        bit_mask = radix >> 1;
                    }
                }
            }
            let cur_bit = (cur_digit as u32 & bit_mask) != 0;
            bit_mask >>= 1;

            match cur_mode {
                Mode::LeadingZero => {
                    // Go through the string until we hit a nonzero bit.
                    if cur_bit {
                        remaining_mantissa -= 1;
                        result = 1.0;
                        // No more leading zeros.
                        cur_mode = Mode::Mantissa;
                    }
                }
                Mode::Mantissa => {
                    // Read into the lower bits of the mantissa (plain binary).
                    result *= 2.0;
                    result += cur_bit as u8 as f64;
                    remaining_mantissa -= 1;
                    if remaining_mantissa == 0 {
                        // Out of bits, set the last bit and go to the next curMode.
                        last_mantissa_bit = cur_bit;
                        cur_mode = Mode::ExpLowBit;
                    }
                }
                Mode::ExpLowBit => {
                    lowest_exponent_bit = cur_bit;
                    exp_factor = 2.0;
                    cur_mode = Mode::ExpLeadingZero;
                }
                Mode::ExpLeadingZero => {
                    if cur_bit {
                        cur_mode = Mode::Exponent;
                    }
                    exp_factor *= 2.0;
                }
                Mode::Exponent => {
                    exp_factor *= 2.0;
                }
            }
        }
        match cur_mode {
            Mode::LeadingZero | Mode::Mantissa | Mode::ExpLowBit => {
                // Nothing to do here, already read those in.
            }
            Mode::ExpLeadingZero => {
                // Rounding up.
                result += (lowest_exponent_bit && last_mantissa_bit) as u8 as f64;
                result *= exp_factor;
            }
            Mode::Exponent => {
                // Rounding up.
                result += lowest_exponent_bit as u8 as f64;
                result *= exp_factor;
            }
        }
    }
    Some(result)
}

/// Parse a cleaned decimal/real numeric buffer (only `[0-9.eE+-]`, separators
/// already stripped) to an f64. Returns the value if the WHOLE buffer parses,
/// or None on invalid input — mirroring `fastStrToDouble`'s "consume all or
/// fail" contract. Out-of-range inputs parse to +/-inf or 0.0 (as fast_float and
/// Rust std both do). Rust std's parser is the same correctly-rounded algorithm
/// as the lexer's `fast_float`, so results are bit-identical.
pub fn str_to_double(bytes: &[u8]) -> Option<f64> {
    // The buffer is pure ASCII; from_utf8 cannot fail, but handle defensively.
    let s = std::str::from_utf8(bytes).ok()?;
    s.parse::<f64>().ok()
}

#[cfg(test)]
mod int_tests {
    use super::*;

    #[test]
    fn small_exact() {
        assert_eq!(parse_int_with_radix(b"ff", 16, true), Some(255.0));
        assert_eq!(parse_int_with_radix(b"777", 8, true), Some(511.0));
        assert_eq!(parse_int_with_radix(b"1010", 2, true), Some(10.0));
        assert_eq!(parse_int_with_radix(b"123", 10, true), Some(123.0));
        assert_eq!(parse_int_with_radix(b"z", 36, true), Some(35.0));
        // Letters are rejected for radix <= 10 (no u32 underflow on radix-10).
        assert_eq!(parse_int_with_radix(b"a", 10, true), None);
        assert_eq!(parse_int_with_radix(b"8", 8, true), None);
    }

    #[test]
    fn separators() {
        assert_eq!(parse_int_with_radix(b"1_000", 10, true), Some(1000.0));
        assert_eq!(
            parse_int_with_radix(b"dead_beef", 16, true),
            Some(0xdeadbeef_u32 as f64)
        );
        assert_eq!(parse_int_with_radix(b"_1", 10, true), None); // leading
        assert_eq!(parse_int_with_radix(b"1_", 10, true), None); // trailing
        assert_eq!(parse_int_with_radix(b"1__2", 10, true), None); // double
        // When separators are disallowed, '_' is just an invalid digit.
        assert_eq!(parse_int_with_radix(b"1_0", 10, false), None);
    }

    #[test]
    fn invalid() {
        assert_eq!(parse_int_with_radix(b"xyz", 16, true), None);
        assert_eq!(parse_int_with_radix(b"12.3", 10, true), None);
    }

    // The power-of-2 high-precision path (result >= 2^53) must produce the
    // correctly-rounded f64. Rust's `u128 as f64` is round-to-nearest-even, an
    // independent correctly-rounded oracle for any value that fits in u128.
    #[test]
    fn large_power_of_two_rounding_matches_u128_oracle() {
        let cases: &[(&[u8], u32)] = &[
            (b"20000000000001", 16),   // 2^53 + 1 region
            (b"1fffffffffffff", 16),   // 2^53 - 1 (exact)
            (b"ffffffffffffffff", 16), // u64::MAX
            (b"123456789abcdef0123", 16), // > 2^64, still < 2^128
            (b"777777777777777777777", 8), // large octal
            (b"1111111111111111111111111111111111111111111111111111111", 2),
            (b"20000000000000", 16),        // exactly 2^53 (>= boundary triggers the path)
            (b"33333333333333333333333333333", 4), // large radix-4
            (b"vvvvvvvvvvvv", 32),          // large radix-32 (v = 31)
        ];
        for &(s, radix) in cases {
            let txt = std::str::from_utf8(s).unwrap();
            let expected = u128::from_str_radix(txt, radix).unwrap() as f64;
            assert_eq!(
                parse_int_with_radix(s, radix, true),
                Some(expected),
                "mismatch for {txt} radix {radix}"
            );
        }
    }

    // Radix 10 is NOT a power of two, so even above 2^53 it uses the plain f64
    // accumulation (no bit-by-bit precision path). This is a sanity check that the
    // accumulation agrees with the u128->f64 cast for such a value (both round to
    // 2^53), not a test of the precision path.
    #[test]
    fn large_decimal() {
        assert_eq!(
            parse_int_with_radix(b"9007199254740993", 10, true),
            Some(9007199254740993u128 as f64)
        );
    }
}

#[cfg(test)]
mod double_tests {
    use super::*;

    fn bits(v: f64) -> u64 {
        v.to_bits()
    }

    #[test]
    fn known_bit_patterns() {
        // These mirror the js-lexer-dump oracle's `bits=` output.
        assert_eq!(str_to_double(b"5").map(bits), Some(0x4014000000000000));
        assert_eq!(str_to_double(b"0.1").map(bits), Some(0x3fb999999999999a));
        assert_eq!(str_to_double(b"255").map(bits), Some(0x406fe00000000000));
        // (5, 0.1, 255 bit patterns were confirmed against the real C++ js-lexer-dump.)
        // These two cross-check delegation to the std parser:
        assert_eq!(str_to_double(b"1e10").map(bits), Some(1e10f64.to_bits()));
        assert_eq!(str_to_double(b"12.5").map(bits), Some(12.5f64.to_bits()));
    }

    #[test]
    fn must_consume_all() {
        assert_eq!(str_to_double(b"12x"), None);
        assert_eq!(str_to_double(b""), None);
        assert_eq!(str_to_double(b"1.2.3"), None);
    }

    #[test]
    fn leading_plus_and_exponent() {
        assert_eq!(str_to_double(b"+5").map(bits), Some(5.0f64.to_bits()));
        assert_eq!(str_to_double(b"5e+3").map(bits), Some(5000.0f64.to_bits()));
        assert_eq!(str_to_double(b"5E-3").map(bits), Some(0.005f64.to_bits()));
    }

    #[test]
    fn out_of_range() {
        // Overflow -> +inf; underflow -> 0.0 (matches fast_float ignoring out-of-range).
        assert_eq!(str_to_double(b"1e400"), Some(f64::INFINITY));
        assert_eq!(str_to_double(b"1e-400"), Some(0.0));
    }
}
