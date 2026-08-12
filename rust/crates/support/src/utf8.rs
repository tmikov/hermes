/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! WTF-8 / UTF-8 → UTF-16 codec helpers, faithfully copied from the subset of
//! `hermes_parser::utf8` (itself ported from `include/hermes/Support/UTF8.h`) that is
//! needed by `JSONEmitter` and the forthcoming AST-dumper port.
//!
//! Keeping this copy in `support` means `json_emitter` and the AST-dumper can
//! use it without taking a dependency on the `parser` crate, and without
//! duplicating logic. The module is zero-`unsafe` (the `support` crate
//! `forbid`s `unsafe_code`).

use hermes_unicode::{
    UNICODE_MAX_VALUE, UNICODE_REPLACEMENT_CHARACTER, UNICODE_SURROGATE_FIRST,
    UNICODE_SURROGATE_LAST, UTF16_HIGH_SURROGATE, UTF16_LOW_SURROGATE,
};

/// Check whether a byte is a regular ASCII or a UTF8 starting byte.
/// \return true if it is UTF8 starting byte.
#[inline]
pub fn is_utf8_start(ch: u8) -> bool {
    (ch & 0x80) != 0
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
// Keep the C++ `result >= FIRST && result <= LAST` surrogate-range check faithful
// to UTF8.h rather than rewriting it as `(FIRST..=LAST).contains(..)`.
#[allow(clippy::manual_range_contains)]
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

#[cfg(test)]
mod tests {
    use super::convert_utf8_with_surrogates_to_utf16;

    #[test]
    fn ascii_passthrough() {
        assert_eq!(convert_utf8_with_surrogates_to_utf16(b"abc"), vec![0x61, 0x62, 0x63]);
    }

    #[test]
    fn bmp_non_ascii() {
        // U+54C8 哈 = e5 93 88
        assert_eq!(convert_utf8_with_surrogates_to_utf16(&[0xE5, 0x93, 0x88]), vec![0x54C8]);
    }

    #[test]
    fn astral_4byte() {
        // U+1F44B 👋 = f0 9f 91 8b -> surrogate pair D83D DC4B
        assert_eq!(
            convert_utf8_with_surrogates_to_utf16(&[0xF0, 0x9F, 0x91, 0x8B]),
            vec![0xD83D, 0xDC4B]
        );
    }

    #[test]
    fn wtf8_lone_surrogate() {
        // Lone high surrogate U+D800 as WTF-8 = ed a0 80 -> single unit 0xD800
        assert_eq!(convert_utf8_with_surrogates_to_utf16(&[0xED, 0xA0, 0x80]), vec![0xD800]);
    }
}
