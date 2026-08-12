//! UTF-8 decode helpers, ported from include/hermes/Support/UTF8.h (decode side).
//!
//! These mirror the inline classifiers and `_decodeUTF8SlowPath`/`decodeUTF8`
//! from `Support/UTF8.h`. The C++ uses an advancing `const char *&from`; here we
//! use a slice plus a `&mut usize` index, so the raw-pointer parity lives only
//! in `cursor.rs`. The lexer always passes the NUL-terminated buffer, so an
//! out-of-range continuation read sees `0x00` (a non-continuation byte) and is
//! correctly rejected; we also guard indexes against `bytes.len()` defensively.

use hermes_unicode::{
    is_high_surrogate, is_low_surrogate, utf16_surrogate_pair_to_code_point,
    UNICODE_MAX_VALUE, UNICODE_REPLACEMENT_CHARACTER, UNICODE_SURROGATE_FIRST,
    UNICODE_SURROGATE_LAST, UTF16_HIGH_SURROGATE, UTF16_LOW_SURROGATE,
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

/// Encode a Unicode code point as UTF-8 (up to the legacy 6-byte form, matching
/// `encodeUTF8`), appending the bytes to `out`. Port of `UTF8.cpp:encodeUTF8`.
#[inline]
pub fn encode_utf8(out: &mut Vec<u8>, cp: u32) {
    if cp <= 0x7F {
        out.push(cp as u8);
    } else if cp <= 0x7FF {
        out.push(((cp >> 6) & 0x1F) as u8 | 0xC0);
        out.push((cp & 0x3F) as u8 | 0x80);
    } else if cp <= 0xFFFF {
        out.push(((cp >> 12) & 0x0F) as u8 | 0xE0);
        out.push(((cp >> 6) & 0x3F) as u8 | 0x80);
        out.push((cp & 0x3F) as u8 | 0x80);
    } else if cp <= 0x1FFFFF {
        out.push(((cp >> 18) & 0x07) as u8 | 0xF0);
        out.push(((cp >> 12) & 0x3F) as u8 | 0x80);
        out.push(((cp >> 6) & 0x3F) as u8 | 0x80);
        out.push((cp & 0x3F) as u8 | 0x80);
    } else if cp <= 0x3FFFFFF {
        out.push(((cp >> 24) & 0x03) as u8 | 0xF8);
        out.push(((cp >> 18) & 0x3F) as u8 | 0x80);
        out.push(((cp >> 12) & 0x3F) as u8 | 0x80);
        out.push(((cp >> 6) & 0x3F) as u8 | 0x80);
        out.push((cp & 0x3F) as u8 | 0x80);
    } else {
        out.push(((cp >> 30) & 0x01) as u8 | 0xFC);
        out.push(((cp >> 24) & 0x3F) as u8 | 0x80);
        out.push(((cp >> 18) & 0x3F) as u8 | 0x80);
        out.push(((cp >> 12) & 0x3F) as u8 | 0x80);
        out.push(((cp >> 6) & 0x3F) as u8 | 0x80);
        out.push((cp & 0x3F) as u8 | 0x80);
    }
}

/// Encode `cp` into `storage` like the lexer's `appendUnicodeToStorage`
/// (JSLexer.h:1125-1143): code points above 0xFFFF are first split into a
/// UTF-16 surrogate pair, and each surrogate is encoded individually into UTF-8
/// (technically invalid UTF-8 / WTF-8, which JS string & identifier storage
/// allows).
#[inline]
pub fn append_unicode_to_storage(storage: &mut Vec<u8>, cp: u32) {
    // We need to normalize code points which would be encoded with a surrogate
    // pair. Note that this produces technically invalid UTF-8.
    if cp < 0x10000 {
        encode_utf8(storage, cp);
    } else {
        debug_assert!(cp <= UNICODE_MAX_VALUE, "invalid Unicode value");
        let cp = cp - 0x10000;
        encode_utf8(storage, UTF16_HIGH_SURROGATE + ((cp >> 10) & 0x3FF));
        encode_utf8(storage, UTF16_LOW_SURROGATE + (cp & 0x3FF));
    }
}

/// Encode a 32-bit value into UTF-16, appending to `out`. If the value is a
/// part of a surrogate pair, it is encoded without any conversion. Port of
/// `encodeUTF16` (UTF8.h:197-210).
#[inline]
pub fn encode_utf16(out: &mut Vec<u16>, cp: u32) {
    if cp < 0x10000 {
        out.push(cp as u16);
    } else {
        debug_assert!(cp <= UNICODE_MAX_VALUE, "invalid Unicode value");
        let cp = cp - 0x10000;
        out.push((UTF16_HIGH_SURROGATE + ((cp >> 10) & 0x3FF)) as u16);
        out.push((UTF16_LOW_SURROGATE + (cp & 0x3FF)) as u16);
    }
}

/// Decode a UTF-8 sequence, which is assumed to be valid, but may possibly
/// contain explicitly encoded surrogate pairs, into a UTF-16 sequence. Port of
/// `convertUTF8WithSurrogatesToUTF16` (UTF8.h:216-225).
pub fn convert_utf8_with_surrogates_to_utf16(bytes: &[u8]) -> Vec<u16> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        // Surrogates are ALLOWED; the input is assumed valid, so the error
        // callback is unreachable (a no-op here).
        let cp = decode_utf8::<true>(bytes, &mut i, |_| {});
        encode_utf16(&mut out, cp);
    }
    out
}

/// Inspect the code unit at `u16s[i]`. If it is a high surrogate followed by a
/// low surrogate, decode the surrogate pair into a single code point. If it is
/// an unpaired surrogate, replace the value with `UNICODE_REPLACEMENT_CHARACTER`
/// (U+FFFD). Port of `convertToCodePointAt` (UTF8.cpp:77-96).
///
/// \return a pair with the first element being the Unicode code point, and the
///         second being how many code units were consumed.
#[inline]
fn convert_to_code_point_at(u16s: &[u16], i: usize) -> (u32, usize) {
    let c = u16s[i] as u32;
    if is_low_surrogate(c) {
        // Unpaired low surrogate.
        (UNICODE_REPLACEMENT_CHARACTER, 1)
    } else if is_high_surrogate(c) {
        // Leading high surrogate. See if the next character is a low surrogate.
        if i + 1 >= u16s.len() || !is_low_surrogate(u16s[i + 1] as u32) {
            // Trailing or unpaired high surrogate.
            (UNICODE_REPLACEMENT_CHARACTER, 1)
        } else {
            // Decode surrogate pair and consume two chars.
            (utf16_surrogate_pair_to_code_point(c, u16s[i + 1] as u32), 2)
        }
    } else {
        // Not a surrogate.
        (c, 1)
    }
}

/// Convert a UTF-16 encoded string `u16s` to valid UTF-8, combining surrogate
/// pairs into supplementary-plane characters and replacing unpaired surrogates
/// with U+FFFD. Port of `convertUTF16ToUTF8WithReplacements` (UTF8.cpp:99-133),
/// dropping the `maxCharacters` parameter (the lexer always passes 0/unbounded).
pub fn convert_utf16_to_utf8_with_replacements(u16s: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(u16s.len());
    let mut cur = 0usize;
    while cur < u16s.len() {
        let c = u16s[cur] as u32;
        // ASCII fast-path.
        if c <= 0x7F {
            out.push(c as u8);
            cur += 1;
            continue;
        }

        let (c32, input_consumed) = convert_to_code_point_at(u16s, cur);
        cur += input_consumed;

        // The code point to be encoded here is guaranteed to be a valid unicode
        // code point and not a surrogate. Because of the
        // convert_to_code_point_at() process.
        encode_utf8(&mut out, c32);
    }
    out
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
    fn encode_basic() {
        let mut v = vec![];
        encode_utf8(&mut v, 'a' as u32);
        assert_eq!(v, b"a");
        let mut v = vec![];
        encode_utf8(&mut v, 0x00E9);
        assert_eq!(v, b"\xc3\xa9"); // é
        let mut v = vec![];
        encode_utf8(&mut v, 0x4E2D);
        assert_eq!(v, b"\xe4\xb8\xad"); // 中
    }

    #[test]
    fn append_storage_surrogate_pair() {
        // BMP: plain UTF-8.
        let mut v = vec![];
        append_unicode_to_storage(&mut v, 0x00E9);
        assert_eq!(v, b"\xc3\xa9");
        // Astral U+1F600: split into surrogate pair, each encoded as 3-byte
        // WTF-8. high = 0xD83D, low = 0xDE00 -> ed a0 bd  ed b8 80
        let mut v = vec![];
        append_unicode_to_storage(&mut v, 0x1F600);
        assert_eq!(v, b"\xed\xa0\xbd\xed\xb8\x80");
    }

    #[test]
    fn utf16_roundtrip_and_replacement() {
        // encode_utf16: BMP -> 1 u16, astral -> surrogate pair.
        let mut v = vec![];
        encode_utf16(&mut v, 0x41);
        assert_eq!(v, [0x41]);
        let mut v = vec![];
        encode_utf16(&mut v, 0x1F600);
        assert_eq!(v, [0xD83D, 0xDE00]);

        // convert_utf8_with_surrogates_to_utf16: WTF-8 astral (surrogate pair,
        // 3 bytes each) -> 2 u16.
        let wtf8: &[u8] = b"\xed\xa0\xbd\xed\xb8\x80"; // U+1F600 as a surrogate pair (WTF-8)
        let u16s = convert_utf8_with_surrogates_to_utf16(wtf8);
        assert_eq!(u16s, [0xD83D, 0xDE00]);

        // convert_utf16_to_utf8_with_replacements: surrogate pair -> 4-byte
        // UTF-8; lone surrogate -> U+FFFD.
        assert_eq!(
            convert_utf16_to_utf8_with_replacements(&[0xD83D, 0xDE00]),
            b"\xf0\x9f\x98\x80".to_vec()
        );
        assert_eq!(
            convert_utf16_to_utf8_with_replacements(&[0xD800]),
            "\u{FFFD}".as_bytes().to_vec()
        ); // lone high
        assert_eq!(
            convert_utf16_to_utf8_with_replacements(&[0xDC00]),
            "\u{FFFD}".as_bytes().to_vec()
        ); // lone low
        assert_eq!(
            convert_utf16_to_utf8_with_replacements(&[0x41, 0x42]),
            b"AB".to_vec()
        );
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
