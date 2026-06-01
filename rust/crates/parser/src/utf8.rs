//! UTF-8 decode helpers, ported from include/hermes/Support/UTF8.h (decode side).
//!
//! These mirror the inline classifiers and `_decodeUTF8SlowPath`/`decodeUTF8`
//! from `Support/UTF8.h`. The C++ uses an advancing `const char *&from`; here we
//! use a slice plus a `&mut usize` index, so the raw-pointer parity lives only
//! in `cursor.rs`. The lexer always passes the NUL-terminated buffer, so an
//! out-of-range continuation read sees `0x00` (a non-continuation byte) and is
//! correctly rejected; we also guard indexes against `bytes.len()` defensively.

use unicode::{
    UNICODE_REPLACEMENT_CHARACTER, UNICODE_SURROGATE_FIRST, UNICODE_SURROGATE_LAST,
    UNICODE_MAX_VALUE,
};

/// First byte of the UTF-8 encoding of U+2028/U+2029 (e2 80 a8/a9).
pub const UTF8_LINE_TERMINATOR_CHAR0: u8 = 0xe2;

/// Check whether a byte is a regular ASCII or a UTF8 starting byte.
/// \return true if it is UTF8 starting byte.
#[inline]
pub fn is_utf8_start(ch: u8) -> bool {
    (ch & 0x80) != 0
}

/// \return true if this is a UTF-8 leading byte.
#[inline]
pub fn is_utf8_leading_byte(ch: u8) -> bool {
    (ch & 0xC0) == 0xC0
}

/// \return true if this is a UTF-8 continuation byte, or in other words, this
/// is a byte in the "middle" of a UTF-8 codepoint.
#[inline]
pub fn is_utf8_continuation_byte(ch: u8) -> bool {
    (ch & 0xC0) == 0x80
}

/// \return true if `bytes` starts with the UTF-8 encoding of U+2028 or U+2029.
/// `bytes[0]` is assumed to be UTF8_LINE_TERMINATOR_CHAR0 (the caller checked).
///
/// Line separator   UTF8 encoded is      : e2 80 a8
/// Paragraph separator   UTF8 encoded is : e2 80 a9
#[inline]
pub fn match_unicode_line_terminator_offset1(bytes: &[u8]) -> bool {
    bytes.len() >= 3
        && bytes[0] == 0xe2
        && bytes[1] == 0x80
        && (bytes[2] == 0xa8 || bytes[2] == 0xa9)
}

/// Read `bytes[i]`, or `0` if `i` is out of range. The lexer's buffer is
/// NUL-terminated, so the only way to read past the end here is in unit tests
/// of malformed/truncated sequences, where `0` (a non-continuation byte)
/// reproduces the buffer's NUL terminator and the same rejection behavior.
#[inline]
fn at(bytes: &[u8], i: usize) -> u32 {
    bytes.get(i).copied().unwrap_or(0) as u32
}

/// Decode a sequence of UTF8 encoded bytes when it is known that the first byte
/// is a start of a UTF8 sequence. Port of `_decodeUTF8SlowPath` (UTF8.h:77-162),
/// reading `bytes` from `*i` and advancing `*i` past the consumed bytes.
/// On malformed input it invokes `error` and returns the replacement character.
///
/// \tparam ALLOW_SURROGATES when false, values in the surrogate range are
///     reported as errors.
pub fn decode_utf8_slow_path<const ALLOW_SURROGATES: bool>(
    bytes: &[u8],
    i: &mut usize,
    mut error: impl FnMut(&str),
) -> u32 {
    let ch = at(bytes, *i);
    let result: u32;

    debug_assert!(is_utf8_start(ch as u8));

    if (ch & 0xE0) == 0xC0 {
        let ch1 = at(bytes, *i + 1);
        if (ch1 & 0xC0) != 0x80 {
            *i += 1;
            error("Invalid UTF-8 continuation byte");
            return UNICODE_REPLACEMENT_CHARACTER;
        }

        *i += 2;
        result = ((ch & 0x1F) << 6) | (ch1 & 0x3F);
        if result <= 0x7F {
            error("Non-canonical UTF-8 encoding");
            return UNICODE_REPLACEMENT_CHARACTER;
        }
    } else if (ch & 0xF0) == 0xE0 {
        let ch1 = at(bytes, *i + 1);
        if (ch1 & 0x40) != 0 || (ch1 & 0x80) == 0 {
            *i += 1;
            error("Invalid UTF-8 continuation byte");
            return UNICODE_REPLACEMENT_CHARACTER;
        }
        let ch2 = at(bytes, *i + 2);
        if (ch2 & 0x40) != 0 || (ch2 & 0x80) == 0 {
            *i += 2;
            error("Invalid UTF-8 continuation byte");
            return UNICODE_REPLACEMENT_CHARACTER;
        }
        *i += 3;
        result = ((ch & 0x0F) << 12) | ((ch1 & 0x3F) << 6) | (ch2 & 0x3F);
        if result <= 0x7FF {
            error("Non-canonical UTF-8 encoding");
            return UNICODE_REPLACEMENT_CHARACTER;
        }
        if result >= UNICODE_SURROGATE_FIRST && result <= UNICODE_SURROGATE_LAST && !ALLOW_SURROGATES
        {
            error(&format!("Invalid UTF-8 code point 0x{:X}", result));
            return UNICODE_REPLACEMENT_CHARACTER;
        }
    } else if (ch & 0xF8) == 0xF0 {
        let ch1 = at(bytes, *i + 1);
        if (ch1 & 0x40) != 0 || (ch1 & 0x80) == 0 {
            *i += 1;
            error("Invalid UTF-8 continuation byte");
            return UNICODE_REPLACEMENT_CHARACTER;
        }
        let ch2 = at(bytes, *i + 2);
        if (ch2 & 0x40) != 0 || (ch2 & 0x80) == 0 {
            *i += 2;
            error("Invalid UTF-8 continuation byte");
            return UNICODE_REPLACEMENT_CHARACTER;
        }
        let ch3 = at(bytes, *i + 3);
        if (ch3 & 0x40) != 0 || (ch3 & 0x80) == 0 {
            *i += 3;
            error("Invalid UTF-8 continuation byte");
            return UNICODE_REPLACEMENT_CHARACTER;
        }
        *i += 4;
        result =
            ((ch & 0x07) << 18) | ((ch1 & 0x3F) << 12) | ((ch2 & 0x3F) << 6) | (ch3 & 0x3F);
        if result <= 0xFFFF {
            error("Non-canonical UTF-8 encoding");
            return UNICODE_REPLACEMENT_CHARACTER;
        }
        if result > UNICODE_MAX_VALUE {
            error(&format!("Invalid UTF-8 code point 0x{:X}", result));
            return UNICODE_REPLACEMENT_CHARACTER;
        }
    } else {
        *i += 1;
        error(&format!("Invalid UTF-8 lead byte 0x{:X}", ch & 0xFF));
        return UNICODE_REPLACEMENT_CHARACTER;
    }

    result
}

/// Decode a sequence of UTF8 encoded bytes into a Unicode codepoint, ASCII fast
/// path. Port of `decodeUTF8` (UTF8.h:187-193). In case of decoding errors, the
/// provided callback is invoked with an appropriate message and
/// UNICODE_REPLACEMENT_CHARACTER is returned.
///
/// \tparam ALLOW_SURROGATES when false, values in the surrogate range are
///     reported as errors.
#[inline]
pub fn decode_utf8<const ALLOW_SURROGATES: bool>(
    bytes: &[u8],
    i: &mut usize,
    error: impl FnMut(&str),
) -> u32 {
    if *i < bytes.len() && (bytes[*i] & 0x80) == 0 {
        // Ordinary ASCII?
        let c = bytes[*i] as u32;
        *i += 1;
        return c;
    }
    decode_utf8_slow_path::<ALLOW_SURROGATES>(bytes, i, error)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classifiers() {
        assert!(!is_utf8_start(b'a'));
        assert!(is_utf8_start(0xc3));
        assert!(is_utf8_leading_byte(0xc3));
        assert!(!is_utf8_leading_byte(0x80));
        assert!(is_utf8_continuation_byte(0x80));
    }
    #[test]
    fn decode_ascii_and_multibyte() {
        // ASCII fast path advances by 1.
        let buf = b"a";
        let mut i = 0usize;
        assert_eq!(decode_utf8::<false>(buf, &mut i, |_| {}), 'a' as u32);
        assert_eq!(i, 1);
        // é = U+00E9 = c3 a9
        let buf = b"\xc3\xa9";
        let mut i = 0usize;
        assert_eq!(decode_utf8::<false>(buf, &mut i, |_| {}), 0x00E9);
        assert_eq!(i, 2);
        // U+1F600 = f0 9f 98 80
        let buf = b"\xf0\x9f\x98\x80";
        let mut i = 0usize;
        assert_eq!(decode_utf8::<false>(buf, &mut i, |_| {}), 0x1F600);
        assert_eq!(i, 4);
    }
    #[test]
    fn line_terminator_match() {
        assert!(match_unicode_line_terminator_offset1(b"\xe2\x80\xa8")); // U+2028
        assert!(match_unicode_line_terminator_offset1(b"\xe2\x80\xa9")); // U+2029
        assert!(!match_unicode_line_terminator_offset1(b"\xe2\x80\xaa"));
    }
    #[test]
    fn invalid_reports_error_and_replacement() {
        let buf = b"\xc3\x20"; // 0x20 is not a continuation byte
        let mut i = 0usize;
        let mut errs = 0;
        let cp = decode_utf8::<false>(buf, &mut i, |_| errs += 1);
        assert_eq!(cp, 0xFFFD);
        assert_eq!(errs, 1);
    }
}
